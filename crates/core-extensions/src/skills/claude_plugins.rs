//! The skill folders of the Claude plugins switched on for the home folder and each registered folder (pv96's
//! `claude-plugin-skill-sources.ts`). Each plugin contributes the one install that applies to the folder.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::roots::{SkillProvider, SkillRoot, SkillSourceKind};
use crate::source::sha256_hex;

/// Plugin metadata larger than this is ignored; pv96 reads at most 4 MiB.
const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;
/// A plugin name shown in a root label is cut to this many characters.
const MAX_LABEL_CHARS: usize = 80;

/// The skill roots of the plugins enabled for each of `contexts`, each root listed once. Claude's own `.claude`
/// folder under `home` holds the installs and the user settings.
pub(super) fn claude_plugin_roots(home: &Path, contexts: &[PathBuf]) -> Vec<SkillRoot> {
    let installed = read_json(&home.join(".claude/plugins/installed_plugins.json"));
    let Some(plugins) = installed
        .as_ref()
        .and_then(|file| file.get("plugins"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let user_settings = read_json(&home.join(".claude/settings.json"));
    let mut seen = BTreeSet::new();
    let mut roots = Vec::new();
    for context in contexts {
        let project = read_json(&context.join(".claude/settings.json"));
        let local = read_json(&context.join(".claude/settings.local.json"));
        let settings = [user_settings.as_ref(), project.as_ref(), local.as_ref()];
        for (plugin_id, installs) in plugins {
            if enabled(&settings, plugin_id) != Some(true) {
                continue;
            }
            let Some(install_path) = active_install(installs, context) else {
                continue;
            };
            let skills = install_path.join("skills");
            if seen.insert(skills.clone()) {
                roots.push(plugin_root(plugin_id, skills));
            }
        }
    }
    roots
}

/// The last `enabledPlugins` value for the plugin across the settings files, which Claude merges in order.
fn enabled(settings: &[Option<&Value>], plugin_id: &str) -> Option<bool> {
    settings
        .iter()
        .flatten()
        .filter_map(|file| file.get("enabledPlugins")?.get(plugin_id)?.as_bool())
        .next_back()
}

/// The install of a plugin that applies to `context`: a local install over a project one over a user one, then the
/// most specific project, then the latest update. ISO timestamps compare correctly as text.
fn active_install(installs: &Value, context: &Path) -> Option<PathBuf> {
    installs
        .as_array()?
        .iter()
        .filter_map(|install| {
            let scope = install.get("scope")?.as_str()?;
            let rank = match scope {
                "local" => 2,
                "project" => 1,
                "user" => 0,
                _ => return None,
            };
            let install_path = PathBuf::from(install.get("installPath")?.as_str()?);
            let project = install
                .get("projectPath")
                .and_then(Value::as_str)
                .map(PathBuf::from);
            let applies = scope == "user"
                || project
                    .as_deref()
                    .is_some_and(|project| project.is_absolute() && context.starts_with(project));
            if !install_path.is_absolute() || !applies {
                return None;
            }
            let specificity = project.map_or(0, |project| project.as_os_str().len());
            let updated = install
                .get("lastUpdated")
                .or_else(|| install.get("installedAt"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Some(((rank, specificity, updated), install_path))
        })
        .reduce(|best, next| if next.0 > best.0 { next } else { best })
        .map(|(_, install_path)| install_path)
}

fn plugin_root(plugin_id: &str, skills: PathBuf) -> SkillRoot {
    SkillRoot {
        id: format!(
            "claude-plugin-{}",
            &sha256_hex(skills.to_string_lossy().as_bytes())[..16]
        ),
        label: format!("Claude plugin {}", safe_label(plugin_id)),
        path: skills,
        kind: SkillSourceKind::Plugin,
        providers: vec![SkillProvider::Claude],
        owner: Some("claude"),
    }
}

/// The package name of a plugin id (`name@market`), with the characters that could spoof a label removed.
fn safe_label(plugin_id: &str) -> String {
    let package = plugin_id
        .split('@')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| plugin_id.rsplit('/').next().unwrap_or(plugin_id));
    let label: String = package
        .chars()
        .filter(|&character| is_display_safe(character))
        .take(MAX_LABEL_CHARS)
        .collect();
    if label.is_empty() {
        "plugin".to_owned()
    } else {
        label
    }
}

/// Controls, zero-width characters and bidi overrides can hide or reorder a label (pv96's display filter).
fn is_display_safe(character: char) -> bool {
    !matches!(
        u32::from(character),
        0x00..=0x1f
            | 0x7f..=0x9f
            | 0x061c
            | 0x200b
            | 0x200e
            | 0x200f
            | 0x2028
            | 0x2029
            | 0x202a..=0x202e
            | 0x2060
            | 0x2066..=0x2069
            | 0xfeff
    )
}

/// A JSON file read from `path`, unless it is missing, not a plain file, too large or not JSON.
fn read_json(path: &Path) -> Option<Value> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_METADATA_BYTES {
        return None;
    }
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
