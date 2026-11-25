//! The rows of a session's row menu (A97, D-WS-02), decided from the facts of its row. A row that does not
//! apply is left out, so the menu never offers a row that cannot be chosen.

/// The actions a session row's menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionMenuId {
    Stop,
    CopyPath,
    OpenFolder,
    RevealInWorkPanel,
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
}

/// The rows of a session's menu for `facts`, top to bottom.
pub fn session_menu(facts: SessionMenuFacts) -> Vec<SessionMenuId> {
    let session = facts.session;
    let mut rows = Vec::new();
    if session.is_some_and(|session| session.running) {
        rows.push(SessionMenuId::Stop);
    }
    rows.push(SessionMenuId::CopyPath);
    if facts.local {
        rows.push(SessionMenuId::OpenFolder);
    }
    if session.is_some_and(|session| session.has_terminal) {
        rows.push(SessionMenuId::RevealInWorkPanel);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNING: Option<SessionFacts> = Some(SessionFacts {
        running: true,
        has_terminal: true,
    });
    const ENDED: Option<SessionFacts> = Some(SessionFacts {
        running: false,
        has_terminal: false,
    });

    fn rows(session: Option<SessionFacts>, local: bool) -> Vec<SessionMenuId> {
        session_menu(SessionMenuFacts { session, local })
    }

    #[test]
    fn a_running_session_on_this_pc_offers_stop_copy_open_and_reveal() {
        assert_eq!(
            rows(RUNNING, true),
            [
                SessionMenuId::Stop,
                SessionMenuId::CopyPath,
                SessionMenuId::OpenFolder,
                SessionMenuId::RevealInWorkPanel,
            ]
        );
    }

    #[test]
    fn an_ended_session_has_no_stop_and_no_terminal_to_reveal() {
        assert_eq!(
            rows(ENDED, true),
            [SessionMenuId::CopyPath, SessionMenuId::OpenFolder]
        );
    }

    #[test]
    fn a_session_on_another_host_cannot_open_its_folder_here() {
        assert_eq!(
            rows(RUNNING, false),
            [
                SessionMenuId::Stop,
                SessionMenuId::CopyPath,
                SessionMenuId::RevealInWorkPanel,
            ]
        );
    }

    #[test]
    fn a_placeholder_row_offers_only_the_folder_rows() {
        assert_eq!(
            rows(None, true),
            [SessionMenuId::CopyPath, SessionMenuId::OpenFolder]
        );
        assert_eq!(rows(None, false), [SessionMenuId::CopyPath]);
    }
}
