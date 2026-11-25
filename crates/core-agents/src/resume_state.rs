//! Whether a session can be resumed (decision 0160): its kind can resume, its agent ref is known, and, when the kind
//! declares where its CLI saves conversations, the conversation for that ref exists. A resumed run that still finds no
//! conversation leaves a mark, so the session is not offered for resume again.
//!
//! The transcript check reads metadata only, never a file, and refuses links the way an import does. The files kept
//! here live in the session's run directory (`paths::run_dir`).

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// Classifies the exit of a resumed run: when it failed and the end of its output matches the kind's not-found pattern,
/// the session's ref is marked as having no conversation. The probe is removed either way. Failures are ignored: the
/// mark only saves the person a resume that would fail again.
pub(crate) fn classify_exit(
    run_dir: &Path,
    decl: Option<&KindDecl>,
    failed: bool,
    output: impl FnOnce() -> Option<String>,
) {
    let Some(agent_ref) = read_mark(&run_dir.join(PROBE_FILE)) else {
        return;
    };
    let _ = fs::remove_file(run_dir.join(PROBE_FILE));
    let Some(pattern) = decl.and_then(|decl| decl.resume_not_found_pattern.as_deref()) else {
        return;
    };
    if !failed {
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
    let run_dir = paths::run_dir(inputs.profile_dir, inputs.session_id);
    if read_mark(&run_dir.join(NOT_FOUND_FILE)).as_deref() == Some(agent_ref) {
        return ResumeState::refused(codes::AGENT_RESUME_NOT_FOUND, NOT_FOUND_REASON);
    }
    if inputs.exited
        && let Some(transcript) = &decl.resume_transcript
        && let Some(folder) = transcript_folder(transcript, decl, &run_dir, inputs)
        && !transcript_exists(&folder, &transcript.glob, agent_ref)
    {
        return ResumeState::refused(codes::AGENT_RESUME_NOT_FOUND, NOT_FOUND_REASON);
    }
    ResumeState::available()
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

/// Whether `glob` under `folder`, with `agent_ref` in place of `{ref}`, names a plain file. Every folder on the way and
/// the file itself must not be a link; a ref that is not a plain file name matches nothing.
pub(crate) fn transcript_exists(folder: &Path, glob: &str, agent_ref: &str) -> bool {
    if !plain_ref(agent_ref) {
        return false;
    }
    let parts: Vec<&str> = glob.split('/').collect();
    let Some((last, folders)) = parts.split_last() else {
        return false;
    };
    let file = last.replacen(REF_PLACEHOLDER, agent_ref, 1);
    let mut current = vec![folder.to_path_buf()];
    for part in folders {
        let mut next = Vec::new();
        for dir in &current {
            if *part == "*" {
                let Ok(entries) = fs::read_dir(dir) else {
                    continue;
                };
                next.extend(
                    entries
                        .take(MAX_FOLDER_ENTRIES)
                        .flatten()
                        .map(|entry| entry.path())
                        .filter(|path| plain_dir(path)),
                );
            } else {
                let path = dir.join(part);
                if plain_dir(&path) {
                    next.push(path);
                }
            }
        }
        current = next;
    }
    current.iter().any(|dir| {
        fs::symlink_metadata(dir.join(&file)).is_ok_and(|meta| !is_link(&meta) && meta.is_file())
    })
}

fn plain_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| !is_link(&meta) && meta.is_dir())
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
    fn a_ref_that_is_not_a_plain_name_matches_nothing() {
        let folder = tempfile::tempdir().unwrap();
        fs::create_dir_all(folder.path().join("projects").join("p")).unwrap();
        fs::write(folder.path().join("projects").join("p").join("x.jsonl"), "").unwrap();
        assert!(transcript_exists(
            folder.path(),
            "projects/*/{ref}.jsonl",
            "x"
        ));
        for bad in ["../p/x", "p/x", "..", ""] {
            assert!(
                !transcript_exists(folder.path(), "projects/*/{ref}.jsonl", bad),
                "{bad}"
            );
        }
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
    fn a_linked_transcript_or_folder_counts_as_missing_when_a_link_can_be_made() {
        let folder = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("x.jsonl"), "").unwrap();
        let project = folder.path().join("projects").join("p");
        fs::create_dir_all(&project).unwrap();
        if make_link(&outside.path().join("x.jsonl"), &project.join("x.jsonl")).is_err() {
            return; // Links need a privilege this machine does not grant.
        }
        assert!(!transcript_exists(
            folder.path(),
            "projects/*/{ref}.jsonl",
            "x"
        ));
        let linked = folder.path().join("projects").join("q");
        make_link(outside.path(), &linked).unwrap();
        assert!(!transcript_exists(
            folder.path(),
            "projects/*/{ref}.jsonl",
            "x"
        ));
    }

    #[test]
    fn a_failed_resume_whose_output_matches_marks_its_ref_not_found() {
        let profile = tempfile::tempdir().unwrap();
        let decl = kind(serde_json::json!({ "resume_not_found_pattern": "No conversation found" }));
        let run_dir = paths::run_dir(profile.path(), "s-1");
        fs::create_dir_all(&run_dir).unwrap();

        record_probe(&run_dir, &decl, Some("ref-1")).unwrap();
        classify_exit(&run_dir, Some(&decl), false, || {
            Some("No conversation found".to_owned())
        });
        assert!(
            state(&decl, Some("ref-1"), profile.path(), None).available,
            "a run that succeeded"
        );

        record_probe(&run_dir, &decl, Some("ref-1")).unwrap();
        classify_exit(&run_dir, Some(&decl), true, || {
            Some("some other failure".to_owned())
        });
        assert!(
            state(&decl, Some("ref-1"), profile.path(), None).available,
            "another failure"
        );

        record_probe(&run_dir, &decl, Some("ref-1")).unwrap();
        classify_exit(&run_dir, Some(&decl), true, || {
            Some("ready\nNo conversation found with session ID: ref-1\n".to_owned())
        });
        let marked = state(&decl, Some("ref-1"), profile.path(), None);
        assert_eq!(marked.code.as_deref(), Some(codes::AGENT_RESUME_NOT_FOUND));
        assert!(
            state(&decl, Some("ref-2"), profile.path(), None).available,
            "another ref"
        );

        // Without a probe, as after a first run, nothing is classified.
        let other = paths::run_dir(profile.path(), "s-2");
        fs::create_dir_all(&other).unwrap();
        classify_exit(&other, Some(&decl), true, || {
            Some("No conversation found".to_owned())
        });
        assert!(!other.join(NOT_FOUND_FILE).exists());
    }
}
