//! The per-launch instructions MassOrca gives an agent run (decision 0161): how the person's request for a child session
//! becomes a `massorca agent delegate` call. The text reaches a run only through its kind's declared `instructions`
//! channel, on the run's own command line; nothing is written into the vendor's configuration. A channel that declares a
//! support probe is used only when the installed CLI passes it; otherwise the run starts without the text, and the
//! session records why.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, SystemTime};

use crate::account_check::{account_strip_names, run_probe};
use crate::kind::KindDecl;

/// How long a support probe may run before it counts as failed.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// The file in a session's run directory that holds why its current run got no instructions.
const WITHHELD_FILE: &str = "instructions-withheld";

/// A probe's identity: the program, its modified time, and what is run.
type ProbeKey = (PathBuf, Option<SystemTime>, Vec<String>);

/// What a probe run printed and how it ended: the exit code (`None` when it did not finish) and its output.
#[derive(Debug, Clone)]
pub(crate) struct ProbeRun {
    pub(crate) exit: Option<i32>,
    pub(crate) output: String,
}

/// The probe runs of this service, so a probe runs once per installed program, not per launch. Shared by the instruction
/// channel and the one-shot title run's `--help` listing (decision 0167).
fn probe_cache() -> &'static Mutex<HashMap<ProbeKey, ProbeRun>> {
    static CACHE: OnceLock<Mutex<HashMap<ProbeKey, ProbeRun>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// What `program` prints for `args` (its `--help`, usually): run once per program path and modified time, with the names an
/// account command strips removed and no account home, and kept for the service's life.
pub(crate) fn probe_run(decl: &KindDecl, program: &Path, args: &[String]) -> ProbeRun {
    let modified = fs::metadata(program)
        .and_then(|metadata| metadata.modified())
        .ok();
    let key = (program.to_path_buf(), modified, args.to_vec());
    if let Some(known) = probe_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return known.clone();
    }
    let output = run_probe(program, args, &account_strip_names(decl), PROBE_TIMEOUT);
    let run = ProbeRun {
        exit: output.exit,
        output: output.output,
    };
    probe_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, run.clone());
    run
}

/// Whether `program`, the kind's resolved program, takes the kind's instruction channel: always without a declared
/// probe; else when the probe exits 0 within [`PROBE_TIMEOUT`] and its output contains the declared text (see
/// [`probe_run`] for how a probe runs and is kept). A kind without a channel takes none.
pub(crate) fn channel_supported(decl: &KindDecl, program: &Path) -> bool {
    let Some(channel) = &decl.instructions else {
        return false;
    };
    let Some(probe) = &channel.supported_when else {
        return true;
    };
    let run = probe_run(decl, program, &probe.args);
    run.exit == Some(0) && run.output.contains(&probe.contains)
}

/// What a session records when its run got no instructions because its CLI failed the channel's probe.
pub(crate) fn withheld_note(decl: &KindDecl) -> String {
    let flag = decl
        .instructions
        .as_ref()
        .and_then(|channel| channel.supported_when.as_ref())
        .map_or("the instruction channel", |probe| probe.contains.as_str());
    format!(
        "Delegation instructions were not given: this {} CLI does not support {flag}.",
        decl.name
    )
}

/// Records in the run directory why the current run got no instructions, or clears an earlier run's record. A failure
/// only loses the note.
pub(crate) fn record_withheld(run_dir: &Path, note: Option<&str>) {
    let path = run_dir.join(WITHHELD_FILE);
    match note {
        Some(note) => {
            let _ = fs::write(path, note);
        }
        None => {
            let _ = fs::remove_file(path);
        }
    }
}

/// The note [`record_withheld`] left for the session's current or last run, if any.
pub(crate) fn withheld(run_dir: &Path) -> Option<String> {
    fs::read_to_string(run_dir.join(WITHHELD_FILE))
        .ok()
        .filter(|note| !note.is_empty())
}

/// The instruction text. One line, without quotes, `<`, `>`, `|`, `&`, `%` or `^`, so a Windows `.cmd` shim passes it
/// as one argument unchanged. It names the commands the CLI takes, and leaves the agent's own subagents allowed.
pub const DELEGATION_INSTRUCTIONS: &str = "MassOrca child sessions. When the person asks for a child session, a \
delegated session, or work in its own session or its own worktree, run massorca agent delegate --prompt TASK --title \
NAME, quoting each value; add --read-only when the child will not edit files, and --kind KIND or --mode MODE to pick its \
agent or permission mode. The command prints the child at once. Your session id is in MASSORCA_AGENT_SESSION: \
massorca agent tree with that id lists the children; massorca agent status CHILD --until done waits for a child, and \
massorca agent status CHILD --json shows its result; massorca agent stop CHILD stops it; massorca agent bring-back \
CHILD merges a finished writing child. Your own subagents are a different thing: you may still use them, but the \
person cannot see them in the Sidebar.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_is_one_short_line_a_cmd_shim_passes_unchanged() {
        assert!(
            DELEGATION_INSTRUCTIONS.len() < 900,
            "{}",
            DELEGATION_INSTRUCTIONS.len()
        );
        assert!(!DELEGATION_INSTRUCTIONS.contains("  "));
        for forbidden in ['\n', '\r', '"', '\'', '<', '>', '|', '&', '%', '^'] {
            assert!(
                !DELEGATION_INSTRUCTIONS.contains(forbidden),
                "{forbidden:?} in the instructions"
            );
        }
        crate::launch::check_prompt(DELEGATION_INSTRUCTIONS).expect("a valid launch argument");
    }

    fn claude() -> KindDecl {
        crate::catalog::built_ins()
            .into_iter()
            .find(|kind| kind.id == "claude")
            .expect("built in")
    }

    #[test]
    fn the_withheld_note_names_the_cli_and_its_flag() {
        assert_eq!(
            withheld_note(&claude()),
            "Delegation instructions were not given: this Claude CLI does not support --append-system-prompt."
        );
    }

    #[test]
    fn the_withheld_note_is_kept_until_a_run_clears_it() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        assert_eq!(withheld(dir.path()), None);
        record_withheld(dir.path(), Some("not given"));
        assert_eq!(withheld(dir.path()).as_deref(), Some("not given"));
        record_withheld(dir.path(), None);
        assert_eq!(withheld(dir.path()), None);
    }

    #[test]
    fn a_channel_without_a_probe_is_supported_and_a_missing_program_fails_its_probe() {
        let mut kind = claude();
        let missing = std::env::temp_dir().join("massorca-no-such-program-for-a-probe.exe");
        assert!(!channel_supported(&kind, &missing));
        kind.instructions
            .as_mut()
            .expect("a channel")
            .supported_when = None;
        assert!(channel_supported(&kind, &missing));
        kind.instructions = None;
        assert!(!channel_supported(&kind, &missing));
    }

    #[test]
    fn the_text_names_the_cli_commands_and_allows_own_subagents() {
        for command in [
            "massorca agent delegate --prompt",
            "--read-only",
            "massorca agent tree",
            "massorca agent status CHILD --until done",
            "massorca agent stop CHILD",
            "massorca agent bring-back CHILD",
        ] {
            assert!(DELEGATION_INSTRUCTIONS.contains(command), "{command}");
        }
        assert!(DELEGATION_INSTRUCTIONS.contains("you may still use them"));
        assert!(!DELEGATION_INSTRUCTIONS.to_lowercase().contains("never use"));
    }
}
