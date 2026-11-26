//! The per-launch instructions MassOrca gives an agent run (decision 0161): how the person's request for a child session
//! becomes a `massorca agent delegate` call. The text reaches a run only through its kind's declared `instructions`
//! channel, on the run's own command line; nothing is written into the vendor's configuration. A channel that declares a
//! support probe is used only when the installed CLI passes it; otherwise the run starts without the text, and the
//! session records why.

use std::fs;
use std::path::Path;

use crate::kind::KindDecl;
pub use crate::probe::PROBE_TIMEOUT;

/// The file in a session's run directory that holds why its current run got no instructions.
const WITHHELD_FILE: &str = "instructions-withheld";
/// The arguments of the help probe that a permission mode's launch value is chosen from (decision 0164).
const HELP_ARGS: [&str; 1] = ["--help"];

/// Whether `program`, the kind's resolved program, takes the kind's instruction channel: always without a declared
/// probe; else when the probe exits 0 within [`PROBE_TIMEOUT`] and its output contains the declared text. The probe runs
/// with the names an account command strips removed and no account home, and its result is kept per program path and
/// modified time. A kind without a channel takes none.
pub(crate) fn channel_supported(decl: &KindDecl, program: &Path) -> bool {
    let Some(channel) = &decl.instructions else {
        return false;
    };
    let Some(probe) = &channel.supported_when else {
        return true;
    };
    let output = crate::probe::cached_probe(decl, program, &probe.args);
    output.exit == Some(0) && output.output.contains(&probe.contains)
}

/// The `--help` text of `program`, which a permission mode's launch value is chosen from (decision 0164). It is the same
/// cached probe the instruction channel runs, so one run serves both.
pub(crate) fn help_output(decl: &KindDecl, program: &Path) -> Option<String> {
    let output = crate::probe::cached_probe(decl, program, &HELP_ARGS.map(str::to_owned));
    (output.exit == Some(0)).then_some(output.output)
}

/// Whether the help text lists `value` among the choices of the option `flag` (decision 0164). The option's own line and
/// the lines it wraps onto, up to the next option or blank line, count; another option's choices do not. A word counts
/// only as a whole, so `manual` is not listed by `manually`.
pub(crate) fn lists_choice(help: &str, flag: &str, value: &str) -> bool {
    let mut inside = false;
    for line in help.lines() {
        if !inside {
            if !names_option(line, flag) {
                continue;
            }
            inside = true;
        } else if line.trim().is_empty() || line.trim_start().starts_with('-') {
            return false;
        }
        if has_word(line, value) {
            return true;
        }
    }
    false
}

/// Whether the names at the start of a help line include `flag`, as in `-p, --permission-mode <MODE>`.
fn names_option(line: &str, flag: &str) -> bool {
    line.trim_start()
        .split(|character: char| {
            character.is_whitespace() || matches!(character, ',' | '=' | '<' | '[' | '>' | ']')
        })
        .filter(|token| !token.is_empty())
        .take_while(|token| token.starts_with('-'))
        .any(|token| token == flag)
}

/// Whether `word` occurs in `text` with no letter, digit, `-` or `_` on either side.
fn has_word(text: &str, word: &str) -> bool {
    let joins = |character: char| character.is_alphanumeric() || matches!(character, '-' | '_');
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(joins) && !after.is_some_and(joins)
    })
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
    fn a_help_line_lists_a_choice_of_its_own_option_as_a_whole_word() {
        let help = "  --permission-mode <MODE>  Permission mode (choices: \"acceptEdits\", \"manual\")\n  --model <M>  Model\n";
        assert!(lists_choice(help, "--permission-mode", "manual"));
        assert!(lists_choice(help, "--permission-mode", "acceptEdits"));
        assert!(!lists_choice(help, "--permission-mode", "default"));
        assert!(!lists_choice(help, "--model", "manual"));
    }

    #[test]
    fn a_choice_on_a_wrapped_line_counts_and_a_longer_word_does_not() {
        let wrapped = "  -p, --permission-mode <MODE>  Permission mode\n      (choices: \"manually\", \"manual\")\n  -v, --verbose  Verbose\n";
        assert!(lists_choice(wrapped, "--permission-mode", "manual"));
        let longer = "  --permission-mode <MODE>  Permission mode (choices: \"manually\")\n";
        assert!(!lists_choice(longer, "--permission-mode", "manual"));
    }

    #[test]
    fn a_choice_after_the_option_block_does_not_count() {
        let help = "  --permission-mode <MODE>  Permission mode\n\n  --other  Lists \"manual\"\n";
        assert!(!lists_choice(help, "--permission-mode", "manual"));
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
