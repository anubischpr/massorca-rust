//! Terminals as the work panel and the status bar show them: the store's records turned into
//! the state words, the exit text and the tab titles (decision 0033; terminals.md).

use massorca_protocol::methods::terminals::{
    ExitReason, ShellSpec, Terminal, TerminalCreateParams, TerminalState,
};

use crate::service::StoreState;

/// A terminal's state in the shared vocabulary: `live`, `unverifiable` or `exited` (terminals.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalStatus {
    Live,
    /// The service cannot prove the process either way. Never shown as dead.
    Unverifiable,
    Exited {
        reason: ExitReason,
        code: Option<i64>,
    },
}

/// The status of a terminal. A state this build does not know is `unverifiable`: it may still run.
pub fn status_of(terminal: &Terminal) -> TerminalStatus {
    match terminal.state {
        TerminalState::Live => TerminalStatus::Live,
        TerminalState::Exited => {
            let (reason, code) = terminal
                .exit
                .as_ref()
                .map_or((ExitReason::Unknown, None), |exit| (exit.reason, exit.code));
            TerminalStatus::Exited { reason, code }
        }
        TerminalState::Unverifiable | TerminalState::Unknown => TerminalStatus::Unverifiable,
    }
}

/// The state word of the status, as the header shows it.
pub fn state_word(status: TerminalStatus) -> &'static str {
    match status {
        TerminalStatus::Live => "live",
        TerminalStatus::Unverifiable => "unverifiable",
        TerminalStatus::Exited { .. } => "exited",
    }
}

/// Whether the process may still run, which is every status but `exited`.
pub fn may_run(status: TerminalStatus) -> bool {
    !matches!(status, TerminalStatus::Exited { .. })
}

/// The row an exited terminal shows (TerminalSurface.md, exited state).
pub fn exit_text(status: TerminalStatus) -> Option<String> {
    let TerminalStatus::Exited { reason, code } = status else {
        return None;
    };
    Some(match (reason, code) {
        (ExitReason::Killed, _) => "Process was killed".to_owned(),
        (ExitReason::ServiceStopped, _) => "The service stopped and ended this terminal".to_owned(),
        (ExitReason::Exited | ExitReason::Unknown, Some(0)) => "Process finished".to_owned(),
        (ExitReason::Exited | ExitReason::Unknown, Some(code)) => {
            format!("Process exited with code {code}")
        }
        (ExitReason::Exited | ExitReason::Unknown, None) => "Process exited".to_owned(),
    })
}

/// The banner of an unverifiable terminal. It says the process may still run, never that it ended.
pub fn unverifiable_text() -> &'static str {
    "Can't reach this terminal's host. The process may still be running."
}

/// The tab title: the program's own title, else the shell and the folder ("zsh, oppm").
pub fn tab_title(terminal: &Terminal) -> String {
    if let Some(title) = terminal.title.as_deref().filter(|title| !title.is_empty()) {
        return title.to_owned();
    }
    let shell = last_segment(&terminal.shell);
    match last_segment(&terminal.cwd) {
        "" => shell.to_owned(),
        folder => format!("{shell}, {folder}"),
    }
}

/// The part of a path after its last separator, for Windows and POSIX paths alike.
fn last_segment(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
}

/// The terminals of one workspace, in the order the store keeps them (oldest first).
pub fn workspace_terminals<'a>(state: &'a StoreState, workspace_id: &str) -> Vec<&'a Terminal> {
    state
        .terminals
        .iter()
        .filter(|terminal| terminal.workspace_id == workspace_id)
        .collect()
}

/// The status bar's count: terminals that may still run, in any workspace.
pub fn running_count(state: &StoreState) -> u32 {
    let count = state
        .terminals
        .iter()
        .filter(|terminal| may_run(status_of(terminal)))
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The program the `terminal.shell` setting names for new terminals; `None` (the host's default shell) when it is
/// empty or not text. The whole value is the program, so a path with spaces stays one.
pub fn shell_setting(value: Option<&serde_json::Value>) -> Option<ShellSpec> {
    let program = value?.as_str()?.trim();
    (!program.is_empty()).then(|| ShellSpec {
        program: program.to_owned(),
        args: Vec::new(),
    })
}

/// Parameters for a new terminal in a workspace at the workspace root (0032): `shell`, or the host's default shell.
pub fn new_terminal_params(
    workspace_id: &str,
    cols: u16,
    rows: u16,
    shell: Option<ShellSpec>,
) -> TerminalCreateParams {
    TerminalCreateParams {
        workspace: workspace_id.to_owned(),
        shell,
        cwd: None,
        env: Default::default(),
        cols: Some(cols),
        rows: Some(rows),
        title: None,
        request_id: None,
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::terminals::TerminalExit;

    use super::*;
    use crate::service::StoreUpdate;

    #[test]
    fn the_shell_setting_names_the_whole_program_and_empty_keeps_the_host_default() {
        let spaced = serde_json::json!(r"C:\Program Files\PowerShell\7\pwsh.exe");
        assert_eq!(
            shell_setting(Some(&spaced)).map(|spec| spec.program),
            Some(r"C:\Program Files\PowerShell\7\pwsh.exe".to_owned())
        );
        for empty in [
            serde_json::json!(""),
            serde_json::json!("  "),
            serde_json::json!(7),
        ] {
            assert_eq!(shell_setting(Some(&empty)), None, "{empty}");
        }
        assert_eq!(shell_setting(None), None);
    }

    fn terminal(id: &str, workspace: &str, state: TerminalState) -> Terminal {
        Terminal {
            id: id.to_owned(),
            workspace_id: workspace.to_owned(),
            state,
            cwd: "/repos/oppm".to_owned(),
            ..crate::service::fixtures::base_terminal()
        }
    }

    fn exited(mut terminal: Terminal, reason: ExitReason, code: Option<i64>) -> Terminal {
        terminal.state = TerminalState::Exited;
        terminal.exit = Some(TerminalExit {
            code,
            reason,
            ..crate::service::fixtures::base_terminal_exit()
        });
        terminal
    }

    fn store_with(terminals: Vec<Terminal>) -> StoreState {
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::TerminalsResync(terminals),
            std::time::Instant::now(),
        );
        state
    }

    #[test]
    fn the_service_states_map_to_the_shared_vocabulary() {
        let live = terminal("a", "w", TerminalState::Live);
        assert_eq!(status_of(&live), TerminalStatus::Live);
        assert_eq!(state_word(status_of(&live)), "live");

        let unverifiable = terminal("b", "w", TerminalState::Unverifiable);
        assert_eq!(state_word(status_of(&unverifiable)), "unverifiable");

        let unknown = terminal("c", "w", TerminalState::Unknown);
        assert_eq!(status_of(&unknown), TerminalStatus::Unverifiable);

        let done = exited(
            terminal("d", "w", TerminalState::Exited),
            ExitReason::Exited,
            Some(1),
        );
        assert_eq!(
            status_of(&done),
            TerminalStatus::Exited {
                reason: ExitReason::Exited,
                code: Some(1)
            }
        );
        assert_eq!(state_word(status_of(&done)), "exited");
    }

    #[test]
    fn an_exited_terminal_says_how_it_ended_and_never_says_unverifiable_is_dead() {
        let status = |code, reason| TerminalStatus::Exited { reason, code };
        assert_eq!(
            exit_text(status(Some(1), ExitReason::Exited)).as_deref(),
            Some("Process exited with code 1")
        );
        assert_eq!(
            exit_text(status(Some(0), ExitReason::Exited)).as_deref(),
            Some("Process finished")
        );
        assert_eq!(
            exit_text(status(None, ExitReason::Killed)).as_deref(),
            Some("Process was killed")
        );
        assert_eq!(exit_text(TerminalStatus::Live), None);
        assert!(may_run(TerminalStatus::Unverifiable));
        assert!(!unverifiable_text().to_lowercase().contains("exited"));
        assert!(unverifiable_text().contains("may still be running"));
    }

    #[test]
    fn a_tab_is_named_by_its_title_or_by_shell_and_folder() {
        let plain = terminal("a", "w", TerminalState::Live);
        assert_eq!(tab_title(&plain), "zsh, oppm");

        let mut titled = plain.clone();
        titled.title = Some("cargo test".to_owned());
        assert_eq!(tab_title(&titled), "cargo test");

        let mut windows = plain;
        windows.shell = r"C:\Program Files\PowerShell\7\pwsh.exe".to_owned();
        windows.cwd = r"D:\projects\oppm\".to_owned();
        assert_eq!(tab_title(&windows), "pwsh.exe, oppm");
    }

    #[test]
    fn a_workspace_lists_only_its_terminals_in_store_order() {
        let state = store_with(vec![
            terminal("t1", "w1", TerminalState::Live),
            terminal("t2", "w2", TerminalState::Live),
            terminal("t3", "w1", TerminalState::Live),
        ]);
        let ids: Vec<&str> = workspace_terminals(&state, "w1")
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(ids, vec!["t1", "t3"]);
    }

    #[test]
    fn the_status_bar_counts_terminals_that_may_still_run() {
        let state = store_with(vec![
            terminal("live", "w", TerminalState::Live),
            terminal("lost", "w", TerminalState::Unverifiable),
            exited(
                terminal("done", "w", TerminalState::Exited),
                ExitReason::Killed,
                None,
            ),
        ]);
        assert_eq!(running_count(&state), 2);
        assert_eq!(running_count(&StoreState::default()), 0);
    }

    #[test]
    fn a_new_terminal_uses_the_default_shell_in_the_workspace() {
        let params = new_terminal_params("w9", 120, 40, None);
        assert_eq!(params.workspace, "w9");
        assert!(params.shell.is_none());
        assert_eq!((params.cols, params.rows), (Some(120), Some(40)));
    }
}
