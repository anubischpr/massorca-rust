//! Whether a session can be resumed (decision 0160): its kind can resume, its agent ref is known, and, when the kind
//! declares where its CLI saves conversations, the conversation for that ref exists. A resumed run that still finds no
//! conversation leaves a mark, so the session is not offered for resume again.
//!
//! The transcript check reads metadata only, never a file, and follows links: only a search that completed and found
//! nothing counts as missing. The files kept
//! here live in the session's run directory (`paths::run_dir`).

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use massorca_protocol::codes;
use massorca_protocol::methods::agents::ResumeState;

use crate::credentials::is_link;
use crate::kind::{KindDecl, REF_PLACEHOLDER, ResumeTranscriptDecl};
use crate::paths;
use crate::sources::{self, Platform, expand};

/// The reason of `agent.resume_not_found`.
pub const NOT_FOUND_REASON: &str = "the agent saved no conversation for this session";

/// The account a run used, by id; absent when it used none.
const ACCOUNT_FILE: &str = "run-account";
/// The agent ref a resumed run was started with, while its kind declares a not-found pattern; removed at its exit.
const PROBE_FILE: &str = "resume-probe";
/// The agent ref a resumed run found no conversation for.
const NOT_FOUND_FILE: &str = "resume-not-found";

/// At most this many entries of one folder are looked at for a `*` part.
const MAX_FOLDER_ENTRIES: usize = 10_000;
/// The newest bytes of a failed resume's output the not-found pattern is matched against.
const OUTPUT_TAIL_BYTES: usize = 4096;
/// Longest text kept in one of the run directory's files.
const MAX_MARK_BYTES: u64 = 1024;

/// How the service reads the variables the vendor CLIs read, for the transcript check (decision 0160): `{home}` and the
/// override variable. `None` in [`crate::AgentsConfig`] never looks outside the profile.
#[derive(Clone)]
pub struct VendorEnv(Arc<VendorEnvFn>);

type VendorEnvFn = dyn Fn(&str) -> Option<OsString> + Send + Sync;

impl VendorEnv {
    pub fn new(lookup: impl Fn(&str) -> Option<OsString> + Send + Sync + 'static) -> Self {
        Self(Arc::new(lookup))
    }

    /// The service's own environment.
    #[must_use]
    pub fn process() -> Self {
        Self::new(|name: &str| std::env::var_os(name))
    }

    fn get(&self, name: &str) -> Option<OsString> {
        (self.0)(name)
    }
}

impl std::fmt::Debug for VendorEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VendorEnv")
    }
}

/// Records the account a run starts with, or that it uses none.
///
/// # Errors
/// The file cannot be written or removed.
pub(crate) fn record_run_account(run_dir: &Path, account: Option<&str>) -> std::io::Result<()> {
    let path = run_dir.join(ACCOUNT_FILE);
    match account {
        Some(account) => fs::write(path, account),
        None => match fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        },
    }
}

/// Records that a resumed run of `agent_ref` should be classified at its exit, when the kind declares a pattern; clears
/// a record of an earlier run otherwise.
///
/// # Errors
/// The file cannot be written or removed.
pub(crate) fn record_probe(
    run_dir: &Path,
    decl: &KindDecl,
    agent_ref: Option<&str>,
) -> std::io::Result<()> {
    let path = run_dir.join(PROBE_FILE);
    match agent_ref.filter(|_| decl.resume_not_found_pattern.is_some()) {
        Some(agent_ref) => fs::write(path, agent_ref),
        None => match fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        },
    }
}

/// Classifies the exit of a resumed run: when it failed early, before any hook report and within [`EARLY_EXIT`] of its
/// start (the probe's write), and the end of its output matches the kind's not-found pattern, the session's ref is
/// marked as having no conversation. A later exit is never classified, so text printed during a conversation cannot set
/// the mark. The probe is removed either way. Failures are ignored: the mark only saves the person a resume that would
/// fail again.
pub(crate) fn classify_exit(
    run_dir: &Path,
    decl: Option<&KindDecl>,
    exit: ExitFacts,
    output: impl FnOnce() -> Option<String>,
) {
    let probe = run_dir.join(PROBE_FILE);
    let started = fs::symlink_metadata(&probe).and_then(|meta| meta.modified());
    let Some(agent_ref) = read_mark(&probe) else {
        return;
    };
    let _ = fs::remove_file(&probe);
    let Some(pattern) = decl.and_then(|decl| decl.resume_not_found_pattern.as_deref()) else {
        return;
    };
    let early = started
        .ok()
        .and_then(|started| started.elapsed().ok())
        .is_some_and(|age| age < EARLY_EXIT);
    if !exit.failed || exit.reported || !early {
        return;
    }
    let Ok(pattern) = regex::Regex::new(pattern) else {
        return;
    };
    let Some(text) = output() else { return };
    let mut start = text.len().saturating_sub(OUTPUT_TAIL_BYTES);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    if pattern.is_match(&text[start..]) {
        let _ = fs::write(run_dir.join(NOT_FOUND_FILE), agent_ref);
    }
}

/// How long after its start a resumed run's failed exit may still be classified.
pub(crate) const EARLY_EXIT: std::time::Duration = std::time::Duration::from_secs(10);

/// What [`classify_exit`] needs to know about a run's end.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExitFacts {
    /// The run exited with a code other than 0.
    pub failed: bool,
    /// The run made a hook report before it exited.
    pub reported: bool,
}

/// A short text file of the run directory, when it is a plain file; links and oversized files read as absent.
fn read_mark(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if is_link(&meta) || !meta.is_file() || meta.len() > MAX_MARK_BYTES {
        return None;
    }
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

/// What the resume state of one session is computed from.
pub(crate) struct ResumeInputs<'a> {
    /// The session's kind when it is declared and valid; `Err` says why it cannot resume.
    pub decl: Result<&'a KindDecl, String>,
    pub agent_ref: Option<&'a str>,
    /// Only an exited session is checked for its transcript: a live run is still writing it.
    pub exited: bool,
    pub profile_dir: &'a Path,
    pub session_id: &'a str,
    pub vendor_env: Option<&'a VendorEnv>,
    /// Transcripts already found, by folder and ref; a found one is not looked for again.
    pub found: Option<&'a FoundTranscripts>,
}

/// Transcripts the check found, by folder and ref, so views of exited sessions do not search the folders again each
/// time. Only a positive answer is kept: a missing transcript is looked for at each view, so one the CLI writes later is
/// seen. The service clears it when a run starts or exits.
#[derive(Debug, Default)]
pub struct FoundTranscripts(Mutex<HashSet<(PathBuf, String)>>);

impl FoundTranscripts {
    fn set(&self) -> std::sync::MutexGuard<'_, HashSet<(PathBuf, String)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn contains(&self, folder: &Path, agent_ref: &str) -> bool {
        self.set()
            .contains(&(folder.to_path_buf(), agent_ref.to_owned()))
    }

    fn insert(&self, folder: &Path, agent_ref: &str) {
        self.set()
            .insert((folder.to_path_buf(), agent_ref.to_owned()));
    }

    /// Forgets every found transcript.
    pub fn clear(&self) {
        self.set().clear();
    }
}

/// What a transcript search concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Search {
    Found,
    /// The search completed and nothing matched.
    Missing,
    /// The search could not complete: an unreadable folder, too many entries, an unusable ref. Counts as available.
    Unknown,
}

/// The session's resume state (decision 0160).
pub(crate) fn resume_state(inputs: &ResumeInputs<'_>) -> ResumeState {
    let decl = match &inputs.decl {
        Ok(decl) => *decl,
        Err(reason) => {
            return ResumeState::refused(codes::AGENT_RESUME_UNAVAILABLE, reason.clone());
        }
    };
    if !decl.resumable() {
        return ResumeState::refused(
            codes::AGENT_RESUME_UNAVAILABLE,
            format!("kind {} declares no resume form", decl.id),
        );
    }
    let Some(agent_ref) = inputs.agent_ref else {
        return ResumeState::refused(
            codes::AGENT_RESUME_UNAVAILABLE,
            "its agent ref is not known",
        );
    };
    if !inputs.exited {
        return ResumeState::refused(codes::AGENT_BUSY, "the session is running");
    }
    let run_dir = paths::run_dir(inputs.profile_dir, inputs.session_id);
    let search = decl.resume_transcript.as_ref().and_then(|transcript| {
        let folder = transcript_folder(transcript, decl, &run_dir, inputs)?;
        if inputs
            .found
            .is_some_and(|found| found.contains(&folder, agent_ref))
        {
            return Some(Search::Found);
        }
        let search = transcript_search(&folder, &transcript.glob, agent_ref);
        if let (Search::Found, Some(found)) = (search, inputs.found) {
            found.insert(&folder, agent_ref);
        }
        Some(search)
    });
    let marked = read_mark(&run_dir.join(NOT_FOUND_FILE)).as_deref() == Some(agent_ref);
    match search {
        // The transcript is there after all: the mark of an earlier failed resume no longer holds.
        Some(Search::Found) => {
            if marked {
                let _ = fs::remove_file(run_dir.join(NOT_FOUND_FILE));
            }
            ResumeState::available()
        }
        Some(Search::Missing) => {
            ResumeState::refused(codes::AGENT_RESUME_NOT_FOUND, NOT_FOUND_REASON)
        }
        _ if marked => ResumeState::refused(codes::AGENT_RESUME_NOT_FOUND, NOT_FOUND_REASON),
        _ => ResumeState::available(),
    }
}

/// The folder the run's CLI saved its conversations under: the account home it used, else the override variable's
/// absolute local folder, else `dir` with `{home}`. `None` when it cannot be told, and then nothing is checked.
fn transcript_folder(
    transcript: &ResumeTranscriptDecl,
    decl: &KindDecl,
    run_dir: &Path,
    inputs: &ResumeInputs<'_>,
) -> Option<PathBuf> {
    if let Some(account) = read_mark(&run_dir.join(ACCOUNT_FILE)) {
        return paths::valid_account_home(inputs.profile_dir, &decl.id, &account);
    }
    let env = inputs.vendor_env?;
    let lookup = |name: &str| env.get(name);
    if let Some(name) = &transcript.env
        && let Some(dir) = sources::override_dir(&lookup, name)
    {
        return Some(dir);
    }
    let home = sources::user_home(Platform::current(), &lookup);
    expand(&transcript.dir, home.as_deref())
}

/// Looks for `glob` under `folder`, with `agent_ref` in place of `{ref}`, from metadata alone; links and junctions are
/// followed. Only a search that completed and matched nothing is [`Search::Missing`]. A folder that cannot be read, a
/// folder with more than [`MAX_FOLDER_ENTRIES`] entries, or a ref that is not a plain file name is [`Search::Unknown`].
pub(crate) fn transcript_search(folder: &Path, glob: &str, agent_ref: &str) -> Search {
    search_capped(folder, glob, agent_ref, MAX_FOLDER_ENTRIES)
}

fn search_capped(folder: &Path, glob: &str, agent_ref: &str, cap: usize) -> Search {
    if !plain_ref(agent_ref) {
        return Search::Unknown;
    }
    let parts: Vec<&str> = glob.split('/').collect();
    let Some((last, folders)) = parts.split_last() else {
        return Search::Unknown;
    };
    let file = last.replacen(REF_PLACEHOLDER, agent_ref, 1);
    let mut current = vec![folder.to_path_buf()];
    for part in folders {
        let mut next = Vec::new();
        for dir in &current {
            if *part == "*" {
                let entries = match fs::read_dir(dir) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == ErrorKind::NotFound => continue,
                    Err(_) => return Search::Unknown,
                };
                for (count, entry) in entries.enumerate() {
                    let Ok(entry) = entry else {
                        return Search::Unknown;
                    };
                    if count >= cap {
                        return Search::Unknown;
                    }
                    match kind_of(&entry.path()) {
                        Some(Some(true)) => next.push(entry.path()),
                        Some(_) => {}
                        None => return Search::Unknown,
                    }
                }
            } else {
                let path = dir.join(part);
                match kind_of(&path) {
                    Some(Some(true)) => next.push(path),
                    Some(_) => {}
                    None => return Search::Unknown,
                }
            }
        }
        current = next;
    }
    let mut found = false;
    for dir in &current {
        match kind_of(&dir.join(&file)) {
            Some(Some(false)) => found = true,
            Some(_) => {}
            None => return Search::Unknown,
        }
    }
    if found {
        Search::Found
    } else {
        Search::Missing
    }
}

/// What `path` is, following links: `Some(Some(true))` a folder, `Some(Some(false))` a file, `Some(None)` absent or
/// neither; `None` when its metadata cannot be read for another reason than its absence.
fn kind_of(path: &Path) -> Option<Option<bool>> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => Some(Some(true)),
        Ok(meta) if meta.is_file() => Some(Some(false)),
        Ok(_) => Some(None),
        Err(error) if error.kind() == ErrorKind::NotFound => Some(None),
        Err(_) => None,
    }
}

/// A ref usable as part of a file name: no separator, no `..`, no NUL.
fn plain_ref(agent_ref: &str) -> bool {
    !agent_ref.is_empty()
        && agent_ref != "."
        && agent_ref != ".."
        && !agent_ref.contains(['/', '\\', '\0', ':'])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(extra: serde_json::Value) -> KindDecl {
        let mut value = serde_json::json!({
            "id": "fake", "name": "Fake", "program": ["fake"],
            "session_ref": { "assign": ["--session-id", "{ref}"] },
            "resume": ["--resume", "{ref}"]
        });
        for (key, field) in extra.as_object().unwrap() {
            value[key] = field.clone();
        }
        KindDecl::from_json(&value.to_string()).unwrap()
    }

    fn transcript_kind() -> KindDecl {
        kind(serde_json::json!({
            "resume_transcript": { "env": "FAKE_VENDOR_DIR", "dir": "{home}/.fake", "glob": "projects/*/{ref}.jsonl" }
        }))
    }

    fn env_of(pairs: Vec<(&'static str, PathBuf)>) -> VendorEnv {
        VendorEnv::new(move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone().into_os_string())
        })
    }

    fn home_env(home: &Path) -> VendorEnv {
        let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        env_of(vec![(name, home.to_path_buf())])
    }

    fn state(
        decl: &KindDecl,
        agent_ref: Option<&str>,
        profile: &Path,
        env: Option<&VendorEnv>,
    ) -> ResumeState {
        resume_state(&ResumeInputs {
            decl: Ok(decl),
            agent_ref,
            exited: true,
            profile_dir: profile,
            session_id: "s-1",
            vendor_env: env,
            found: None,
        })
    }

    #[test]
    fn a_kind_that_cannot_resume_or_an_unknown_ref_is_unavailable() {
        let profile = tempfile::tempdir().unwrap();
        let plain =
            KindDecl::from_json(r#"{"id":"plain","name":"Plain","program":["p"]}"#).unwrap();
        let refused = state(&plain, Some("r"), profile.path(), None);
        assert_eq!(
            refused.code.as_deref(),
            Some(codes::AGENT_RESUME_UNAVAILABLE)
        );
        let no_ref = state(&kind(serde_json::json!({})), None, profile.path(), None);
        assert_eq!(
            no_ref.code.as_deref(),
            Some(codes::AGENT_RESUME_UNAVAILABLE)
        );
        assert!(!no_ref.available);
        let missing = resume_state(&ResumeInputs {
            decl: Err("kind gone is no longer declared".to_owned()),
            agent_ref: Some("r"),
            exited: true,
            profile_dir: profile.path(),
            session_id: "s-1",
            vendor_env: None,
            found: None,
        });
        assert_eq!(
            missing.reason.as_deref(),
            Some("kind gone is no longer declared")
        );
        assert!(
            state(
                &kind(serde_json::json!({})),
                Some("r"),
                profile.path(),
                None
            )
            .available
        );
    }

    #[test]
    fn the_transcript_decides_under_the_home_the_override_or_the_account() {
        let profile = tempfile::tempdir().unwrap();
        let user = tempfile::tempdir().unwrap();
        let decl = transcript_kind();
        let env = home_env(user.path());
        let missing = state(&decl, Some("ref-1"), profile.path(), Some(&env));
        assert_eq!(missing.code.as_deref(), Some(codes::AGENT_RESUME_NOT_FOUND));
        assert_eq!(missing.reason.as_deref(), Some(NOT_FOUND_REASON));

        let project = user.path().join(".fake").join("projects").join("p1");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("ref-1.jsonl"), "{}").unwrap();
        assert!(state(&decl, Some("ref-1"), profile.path(), Some(&env)).available);
        assert!(!state(&decl, Some("ref-2"), profile.path(), Some(&env)).available);

        // The override variable, set to an absolute folder, wins over the home.
        let elsewhere = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let overridden = env_of(vec![
            (name, user.path().to_path_buf()),
            ("FAKE_VENDOR_DIR", elsewhere.path().to_path_buf()),
        ]);
        assert!(!state(&decl, Some("ref-1"), profile.path(), Some(&overridden)).available);

        // A run that used an account is checked under that account's home, whatever the variables say.
        let account = "0190d5d1-1111-7000-8000-000000000001";
        let run_dir = paths::run_dir(profile.path(), "s-1");
        fs::create_dir_all(&run_dir).unwrap();
        record_run_account(&run_dir, Some(account)).unwrap();
        assert!(!state(&decl, Some("ref-1"), profile.path(), Some(&env)).available);
        let home = paths::account_home(profile.path(), "fake", account)
            .join("projects")
            .join("p2");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join("ref-1.jsonl"), "{}").unwrap();
        assert!(state(&decl, Some("ref-1"), profile.path(), None).available);
        record_run_account(&run_dir, None).unwrap();
        assert!(
            state(&decl, Some("ref-1"), profile.path(), None).available,
            "no env: not checked"
        );
    }

    #[test]
    fn a_live_session_is_busy() {
        let profile = tempfile::tempdir().unwrap();
        let live = resume_state(&ResumeInputs {
            decl: Ok(&kind(serde_json::json!({}))),
            agent_ref: Some("r"),
            exited: false,
            profile_dir: profile.path(),
            session_id: "s-1",
            vendor_env: None,
            found: None,
        });
        assert_eq!(
            live,
            ResumeState::refused(codes::AGENT_BUSY, "the session is running")
        );
    }

    #[test]
    fn a_ref_that_is_not_a_plain_name_is_unknown() {
        let folder = tempfile::tempdir().unwrap();
        fs::create_dir_all(folder.path().join("projects").join("p")).unwrap();
        fs::write(folder.path().join("projects").join("p").join("x.jsonl"), "").unwrap();
        let glob = "projects/*/{ref}.jsonl";
        assert_eq!(transcript_search(folder.path(), glob, "x"), Search::Found);
        assert_eq!(transcript_search(folder.path(), glob, "y"), Search::Missing);
        for bad in ["../p/x", "p/x", "..", ""] {
            assert_eq!(
                transcript_search(folder.path(), glob, bad),
                Search::Unknown,
                "{bad}"
            );
        }
    }

    #[test]
    fn an_unreadable_or_capped_search_is_unknown_and_the_session_stays_resumable() {
        let user = tempfile::tempdir().unwrap();
        let projects = user.path().join(".fake").join("projects");
        for name in ["a", "b", "c"] {
            fs::create_dir_all(projects.join(name)).unwrap();
        }
        let glob = "projects/*/{ref}.jsonl";
        let folder = user.path().join(".fake");
        assert_eq!(search_capped(&folder, glob, "r", 3), Search::Missing);
        assert_eq!(
            search_capped(&folder, glob, "r", 2),
            Search::Unknown,
            "over the cap"
        );

        // A folder that cannot be listed (here a file where the folder should be).
        let blocked = tempfile::tempdir().unwrap();
        let not_a_folder = blocked.path().join("projects");
        fs::write(&not_a_folder, "").unwrap();
        assert_eq!(
            transcript_search(&not_a_folder, "*/{ref}.jsonl", "r"),
            Search::Unknown
        );
        let profile = tempfile::tempdir().unwrap();
        let decl = kind(serde_json::json!({
            "resume_transcript": { "dir": "{home}/projects", "glob": "*/{ref}.jsonl" }
        }));
        let env = home_env(blocked.path());
        assert!(
            state(&decl, Some("r"), profile.path(), Some(&env)).available,
            "unknown is available"
        );

        // No home to look under: nothing is checked.
        let no_home = env_of(Vec::new());
        assert!(
            state(
                &transcript_kind(),
                Some("r"),
                profile.path(),
                Some(&no_home)
            )
            .available
        );
    }

    #[cfg(unix)]
    fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }

    #[test]
    fn a_linked_transcript_or_folder_is_followed_when_a_link_can_be_made() {
        let folder = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("x.jsonl"), "").unwrap();
        fs::write(outside.path().join("y.jsonl"), "").unwrap();
        let project = folder.path().join("projects").join("p");
        fs::create_dir_all(&project).unwrap();
        if make_link(&outside.path().join("x.jsonl"), &project.join("x.jsonl")).is_err() {
            return; // Links need a privilege this machine does not grant.
        }
        let glob = "projects/*/{ref}.jsonl";
        assert_eq!(transcript_search(folder.path(), glob, "x"), Search::Found);
        make_link(outside.path(), &folder.path().join("projects").join("q")).unwrap();
        assert_eq!(transcript_search(folder.path(), glob, "y"), Search::Found);
    }

    /// A failed exit of a resumed run that made no report, `age` after its probe was written.
    fn exit_after(run_dir: &Path, decl: &KindDecl, age: std::time::Duration, reported: bool) {
        record_probe(run_dir, decl, Some("ref-1")).unwrap();
        let written = std::time::SystemTime::now() - age;
        fs::File::options()
            .write(true)
            .open(run_dir.join(PROBE_FILE))
            .unwrap()
            .set_modified(written)
            .unwrap();
        let facts = ExitFacts {
            failed: true,
            reported,
        };
        classify_exit(run_dir, Some(decl), facts, || {
            Some("ready\nNo conversation found with session ID: ref-1\n".to_owned())
        });
    }

    #[test]
    fn only_an_early_failed_resume_whose_output_matches_marks_its_ref_not_found() {
        let profile = tempfile::tempdir().unwrap();
        let decl = kind(serde_json::json!({ "resume_not_found_pattern": "No conversation found" }));
        let run_dir = paths::run_dir(profile.path(), "s-1");
        fs::create_dir_all(&run_dir).unwrap();
        let ok = |run_dir: &Path, failed: bool, text: &str| {
            record_probe(run_dir, &decl, Some("ref-1")).unwrap();
            let text = text.to_owned();
            classify_exit(
                run_dir,
                Some(&decl),
                ExitFacts {
                    failed,
                    reported: false,
                },
                || Some(text),
            );
        };

        ok(&run_dir, false, "No conversation found");
        assert!(
            state(&decl, Some("ref-1"), profile.path(), None).available,
            "a run that succeeded"
        );
        ok(&run_dir, true, "some other failure");
        assert!(
            state(&decl, Some("ref-1"), profile.path(), None).available,
            "another failure"
        );

        // A late exit, or one after a hook report, with the phrase on screen does not mark.
        exit_after(
            &run_dir,
            &decl,
            EARLY_EXIT + std::time::Duration::from_secs(5),
            false,
        );
        assert!(
            state(&decl, Some("ref-1"), profile.path(), None).available,
            "a late exit"
        );
        exit_after(&run_dir, &decl, std::time::Duration::ZERO, true);
        assert!(
            state(&decl, Some("ref-1"), profile.path(), None).available,
            "after a report"
        );

        exit_after(&run_dir, &decl, std::time::Duration::ZERO, false);
        let marked = state(&decl, Some("ref-1"), profile.path(), None);
        assert_eq!(marked.code.as_deref(), Some(codes::AGENT_RESUME_NOT_FOUND));
        assert!(
            state(&decl, Some("ref-2"), profile.path(), None).available,
            "another ref"
        );

        // Without a probe, as after a first run, nothing is classified.
        let other = paths::run_dir(profile.path(), "s-2");
        fs::create_dir_all(&other).unwrap();
        classify_exit(
            &other,
            Some(&decl),
            ExitFacts {
                failed: true,
                reported: false,
            },
            || Some("No conversation found".to_owned()),
        );
        assert!(!other.join(NOT_FOUND_FILE).exists());
    }

    #[test]
    fn a_found_transcript_clears_the_mark() {
        let profile = tempfile::tempdir().unwrap();
        let user = tempfile::tempdir().unwrap();
        let decl = kind(serde_json::json!({
            "resume_not_found_pattern": "No conversation found",
            "resume_transcript": { "dir": "{home}/.fake", "glob": "projects/*/{ref}.jsonl" }
        }));
        let run_dir = paths::run_dir(profile.path(), "s-1");
        fs::create_dir_all(&run_dir).unwrap();
        exit_after(&run_dir, &decl, std::time::Duration::ZERO, false);
        assert!(run_dir.join(NOT_FOUND_FILE).exists());
        let env = home_env(user.path());
        assert!(!state(&decl, Some("ref-1"), profile.path(), Some(&env)).available);

        let project = user.path().join(".fake").join("projects").join("p");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("ref-1.jsonl"), "").unwrap();
        let found = FoundTranscripts::default();
        let shown = resume_state(&ResumeInputs {
            decl: Ok(&decl),
            agent_ref: Some("ref-1"),
            exited: true,
            profile_dir: profile.path(),
            session_id: "s-1",
            vendor_env: Some(&env),
            found: Some(&found),
        });
        assert!(shown.available);
        assert!(
            !run_dir.join(NOT_FOUND_FILE).exists(),
            "the mark is dropped"
        );
        assert!(
            found.contains(&user.path().join(".fake"), "ref-1"),
            "the find is kept"
        );
    }
}
