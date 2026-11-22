//! `Git::log` against real repositories: fields, paging pinned to a commit, and empty history.

use massorca_core_git::{Git, LogCursor};

use crate::fixture::{Fixture, commit_file, rev, run_git};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

#[test]
fn a_page_carries_ids_parents_authors_and_messages() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, "a.txt", "a\n", "second commit\n\nwith a body");
    let head = rev(&repo, "HEAD");
    let start = LogCursor {
        start: head.clone(),
        skip: 0,
    };
    let log = git().log(&repo, &start, 10).expect("log");
    assert_eq!(log.commits.len(), 2);
    assert_eq!(log.next_cursor, None);
    let newest = &log.commits[0];
    assert_eq!(newest.id, head);
    assert_eq!(newest.parents, [rev(&repo, "HEAD~1")]);
    assert_eq!(newest.subject, "second commit");
    assert_eq!(newest.message, "second commit\n\nwith a body");
    assert_eq!(newest.author.name, "t");
    assert_eq!(newest.author.email, "t@example.invalid");
    assert!(newest.author.time_ms > 1_600_000_000_000);
    assert!(log.commits[1].parents.is_empty());
}

#[test]
fn pages_stay_on_the_first_commit_even_when_the_branch_moves() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    for n in 0..4 {
        commit_file(&repo, "n.txt", &format!("{n}\n"), &format!("commit {n}"));
    }
    let start = LogCursor {
        start: rev(&repo, "HEAD"),
        skip: 0,
    };
    let first = git().log(&repo, &start, 2).expect("page 1");
    let subjects: Vec<_> = first.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["commit 3", "commit 2"]);

    commit_file(&repo, "n.txt", "moved\n", "a commit after page one");
    let cursor = LogCursor::decode(first.next_cursor.as_deref().expect("more")).expect("cursor");
    let second = git().log(&repo, &cursor, 2).expect("page 2");
    let subjects: Vec<_> = second.commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, ["commit 1", "commit 0"]);

    let cursor = LogCursor::decode(second.next_cursor.as_deref().expect("more")).expect("cursor");
    let last = git().log(&repo, &cursor, 2).expect("page 3");
    assert_eq!(last.commits.len(), 1);
    assert_eq!(last.next_cursor, None);
}

#[test]
fn an_unborn_head_resolves_to_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    run_git(dir.path(), &["init", "-q"]);
    assert_eq!(
        git().resolve_commit(dir.path(), "HEAD").expect("resolve"),
        None
    );
    assert_eq!(
        git().resolve_commit(dir.path(), "--all").expect("resolve"),
        None
    );
}
