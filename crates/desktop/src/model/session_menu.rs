//! The rows of a session's row menu (A97, D-WS-02), decided from the facts of its row. A row that does not
//! apply is left out, so the menu never offers a row that cannot be chosen.

/// The actions a session row's menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMenuId {
    Stop,
    CopyPath,
    OpenFolder,
    RevealInWorkPanel,
    Delete,
}

/// One row of the menu: an action, or the divider before the destructive group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMenuRow {
    Item(SessionMenuId),
    Divider,
}

/// What the menu needs to know about the session its row stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionFacts {
    /// The run has not ended, so there is a run to stop.
    pub running: bool,
    /// The session has a terminal the Work panel can show.
    pub has_terminal: bool,
}

/// What the menu is decided from: the session, when the row has one, and where its folder is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionMenuFacts {
    /// None for a workspace with no session yet, which has a placeholder row.
    pub session: Option<SessionFacts>,
    /// Whether the folder is on This PC. A folder on another host cannot be opened from here.
    pub local: bool,
    /// Whether the service offers `agent-archive/1`, which forgets a session.
    pub delete_offered: bool,
}

/// The rows of a session's menu for `facts`, top to bottom.
pub fn session_menu(facts: SessionMenuFacts) -> Vec<SessionMenuRow> {
    let session = facts.session;
    let mut rows = Vec::new();
    if session.is_some_and(|session| session.running) {
        rows.push(SessionMenuRow::Item(SessionMenuId::Stop));
    }
    rows.push(SessionMenuRow::Item(SessionMenuId::CopyPath));
    if facts.local {
        rows.push(SessionMenuRow::Item(SessionMenuId::OpenFolder));
    }
    if session.is_some_and(|session| session.has_terminal) {
        rows.push(SessionMenuRow::Item(SessionMenuId::RevealInWorkPanel));
    }
    // The service forgets only a session whose run has ended, so Delete waits for that.
    if facts.delete_offered && session.is_some_and(|session| !session.running) {
        rows.push(SessionMenuRow::Divider);
        rows.push(SessionMenuRow::Item(SessionMenuId::Delete));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::SessionMenuId::{CopyPath, Delete, OpenFolder, RevealInWorkPanel, Stop};
    use super::*;

    const RUNNING: Option<SessionFacts> = Some(SessionFacts {
        running: true,
        has_terminal: true,
    });
    const ENDED: Option<SessionFacts> = Some(SessionFacts {
        running: false,
        has_terminal: false,
    });

    fn facts(session: Option<SessionFacts>, local: bool, delete_offered: bool) -> SessionMenuFacts {
        SessionMenuFacts {
            session,
            local,
            delete_offered,
        }
    }

    fn item(id: SessionMenuId) -> SessionMenuRow {
        SessionMenuRow::Item(id)
    }

    #[test]
    fn a_running_session_on_this_pc_offers_stop_copy_open_and_reveal() {
        assert_eq!(
            session_menu(facts(RUNNING, true, false)),
            [
                item(Stop),
                item(CopyPath),
                item(OpenFolder),
                item(RevealInWorkPanel),
            ]
        );
    }

    #[test]
    fn an_ended_session_has_no_stop_and_no_terminal_to_reveal() {
        assert_eq!(
            session_menu(facts(ENDED, true, false)),
            [item(CopyPath), item(OpenFolder)]
        );
    }

    #[test]
    fn a_session_on_another_host_cannot_open_its_folder_here() {
        assert_eq!(
            session_menu(facts(RUNNING, false, false)),
            [item(Stop), item(CopyPath), item(RevealInWorkPanel)]
        );
    }

    #[test]
    fn a_placeholder_row_offers_only_the_folder_rows() {
        assert_eq!(
            session_menu(facts(None, true, false)),
            [item(CopyPath), item(OpenFolder)]
        );
        assert_eq!(session_menu(facts(None, false, false)), [item(CopyPath)]);
    }

    #[test]
    fn an_ended_session_offers_delete_after_a_divider_when_the_service_offers_it() {
        assert_eq!(
            session_menu(facts(ENDED, true, true)),
            [
                item(CopyPath),
                item(OpenFolder),
                SessionMenuRow::Divider,
                item(Delete),
            ]
        );
    }

    #[test]
    fn a_running_session_offers_no_delete_until_its_run_ends() {
        assert!(!session_menu(facts(RUNNING, true, true)).contains(&item(Delete)));
    }

    #[test]
    fn delete_needs_the_capability_and_a_session() {
        assert!(!session_menu(facts(ENDED, true, false)).contains(&item(Delete)));
        assert!(!session_menu(facts(None, true, true)).contains(&item(Delete)));
    }
}
