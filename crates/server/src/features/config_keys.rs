//! The core settings the desktop's Settings page shows (decision 0070). Defaults marked assumed there are not in the design.

use massorca_core_agents::{DELEGATION_DEPTH_DEFAULT, DELEGATION_DEPTH_MAX, DELEGATION_DEPTH_MIN};
use serde_json::{Value, json};

use crate::owner::Owner;
use crate::settings::{SettingDecl, SettingKind};

const STATUS_BAR_ITEMS: &[&str] = &["agent_status", "memory", "terminals", "ports", "hosts"];

/// Old names of canonical keys, as `(canonical key, old name)`. Values stored under an old name still count until the
/// canonical key is written.
const LEGACY_NAMES: &[(&str, &str)] = &[("agents.default_kind", "agents.default")];

/// Every core setting except `workspaces.default_root`, which `config` declares with its own home lookup.
/// `conversations_root` is the absolute default for `conversations.folder`.
pub(crate) fn core_settings(conversations_root: String) -> Vec<SettingDecl> {
    use SettingKind::{Bool, Choice, Colour, Keybindings, Path, Range, Set, String};
    [
        (
            "conversations.folder",
            Path,
            json!(conversations_root),
            "Folder where conversations without a project are saved.",
        ),
        (
            "updates.channel",
            Choice(&["stable", "preview"]),
            json!("stable"),
            "Release channel for updates.",
        ),
        (
            "agents.default_kind",
            String,
            json!("claude"),
            "Agent a new session starts with.",
        ),
        (
            "agents.delegation_depth",
            Range {
                min: DELEGATION_DEPTH_MIN,
                max: DELEGATION_DEPTH_MAX,
            },
            json!(DELEGATION_DEPTH_DEFAULT),
            "How many levels deep agents may delegate.",
        ),
        (
            "agents.computer_use",
            Bool,
            json!(false),
            "Let agents control the computer.",
        ),
        (
            "agents.browser_access",
            Bool,
            json!(false),
            "Let agents use the browser.",
        ),
        (
            "agents.keep_awake",
            Bool,
            json!(false),
            "Keep the computer awake while agents work.",
        ),
        (
            "workspaces.rename_from_first_prompt",
            Bool,
            json!(true),
            "Rename a generated workspace after its first agent prompt, from a title the agent writes.",
        ),
        (
            "usage.enabled",
            Bool,
            json!(false),
            "Record and show agent usage and limits for this computer and its hosts.",
        ),
        (
            "sessions.search",
            Bool,
            json!(true),
            "Index sessions so they can be searched.",
        ),
        (
            "appearance.theme",
            Choice(&["system", "dark", "light"]),
            json!("system"),
            "App theme. System follows the operating system.",
        ),
        (
            "appearance.accent",
            Colour(&["lavender", "blue", "teal", "rose", "orange", "graphite"]),
            json!("lavender"),
            "Accent colour: a preset or a custom #rrggbb seed.",
        ),
        (
            "appearance.density",
            Choice(&["compact", "default", "comfortable"]),
            json!("default"),
            "Spacing of the interface.",
        ),
        (
            "appearance.text_size",
            Range { min: 90, max: 130 },
            json!(100),
            "Text size, in percent.",
        ),
        (
            "appearance.font.interface",
            String,
            json!("Geist"),
            "Interface font.",
        ),
        (
            "appearance.font.monospace",
            String,
            json!("JetBrains Mono"),
            "Monospace font for code and paths.",
        ),
        (
            "appearance.session_summary",
            Bool,
            json!(true),
            "Show the session summary cards that have information.",
        ),
        (
            "appearance.status_bar",
            Set(STATUS_BAR_ITEMS),
            json!(STATUS_BAR_ITEMS),
            "Items shown in the status bar.",
        ),
        (
            "input.keybindings",
            Keybindings,
            json!({}),
            "Shortcut overrides by action id; null unbinds an action.",
        ),
        (
            "editor.tab_width",
            Range { min: 1, max: 16 },
            json!(4),
            "Tab width in the editor.",
        ),
        (
            "terminal.font",
            String,
            json!("JetBrains Mono"),
            "Terminal font.",
        ),
        (
            "terminal.scrollback",
            Range {
                min: 100,
                max: 100_000,
            },
            json!(1000),
            "Rows of scrollback a terminal keeps.",
        ),
        (
            "terminal.shell",
            String,
            json!(""),
            "Shell for new terminals; empty uses the host default.",
        ),
        (
            "terminal.theme",
            Choice(&["system", "dark", "light"]),
            json!("system"),
            "Terminal colour palette: system follows appearance.theme; dark and light set it.",
        ),
        (
            "browser.home",
            String,
            json!("about:blank"),
            "Browser home page.",
        ),
        (
            "git.fetch_on_focus",
            Bool,
            json!(false),
            "Fetch when the window gains focus.",
        ),
        (
            "notifications.agent_finished",
            Bool,
            json!(true),
            "Notify when an agent finishes.",
        ),
        (
            "notifications.waiting",
            Bool,
            json!(true),
            "Notify when an agent is waiting for you.",
        ),
        (
            "notifications.routine_ran",
            Bool,
            json!(true),
            "Notify when a routine runs.",
        ),
        (
            "notifications.extension_notices",
            Bool,
            json!(true),
            "Show notices from extensions.",
        ),
        (
            "voice.dictation",
            Bool,
            json!(false),
            "Dictation in the composer.",
        ),
        (
            "privacy.telemetry",
            Bool,
            json!(false),
            "Send telemetry. Off unless the owner turns it on.",
        ),
        (
            "network.proxy",
            String,
            json!(""),
            "Network proxy; empty for none.",
        ),
        (
            "advanced.enabled",
            Bool,
            json!(false),
            "Show developer tools and advanced settings.",
        ),
    ]
    .into_iter()
    .map(
        |(key, kind, default, description): (&str, SettingKind, Value, &str)| SettingDecl {
            owner: Owner::Core,
            key: key.to_owned(),
            kind,
            default,
            description: description.to_owned(),
            aliases: LEGACY_NAMES
                .iter()
                .filter(|(canonical, _)| *canonical == key)
                .map(|(_, old)| (*old).to_owned())
                .collect(),
        },
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde_json::json;

    use super::core_settings;
    use crate::names;

    #[test]
    fn every_default_fits_its_kind_and_every_key_is_unique() {
        let root = std::env::temp_dir()
            .join("Conversations")
            .to_string_lossy()
            .into_owned();
        let settings = core_settings(root);
        assert_eq!(settings.len(), 34);
        let mut keys = HashSet::new();
        for decl in &settings {
            assert!(
                names::is_valid(&decl.key),
                "{} follows the name rule",
                decl.key
            );
            assert!(
                keys.insert(decl.key.clone()),
                "{} is declared once",
                decl.key
            );
            assert_eq!(decl.check(&decl.default), None, "{} default fits", decl.key);
        }
    }

    #[test]
    fn terminal_theme_is_a_choice_that_defaults_to_system() {
        let settings = core_settings(String::new());
        let theme = settings
            .iter()
            .find(|decl| decl.key == "terminal.theme")
            .expect("terminal.theme is declared");
        assert_eq!(theme.default, json!("system"));
        for choice in ["system", "dark", "light"] {
            assert_eq!(theme.check(&json!(choice)), None, "{choice} is accepted");
        }
        assert!(theme.check(&json!("purple")).is_some());
    }
}
