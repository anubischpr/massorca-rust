//! How a kind's declared permission mode presents itself: its menu name and icon, and whether the start page offers it.
//! `agent.kinds` does not carry icons or selectability on `PermissionMode` yet, so one by-id table supplies them here.
//! When the agents session puts `icon`, `tone` and `selectable` on the wire, read them in `presentation` and drop the table.

use massorca_protocol::methods::agents::PermissionMode;
use massorca_ui::IconName;

/// Icon, selectability and name by mode id (decision 0083). An id not listed gets the shield, is offered, and keeps its
/// declared name. `dontAsk` is never offered on the start page; a session that runs it still shows it.
const BY_ID: &[(&str, IconName, bool, Option<&str>)] = &[
    ("auto", IconName::WandSparkles, true, None),
    ("default", IconName::User, true, None),
    ("acceptEdits", IconName::Pencil, true, Some("Accept edits")),
    ("plan", IconName::ListChecks, true, None),
    ("dontAsk", IconName::Ban, false, None),
    ("bypassPermissions", IconName::TriangleAlert, true, None),
    ("accept-edits", IconName::Pencil, true, None),
    ("always-proceed", IconName::TriangleAlert, true, None),
    ("read-only", IconName::Eye, true, None),
    ("read-only-never", IconName::Lock, true, None),
    ("auto-review", IconName::Shield, true, None),
    ("full-access", IconName::TriangleAlert, true, None),
    ("normal", IconName::User, true, None),
    ("yolo", IconName::Zap, true, None),
    ("super-yolo", IconName::TriangleAlert, true, None),
];

/// How a declared mode presents itself in a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModePresentation {
    pub icon: IconName,
    /// Whether the start page offers the mode in its menu.
    pub selectable: bool,
}

fn table_row(id: &str) -> Option<(IconName, bool, Option<&'static str>)> {
    BY_ID
        .iter()
        .find(|(listed, ..)| *listed == id)
        .map(|(_, icon, selectable, name)| (*icon, *selectable, *name))
}

/// The icon and selectability of a declared mode. A bypass mode always carries the warning icon.
pub fn presentation(mode: &PermissionMode) -> ModePresentation {
    let (icon, selectable, _) = table_row(&mode.id).unwrap_or((IconName::Shield, true, None));
    ModePresentation {
        icon: if mode.bypass {
            IconName::TriangleAlert
        } else {
            icon
        },
        selectable,
    }
}

/// The name a declared mode shows: the owner's name from the table where it sets one, else the kind's label.
pub fn mode_name(mode: &PermissionMode) -> String {
    table_row(&mode.id)
        .and_then(|(_, _, name)| name)
        .map_or_else(|| mode.label.clone(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(id: &str, label: &str, bypass: bool) -> PermissionMode {
        PermissionMode {
            id: id.into(),
            label: label.into(),
            description: None,
            bypass,
            live_switch: false,
            may_fall_back: false,
            icon: None,
            tone: None,
            selectable: true,
        }
    }

    #[test]
    fn dont_ask_is_listed_but_not_selectable() {
        assert!(!presentation(&mode("dontAsk", "Don't ask", false)).selectable);
        assert!(presentation(&mode("plan", "Plan", false)).selectable);
    }

    #[test]
    fn every_listed_claude_mode_has_its_own_icon() {
        let ids = ["auto", "default", "acceptEdits", "plan", "dontAsk"];
        let icons: Vec<IconName> = ids
            .iter()
            .map(|id| presentation(&mode(id, id, false)).icon)
            .collect();
        for (index, icon) in icons.iter().enumerate() {
            assert!(
                !icons[..index].contains(icon),
                "{} repeats an icon",
                ids[index]
            );
        }
    }

    #[test]
    fn a_bypass_mode_takes_the_warning_icon_whatever_its_id() {
        let bypass = presentation(&mode("bypassPermissions", "Bypass permissions", true));
        assert_eq!(bypass.icon, IconName::TriangleAlert);
        assert_eq!(
            presentation(&mode("full-access", "Full", true)).icon,
            IconName::TriangleAlert
        );
    }

    #[test]
    fn accept_edits_takes_the_owners_name_whatever_the_kind_calls_it() {
        assert_eq!(
            mode_name(&mode("acceptEdits", "Edit automatically", false)),
            "Accept edits"
        );
        assert_eq!(mode_name(&mode("plan", "Plan", false)), "Plan");
    }

    #[test]
    fn an_unlisted_mode_keeps_its_declared_name_and_is_offered() {
        let unknown = mode("brand-new", "Brand new", false);
        assert_eq!(mode_name(&unknown), "Brand new");
        assert!(presentation(&unknown).selectable);
    }
}
