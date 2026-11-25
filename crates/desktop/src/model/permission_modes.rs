//! A kind's permission modes as the composer's mode chip offers them (decision 0083). The start page offers the CLI's
//! default and each declared mode; a session's chip shows the mode it runs in, whether that is only requested, and a
//! mode waiting for the next resume. Free of GPUI, so the rules test without a window.

use massorca_protocol::methods::agents::{AgentKind, AgentSession, ModeSource};

/// The entry for no chosen mode: no flag is passed and the vendor CLI starts in its own default.
pub const CLI_DEFAULT: &str = "The CLI's default";
/// The note on a chosen mode nothing confirmed the agent took.
pub const REQUESTED: &str = "Requested";

/// One entry of the mode chip. `id` is `None` for the CLI's default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeChoice {
    pub id: Option<String>,
    pub name: String,
    /// The mode lifts the vendor's permission prompts or sandbox; the chip marks it with a warning.
    pub bypass: bool,
    /// Secondary text for the entry: its menu row's second line and the chip's tooltip.
    pub detail: Option<String>,
}

/// What a session's chip shows: its entries, the selected one, and the caption after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionModes {
    pub choices: Vec<ModeChoice>,
    pub selected: usize,
    pub note: Option<String>,
}

/// The CLI's default entry, named after the declared vendor default when the kind lists it.
fn cli_default(kind: &AgentKind) -> ModeChoice {
    let name = kind
        .default_permission_mode
        .as_deref()
        .and_then(|id| mode_label(kind, id))
        .map_or_else(
            || CLI_DEFAULT.to_owned(),
            |label| format!("{CLI_DEFAULT} ({label})"),
        );
    ModeChoice {
        id: None,
        name,
        bypass: false,
        detail: None,
    }
}

/// The start page's CLI's default entry: named after the declared default it runs in, with "The CLI's default" as its
/// detail, so a declared mode with the same name is told apart. Without a declared default it is named the CLI's default.
fn start_default(kind: &AgentKind) -> ModeChoice {
    match kind
        .default_permission_mode
        .as_deref()
        .and_then(|id| mode_label(kind, id))
    {
        Some(label) => ModeChoice {
            id: None,
            name: label.to_owned(),
            bypass: false,
            detail: Some(CLI_DEFAULT.to_owned()),
        },
        None => ModeChoice {
            id: None,
            name: CLI_DEFAULT.to_owned(),
            bypass: false,
            detail: None,
        },
    }
}

fn declared(kind: &AgentKind) -> impl Iterator<Item = ModeChoice> + '_ {
    kind.permission_modes.iter().map(|mode| ModeChoice {
        id: Some(mode.id.clone()),
        name: mode.label.clone(),
        bypass: mode.bypass,
        detail: None,
    })
}

/// The label a kind gives mode `id`, if it declares it.
fn mode_label<'a>(kind: &'a AgentKind, id: &str) -> Option<&'a str> {
    kind.permission_modes
        .iter()
        .find(|mode| mode.id == id)
        .map(|mode| mode.label.as_str())
}

/// The start page's entries: the CLI's default first, which is selected until the person picks a mode, then each
/// declared mode in order. Empty when the kind declares none, which hides the chip.
pub fn start_choices(kind: &AgentKind) -> Vec<ModeChoice> {
    if kind.permission_modes.is_empty() {
        return Vec::new();
    }
    // The start sends no mode for this entry; it names the declared default it runs in, and its detail says so.
    std::iter::once(start_default(kind))
        .chain(declared(kind))
        .collect()
}

/// The explicit mode the person last picked for a kind, if the kind still declares it; a pick it no longer declares
/// is ignored.
pub fn remembered_mode<'a>(kind: &AgentKind, remembered: Option<&'a str>) -> Option<&'a str> {
    remembered.filter(|id| mode_label(kind, id).is_some())
}

/// A session's chip, or `None` when its kind declares no modes. A session in the vendor's default selects the CLI's
/// default entry, never the declared default; a mode the kind no longer lists keeps its own entry.
pub fn session_modes(kind: &AgentKind, session: &AgentSession) -> Option<SessionModes> {
    if kind.permission_modes.is_empty() {
        return None;
    }
    let mut choices: Vec<ModeChoice> = declared(kind).collect();
    let selected = match session.permission_mode.as_deref() {
        Some(id) => match choices
            .iter()
            .position(|choice| choice.id.as_deref() == Some(id))
        {
            Some(index) => index,
            None => {
                choices.insert(
                    0,
                    ModeChoice {
                        id: Some(id.to_owned()),
                        name: id.to_owned(),
                        bypass: false,
                        detail: None,
                    },
                );
                0
            }
        },
        None => {
            choices.insert(0, cli_default(kind));
            0
        }
    };
    let requested = session.mode_source == Some(ModeSource::Chosen) && !session.mode_confirmed;
    let pending = session
        .pending_permission_mode
        .as_deref()
        .map(|id| format!("Next resume: {}", mode_label(kind, id).unwrap_or(id)));
    let note = match (requested, pending) {
        (true, Some(pending)) => Some(format!("{REQUESTED} · {pending}")),
        (true, None) => Some(REQUESTED.to_owned()),
        (false, pending) => pending,
    };
    Some(SessionModes {
        choices,
        selected,
        note,
    })
}

/// The mode `agent.set_mode` should get when the person picks entry `index`, or `None` when nothing changes: the
/// CLI's default (which `agent.set_mode` cannot name), or the mode already running with none pending.
pub fn mode_to_set(modes: &SessionModes, session: &AgentSession, index: usize) -> Option<String> {
    let id = modes.choices.get(index)?.id.clone()?;
    let running = session.permission_mode.as_deref() == Some(id.as_str());
    (!running || session.pending_permission_mode.is_some()).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::{agent, agent_kind};
    use massorca_protocol::methods::agents::{PackageState, PermissionMode};

    fn mode(id: &str, label: &str, bypass: bool) -> PermissionMode {
        PermissionMode {
            id: id.into(),
            label: label.into(),
            description: None,
            bypass,
            live_switch: false,
            may_fall_back: false,
        }
    }

    fn claude() -> AgentKind {
        let mut kind = agent_kind("claude", "Claude", PackageState::Enabled);
        kind.permission_modes = vec![
            mode("default", "Manual", false),
            mode("plan", "Plan", false),
            mode("auto", "Auto", false),
            mode("bypassPermissions", "Bypass permissions", true),
        ];
        kind.default_permission_mode = Some("auto".into());
        kind
    }

    fn names(choices: &[ModeChoice]) -> Vec<&str> {
        choices.iter().map(|choice| choice.name.as_str()).collect()
    }

    #[test]
    fn the_start_chip_opens_on_the_clis_default_named_after_the_declared_default() {
        let choices = start_choices(&claude());
        assert_eq!(
            names(&choices),
            ["Auto", "Manual", "Plan", "Auto", "Bypass permissions"]
        );
        assert_eq!(choices[0].id, None, "the default entry sends no mode");
        assert_eq!(choices[0].detail.as_deref(), Some("The CLI's default"));
        assert!(choices[1..].iter().all(|choice| choice.detail.is_none()));
        assert!(choices[4].bypass);
        assert!(choices[..4].iter().all(|choice| !choice.bypass));
    }

    #[test]
    fn a_declared_mode_with_the_default_label_is_told_apart_by_its_detail() {
        let choices = start_choices(&claude());
        assert_eq!(choices[0].name, choices[3].name);
        assert_ne!(choices[0].detail, choices[3].detail);
        assert_eq!(choices[3].id.as_deref(), Some("auto"));
    }

    #[test]
    fn a_kind_without_a_declared_default_names_its_entry_the_clis_default() {
        let mut kind = claude();
        kind.default_permission_mode = None;
        let choices = start_choices(&kind);
        assert_eq!(names(&choices)[0], "The CLI's default");
        assert_eq!(choices[0].detail, None, "the name already says it");
    }

    #[test]
    fn codex_names_its_default_entry_after_its_declared_auto_preset() {
        let mut kind = agent_kind("codex", "Codex", PackageState::Enabled);
        kind.permission_modes = vec![
            mode("auto", "Auto", false),
            mode("full-access", "Dangerous full access", true),
        ];
        kind.default_permission_mode = Some("auto".into());
        let choices = start_choices(&kind);
        assert_eq!(names(&choices)[0], "Auto");
        assert_eq!(choices[0].detail.as_deref(), Some("The CLI's default"));
    }

    #[test]
    fn a_remembered_pick_counts_only_while_the_kind_still_declares_it() {
        let kind = claude();
        assert_eq!(remembered_mode(&kind, Some("plan")), Some("plan"));
        assert_eq!(remembered_mode(&kind, Some("acceptEdits")), None);
        assert_eq!(remembered_mode(&kind, None), None);
    }

    #[test]
    fn a_kind_without_modes_has_no_chip() {
        let kind = agent_kind("codex", "Codex", PackageState::Enabled);
        assert!(start_choices(&kind).is_empty());
        assert_eq!(session_modes(&kind, &agent("s1", "w1", 1)), None);
    }

    #[test]
    fn a_session_in_the_vendors_default_shows_the_clis_default_not_the_declared_one() {
        let mut session = agent("s1", "w1", 1);
        session.mode_source = Some(ModeSource::VendorDefault);
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(
            modes.choices[modes.selected].name,
            "The CLI's default (Auto)"
        );
        assert_eq!(modes.note, None);
        assert_eq!(mode_to_set(&modes, &session, 0), None);
        assert_eq!(mode_to_set(&modes, &session, 2).as_deref(), Some("plan"));
    }

    #[test]
    fn an_unconfirmed_mode_says_requested_and_a_pending_one_names_the_next_resume() {
        let mut session = agent("s1", "w1", 1);
        session.permission_mode = Some("plan".into());
        session.mode_source = Some(ModeSource::Chosen);
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(modes.choices[modes.selected].name, "Plan");
        assert_eq!(modes.note.as_deref(), Some(REQUESTED));
        assert_eq!(mode_to_set(&modes, &session, 1), None);

        session.mode_confirmed = true;
        session.pending_permission_mode = Some("bypassPermissions".into());
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(
            modes.note.as_deref(),
            Some("Next resume: Bypass permissions")
        );
        assert_eq!(mode_to_set(&modes, &session, 1).as_deref(), Some("plan"));
    }

    #[test]
    fn a_mode_the_kind_no_longer_lists_keeps_its_own_entry() {
        let mut session = agent("s1", "w1", 1);
        session.permission_mode = Some("acceptEdits".into());
        session.mode_source = Some(ModeSource::Chosen);
        session.mode_confirmed = true;
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(modes.selected, 0);
        assert_eq!(modes.choices[0].name, "acceptEdits");
    }
}
