//! The Work panel's terminal tabs as plain data (WorkPanelTabs.md, D-SH-17): the tabs a workspace's
//! strip lists, the tab selected in it, what closing a tab asks, and what a pane says about its
//! terminal. Free of GPUI, so the rules test without a window.

use std::collections::BTreeMap;

use super::terminals::{
    TerminalStatus, may_run, status_of, tab_title, unverifiable_text, workspace_terminals,
};
use crate::service::StoreState;

/// The size a new terminal starts at until its pane reports the size it has (TERM-01).
pub const NEW_TERMINAL_COLS: u16 = 120;
pub const NEW_TERMINAL_ROWS: u16 = 40;

/// The grid a new Work panel terminal opens at, from the first source that has one: the grid its pane fits now
/// (`measured`), else the grid the last Work panel pane applied, else the fixed size.
pub fn new_terminal_grid(measured: Option<(u16, u16)>, applied: Option<(u16, u16)>) -> (u16, u16) {
    measured
        .or(applied)
        .unwrap_or((NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS))
}

/// Why the kinds of the empty state that have no layer yet are off (A43).
pub const BROWSER_LATER: &str = "Browser tabs come later.";
pub const MARKDOWN_LATER: &str = "Markdown tabs come later.";
pub const MOBILE_LATER: &str = "Mobile emulators come later.";

/// Why "New terminal" is off, if it is: no workspace to open it in, or no service to ask.
pub fn terminal_unavailable(has_workspace: bool, connected: bool) -> Option<&'static str> {
    if !has_workspace {
        Some("Choose a workspace first.")
    } else if !connected {
        Some("The service is not connected.")
    } else {
        None
    }
}

/// The terminal each workspace's strip has selected. Window state only: the store holds the
/// terminals themselves (decision 0033).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalSelection {
    by_workspace: BTreeMap<String, String>,
}

impl TerminalSelection {
    pub fn select(&mut self, workspace: &str, terminal: &str) {
        self.by_workspace
            .insert(workspace.to_owned(), terminal.to_owned());
    }

    /// Drops every selection of a terminal that is gone.
    pub fn forget(&mut self, terminal: &str) {
        self.by_workspace.retain(|_, selected| selected != terminal);
    }

    pub fn selected(&self, workspace: &str) -> Option<&str> {
        self.by_workspace.get(workspace).map(String::as_str)
    }
}

/// One tab of a workspace's strip, as the store and the selection give it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalTab {
    pub id: String,
    pub title: String,
    pub status: TerminalStatus,
}

/// A workspace's strip: its terminals in store order, and the index of the selected one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalStrip {
    pub tabs: Vec<TerminalTab>,
    /// The chosen terminal while it is still listed, else the first; `None` without tabs.
    pub selected: Option<usize>,
}

/// The strip of `workspace`, from the terminals the store holds.
pub fn terminal_strip(
    state: &StoreState,
    workspace: &str,
    selection: &TerminalSelection,
) -> TerminalStrip {
    let tabs: Vec<TerminalTab> = workspace_terminals(state, workspace)
        .into_iter()
        .map(|terminal| TerminalTab {
            id: terminal.id.clone(),
            title: tab_title(terminal),
            status: status_of(terminal),
        })
        .collect();
    let chosen = selection
        .selected(workspace)
        .and_then(|id| tabs.iter().position(|tab| tab.id == id));
    let selected = chosen.or((!tabs.is_empty()).then_some(0));
    TerminalStrip { tabs, selected }
}

/// What closing a tab does: a terminal that has ended is forgotten at once, one that may still run
/// asks first (X-DLG-01, D-TM-04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStep {
    Forget,
    AskWhetherToStop,
}

pub fn close_step(status: TerminalStatus) -> CloseStep {
    if may_run(status) {
        CloseStep::AskWhetherToStop
    } else {
        CloseStep::Forget
    }
}

/// The banner a pane shows for its status: an unverifiable terminal may still run, and the banner
/// never says it ended (terminals.md).
pub fn status_banner(status: TerminalStatus) -> Option<&'static str> {
    matches!(status, TerminalStatus::Unverifiable).then(unverifiable_text)
}

/// What a selected tab without a pane says: its terminal runs on in the service, or it has ended.
pub fn detached_text(status: TerminalStatus) -> &'static str {
    if may_run(status) {
        "This terminal keeps running in the background."
    } else {
        "This terminal has exited."
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::terminals::{
        ExitReason, Terminal, TerminalExit, TerminalState,
    };

    use super::*;

    #[test]
    fn a_new_terminal_opens_at_the_grid_its_pane_fits_else_the_last_applied_else_the_fixed_size() {
        assert_eq!(
            new_terminal_grid(Some((100, 30)), Some((80, 24))),
            (100, 30)
        );
        assert_eq!(new_terminal_grid(None, Some((80, 24))), (80, 24));
        assert_eq!(
            new_terminal_grid(None, None),
            (NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS)
        );
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

    fn state_with(terminals: Vec<Terminal>) -> StoreState {
        StoreState {
            terminals,
            ..StoreState::default()
        }
    }

    fn two_in_w1() -> StoreState {
        state_with(vec![
            terminal("t1", "w1", TerminalState::Live),
            terminal("t2", "w2", TerminalState::Live),
            terminal("t3", "w1", TerminalState::Unverifiable),
        ])
    }

    fn ended(code: Option<i64>, reason: ExitReason) -> TerminalStatus {
        TerminalStatus::Exited { reason, code }
    }

    #[test]
    fn a_strip_lists_only_its_workspace_terminals_in_store_order() {
        let strip = terminal_strip(&two_in_w1(), "w1", &TerminalSelection::default());
        let ids: Vec<&str> = strip.tabs.iter().map(|tab| tab.id.as_str()).collect();
        assert_eq!(ids, vec!["t1", "t3"]);
        assert_eq!(strip.tabs[0].title, "zsh, oppm");
        assert_eq!(strip.tabs[1].status, TerminalStatus::Unverifiable);
        assert_eq!(
            strip.selected,
            Some(0),
            "without a choice the first tab is selected"
        );
    }

    #[test]
    fn the_chosen_terminal_is_selected_while_it_is_listed() {
        let mut selection = TerminalSelection::default();
        selection.select("w1", "t3");
        let strip = terminal_strip(&two_in_w1(), "w1", &selection);
        assert_eq!(strip.selected, Some(1));
        let other = terminal_strip(&two_in_w1(), "w2", &selection);
        assert_eq!(other.selected, Some(0), "w2 has its own strip");
    }

    #[test]
    fn a_chosen_terminal_that_is_gone_falls_back_to_the_first_tab() {
        let mut selection = TerminalSelection::default();
        selection.select("w1", "gone");
        let strip = terminal_strip(&two_in_w1(), "w1", &selection);
        assert_eq!(strip.selected, Some(0));
    }

    #[test]
    fn a_workspace_without_terminals_has_no_selection() {
        let strip = terminal_strip(&StoreState::default(), "w1", &TerminalSelection::default());
        assert!(strip.tabs.is_empty());
        assert_eq!(strip.selected, None);
    }

    #[test]
    fn forgetting_a_terminal_drops_only_the_selections_of_it() {
        let mut selection = TerminalSelection::default();
        selection.select("w1", "t1");
        selection.select("w2", "t2");
        selection.forget("t1");
        assert_eq!(selection.selected("w1"), None);
        assert_eq!(selection.selected("w2"), Some("t2"));
    }

    #[test]
    fn closing_a_tab_asks_only_when_its_process_may_still_run() {
        assert_eq!(
            close_step(TerminalStatus::Live),
            CloseStep::AskWhetherToStop
        );
        assert_eq!(
            close_step(TerminalStatus::Unverifiable),
            CloseStep::AskWhetherToStop
        );
        assert_eq!(
            close_step(ended(Some(0), ExitReason::Exited)),
            CloseStep::Forget
        );
        assert_eq!(
            close_step(ended(None, ExitReason::Killed)),
            CloseStep::Forget
        );
    }

    #[test]
    fn an_unverifiable_pane_says_it_may_still_run_and_never_that_it_ended() {
        let banner = status_banner(TerminalStatus::Unverifiable).expect("a banner");
        assert!(banner.contains("may still be running"));
        assert!(!banner.to_lowercase().contains("exited"));
        assert_eq!(status_banner(TerminalStatus::Live), None);
        assert_eq!(status_banner(ended(Some(1), ExitReason::Exited)), None);
    }

    #[test]
    fn a_detached_tab_says_whether_its_terminal_still_runs() {
        assert!(detached_text(TerminalStatus::Live).contains("keeps running"));
        assert!(detached_text(ended(None, ExitReason::Killed)).contains("has exited"));
    }

    #[test]
    fn a_new_terminal_needs_a_workspace_and_a_connection() {
        assert_eq!(
            terminal_unavailable(false, true),
            Some("Choose a workspace first.")
        );
        assert_eq!(
            terminal_unavailable(true, false),
            Some("The service is not connected.")
        );
        assert_eq!(terminal_unavailable(true, true), None);
    }

    #[test]
    fn an_exit_record_does_not_change_which_tab_is_listed() {
        let mut done = terminal("t9", "w1", TerminalState::Exited);
        done.exit = Some(TerminalExit {
            code: Some(0),
            ..crate::service::fixtures::base_terminal_exit()
        });
        let state = state_with(vec![done]);
        let strip = terminal_strip(&state, "w1", &TerminalSelection::default());
        assert_eq!(strip.tabs[0].status, ended(Some(0), ExitReason::Exited));
    }
}
