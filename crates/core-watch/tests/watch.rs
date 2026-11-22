//! Real file-system events on a real repository: each kind arrives, bursts are debounced, a quiet tree is silent.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use massorca_core_watch::{WatchKind, WatchSpec, watch};

fn git(dir: &Path, args: &[&str]) {
    let output = massorca_process::command("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Watched {
    _dir: tempfile::TempDir,
    repo: std::path::PathBuf,
    bursts: mpsc::Receiver<BTreeSet<WatchKind>>,
    _handle: massorca_core_watch::WatchHandle,
}

fn watched_repo() -> Watched {
    let dir = tempfile::tempdir().expect("temp dir");
    let repo = dunce::canonicalize(dir.path())
        .expect("canonical")
        .join("repo");
    std::fs::create_dir_all(&repo).expect("mkdir");
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("a.txt"), "a\n").expect("write");
    git(&repo, &["add", "a.txt"]);
    git(&repo, &["commit", "-q", "-m", "initial"]);
    let spec = WatchSpec {
        worktree: repo.clone(),
        git_dir: repo.join(".git"),
        common_dir: repo.join(".git"),
    };
    let (sender, bursts) = mpsc::channel();
    let handle = watch(spec, move |kinds| {
        let _ = sender.send(kinds);
    })
    .expect("watch");
    assert_eq!(handle.degraded(), None);
    // Let the watches settle before the first change.
    std::thread::sleep(Duration::from_millis(300));
    Watched {
        _dir: dir,
        repo,
        bursts,
        _handle: handle,
    }
}

/// Every kind reported within `limit`, until `wanted` are all seen.
fn kinds_within(
    bursts: &mpsc::Receiver<BTreeSet<WatchKind>>,
    wanted: &[WatchKind],
    limit: Duration,
) -> BTreeSet<WatchKind> {
    let deadline = Instant::now() + limit;
    let mut seen = BTreeSet::new();
    while !wanted.iter().all(|kind| seen.contains(kind)) {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        match bursts.recv_timeout(left) {
            Ok(kinds) => seen.extend(kinds),
            Err(_) => break,
        }
    }
    seen
}

#[test]
fn an_edit_a_stage_and_a_commit_each_report_their_kinds() {
    let watched = watched_repo();
    std::fs::write(watched.repo.join("a.txt"), "edited\n").expect("write");
    let seen = kinds_within(
        &watched.bursts,
        &[WatchKind::Worktree],
        Duration::from_secs(10),
    );
    assert!(seen.contains(&WatchKind::Worktree), "{seen:?}");

    git(&watched.repo, &["add", "a.txt"]);
    let seen = kinds_within(
        &watched.bursts,
        &[WatchKind::Index],
        Duration::from_secs(10),
    );
    assert!(seen.contains(&WatchKind::Index), "{seen:?}");

    git(&watched.repo, &["commit", "-q", "-m", "edit"]);
    let seen = kinds_within(&watched.bursts, &[WatchKind::Refs], Duration::from_secs(10));
    assert!(seen.contains(&WatchKind::Refs), "{seen:?}");

    git(&watched.repo, &["checkout", "-q", "-b", "other"]);
    let seen = kinds_within(&watched.bursts, &[WatchKind::Head], Duration::from_secs(10));
    assert!(seen.contains(&WatchKind::Head), "{seen:?}");
}

#[test]
fn a_burst_of_writes_is_reported_once_and_a_quiet_tree_is_silent() {
    let watched = watched_repo();
    for n in 0..50 {
        std::fs::write(watched.repo.join(format!("f{n}.txt")), "x").expect("write");
    }
    let first = watched
        .bursts
        .recv_timeout(Duration::from_secs(10))
        .expect("a burst");
    assert!(first.contains(&WatchKind::Worktree), "{first:?}");
    // Fifty writes within a few milliseconds are one or two bursts, never fifty.
    let mut extra = 0;
    while watched
        .bursts
        .recv_timeout(Duration::from_millis(600))
        .is_ok()
    {
        extra += 1;
    }
    assert!(extra <= 2, "{extra} extra bursts");
    assert!(
        watched.bursts.recv_timeout(Duration::from_secs(2)).is_err(),
        "a quiet tree reported a change"
    );
}

#[test]
fn reading_the_repository_reports_nothing() {
    let watched = watched_repo();
    // A status read under GIT_OPTIONAL_LOCKS=0 must not wake the watcher (decision 0012).
    let output = massorca_process::command("git")
        .args(["status", "--porcelain=v2"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .current_dir(&watched.repo)
        .output()
        .expect("status");
    assert!(output.status.success());
    assert!(
        watched.bursts.recv_timeout(Duration::from_secs(2)).is_err(),
        "a read was reported as a change"
    );
}
