//! A kind's permission modes as the composer's mode chip offers them (decision 0083). The start page offers the kind's
//! declared default first, then its other selectable modes in declared order; a session's chip shows the mode it runs
//! in, whether that is only requested, and a mode waiting for the next resume. Free of GPUI, so the rules test without
//! a window.

use massorca_protocol::methods::agents::{AgentKind, AgentSession, ModeSource, PermissionMode};
use massorca_ui::IconName;

use super::mode_presentation::{mode_name, presentation};

/// The entry for no chosen mode: no flag is passed and the vendor CLI starts in its own default.
pub const CLI_DEFAULT: &str = "The CLI's default";
/// The note on a chosen mode nothing confirmed the agent took.
pub const REQUESTED: &str = "Requested";
/// The tag on the start page row of the kind's declared default mode.
pub const DEFAULT_TAG: &str = "Default";

/// One entry of the mode chip. `id` is `None` for the entry that sends no flag: on the start page that is the kind's
/// declared default mode, and a session in the vendor's default shows the same entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeChoice {
    pub id: Option<String>,
    pub name: String,
    /// The mode lifts the vendor's permission prompts or sandbox; the chip marks it with a warning.
    pub bypass: bool,
    /// Secondary text for the entry: the declared description on the start page, and the chip's tooltip.
    pub detail: Option<String>,
    /// The menu icon of the mode (`mode_presentation`).
    pub icon: IconName,
    /// The kind's declared default mode: its start page row carries the Default tag.
    pub default: bool,
}

/// What a session's chip shows: its entries, the selected one, and the caption after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionModes {
    pub choices: Vec<ModeChoice>,
    pub selected: usize,
    pub note: Option<String>,
}

fn declared_default(kind: &AgentKind) -> Option<&PermissionMode> {
    let id = kind.default_permission_mode.as_deref()?;
    kind.permission_modes.iter().find(|mode| mode.id == id)
}

/// A declared mode as an entry that sends its own flag.
fn declared_entry(mode: &PermissionMode) -> ModeChoice {
    ModeChoice {
        id: Some(mode.id.clone()),
        name: mode_name(mode),
        bypass: mode.bypass,
        detail: None,
        icon: presentation(mode).icon,
        default: false,
    }
}

/// The entry for the CLI's own default when the kind declares no default it can name: no flag and no tag.
fn unnamed_default() -> ModeChoice {
    ModeChoice {
        id: None,
        name: CLI_DEFAULT.to_owned(),
        bypass: false,
        detail: None,
        icon: IconName::Shield,
        default: false,
    }
}

/// The session chip's entry for a session in the vendor's default: named after the declared default it runs in.
fn cli_default(kind: &AgentKind) -> ModeChoice {
    match declared_default(kind) {
        Some(mode) => ModeChoice {
            id: None,
            name: format!("{CLI_DEFAULT} ({})", mode_name(mode)),
            bypass: false,
            detail: None,
            icon: presentation(mode).icon,
            default: false,
        },
        None => unnamed_default(),
    }
}

/// The start page's entries: the declared default first, then each other selectable mode in declared order. The
/// default's row sends no flag, since the vendor CLI starts in that mode by itself; it carries the mode's name, its
/// description and the Default tag. Each mode row carries its declared description. Empty when the kind declares no
/// modes, which hides the chip.
pub fn start_choices(kind: &AgentKind) -> Vec<ModeChoice> {
    if kind.permission_modes.is_empty() {
        return Vec::new();
    }
    let default = declared_default(kind).filter(|mode| presentation(mode).selectable);
    let mut choices = vec![match default {
        Some(mode) => ModeChoice {
            id: None,
            name: mode_name(mode),
            bypass: mode.bypass,
            detail: mode.description.clone(),
            icon: presentation(mode).icon,
            default: true,
        },
        None => unnamed_default(),
    }];
    choices.extend(
        kind.permission_modes
            .iter()
            .filter(|mode| presentation(mode).selectable)
            .filter(|mode| default.is_none_or(|default| default.id != mode.id))
            .map(|mode| ModeChoice {
                detail: mode.description.clone(),
                ..declared_entry(mode)
            }),
    );
    choices
}

/// The explicit mode the person last picked for a kind, if the kind still declares it; a pick it no longer declares
/// is ignored.
pub fn remembered_mode<'a>(kind: &AgentKind, remembered: Option<&'a str>) -> Option<&'a str> {
    remembered.filter(|id| kind.permission_modes.iter().any(|mode| mode.id == *id))
}

/// A session's chip, or `None` when its kind declares no modes. A session in the vendor's default selects the CLI's
/// default entry, never the declared default; a mode the kind no longer lists keeps its own entry.
pub fn session_modes(kind: &AgentKind, session: &AgentSession) -> Option<SessionModes> {
    if kind.permission_modes.is_empty() {
        return None;
    }
    let mut choices: Vec<ModeChoice> = kind.permission_modes.iter().map(declared_entry).collect();
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
                        icon: IconName::Shield,
                        default: false,
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
    let pending = session.pending_permission_mode.as_deref().map(|id| {
        let name = kind
            .permission_modes
            .iter()
            .find(|mode| mode.id == id)
            .map_or_else(|| id.to_owned(), mode_name);
        format!("Next resume: {name}")
    });
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
    use massorca_protocol::methods::agents::PackageState;

    fn mode(id: &str, label: &str, bypass: bool) -> PermissionMode {
        PermissionMode {
            id: id.into(),
            label: label.into(),
            description: Some(format!("{label} description")),
            bypass,
            live_switch: false,
            may_fall_back: false,
            icon: None,
            tone: None,
            selectable: true,
        }
    }

    /// Claude's declaration as `claude.json` lists it: the declared default is Auto, and Don't ask is not selectable.
    fn claude() -> AgentKind {
        let mut kind = agent_kind("claude", "Claude", PackageState::Enabled);
        kind.permission_modes = vec![
            mode("default", "Manual", false),
            mode("acceptEdits", "Edit automatically", false),
            mode("plan", "Plan", false),
            mode("auto", "Auto", false),
            mode("dontAsk", "Don't ask", false),
            mode("bypassPermissions", "Bypass permissions", true),
        ];
        kind.default_permission_mode = Some("auto".into());
        kind
    }

    fn names(choices: &[ModeChoice]) -> Vec<&str> {
        choices.iter().map(|choice| choice.name.as_str()).collect()
    }

    #[test]
    fn the_start_menu_lists_the_declared_default_first_then_the_other_selectable_modes() {
        let choices = start_choices(&claude());
        assert_eq!(
            names(&choices),
            [
                "Auto",
                "Manual",
                "Accept edits",
                "Plan",
                "Bypass permissions"
            ]
        );
        assert_eq!(
            choices
                .iter()
                .map(|choice| choice.id.clone())
                .collect::<Vec<_>>(),
            [
                None,
                Some("default".to_owned()),
                Some("acceptEdits".to_owned()),
                Some("plan".to_owned()),
                Some("bypassPermissions".to_owned()),
            ],
            "the default row sends no flag; each other row sends its own"
        );
    }

    #[test]
    fn the_default_row_is_the_declared_default_tagged_and_no_mode_is_listed_twice() {
        let choices = start_choices(&claude());
        assert_eq!(
            choices
                .iter()
                .map(|choice| choice.default)
                .collect::<Vec<_>>(),
            [true, false, false, false, false]
        );
        assert_eq!(
            choices
                .iter()
                .filter(|choice| choice.name == "Auto")
                .count(),
            1
        );
        assert_eq!(choices[0].detail.as_deref(), Some("Auto description"));
        assert_eq!(choices[1].detail.as_deref(), Some("Manual description"));
    }

    #[test]
    fn dont_ask_is_never_offered_and_bypass_is_the_only_warning() {
        let choices = start_choices(&claude());
        assert!(choices.iter().all(|choice| choice.name != "Don't ask"));
        assert_eq!(
            choices
                .iter()
                .map(|choice| choice.bypass)
                .collect::<Vec<_>>(),
            [false, false, false, false, true]
        );
    }

    #[test]
    fn each_offered_row_has_its_own_icon_and_the_bypass_row_the_warning_icon() {
        let choices = start_choices(&claude());
        let icons: Vec<IconName> = choices.iter().map(|choice| choice.icon).collect();
        assert_eq!(icons[4], IconName::TriangleAlert);
        for (index, icon) in icons[..4].iter().enumerate() {
            assert!(
                !icons[..index].contains(icon),
                "row {index} repeats an icon"
            );
            assert_ne!(*icon, IconName::TriangleAlert);
        }
    }

    #[test]
    fn a_kind_without_a_declared_default_starts_on_the_clis_default_untagged() {
        let mut kind = claude();
        kind.default_permission_mode = None;
        let choices = start_choices(&kind);
        assert_eq!(names(&choices)[0], CLI_DEFAULT);
        assert_eq!(choices[0].id, None);
        assert!(!choices[0].default);
        assert_eq!(
            choices.len(),
            6,
            "the CLI's default, then five selectable modes; dont ask is left out"
        );
    }

    #[test]
    fn codex_leads_with_its_declared_auto_preset() {
        let mut kind = agent_kind("codex", "Codex", PackageState::Enabled);
        kind.permission_modes = vec![
            mode("auto", "Auto", false),
            mode("read-only", "Safe read-only browsing", false),
            mode("full-access", "Dangerous full access", true),
        ];
        kind.default_permission_mode = Some("auto".into());
        let choices = start_choices(&kind);
        assert_eq!(
            names(&choices),
            ["Auto", "Safe read-only browsing", "Dangerous full access"]
        );
        assert!(choices[0].default);
    }

    #[test]
    fn a_remembered_pick_counts_only_while_the_kind_still_declares_it() {
        let kind = claude();
        assert_eq!(remembered_mode(&kind, Some("plan")), Some("plan"));
        assert_eq!(
            remembered_mode(&kind, Some("acceptEdits")),
            Some("acceptEdits")
        );
        assert_eq!(remembered_mode(&kind, Some("gone")), None);
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
        assert_eq!(
            mode_to_set(&modes, &session, 2).as_deref(),
            Some("acceptEdits")
        );
    }

    #[test]
    fn a_session_that_runs_dont_ask_still_shows_it() {
        let mut session = agent("s1", "w1", 1);
        session.permission_mode = Some("dontAsk".into());
        session.mode_source = Some(ModeSource::Chosen);
        session.mode_confirmed = true;
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(modes.choices[modes.selected].name, "Don't ask");
        assert!(modes.choices.iter().all(|choice| choice.detail.is_none()));
    }

    #[test]
    fn an_unconfirmed_mode_says_requested_and_a_pending_one_names_the_next_resume() {
        let mut session = agent("s1", "w1", 1);
        session.permission_mode = Some("plan".into());
        session.mode_source = Some(ModeSource::Chosen);
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(modes.choices[modes.selected].name, "Plan");
        assert_eq!(modes.note.as_deref(), Some(REQUESTED));
        assert_eq!(mode_to_set(&modes, &session, 2), None);

        session.mode_confirmed = true;
        session.pending_permission_mode = Some("bypassPermissions".into());
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(
            modes.note.as_deref(),
            Some("Next resume: Bypass permissions")
        );
        assert_eq!(mode_to_set(&modes, &session, 2).as_deref(), Some("plan"));
    }

    #[test]
    fn a_mode_the_kind_no_longer_lists_keeps_its_own_entry() {
        let mut session = agent("s1", "w1", 1);
        session.permission_mode = Some("retired".into());
        session.mode_source = Some(ModeSource::Chosen);
        session.mode_confirmed = true;
        let modes = session_modes(&claude(), &session).expect("claude declares modes");
        assert_eq!(modes.selected, 0);
        assert_eq!(modes.choices[0].name, "retired");
    }
}
