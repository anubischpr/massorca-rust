//! The desktop's remembered UI state (SET-02): the theme last applied and the regions left open. It is saved in
//! `desktop/state/ui-state.json` of the profile, so the first frame of a start already uses the saved theme.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use gpui::{App, Global};
use massorca_profile::{Profile, UI_STATE_FILE, open_desktop_state_dir};
use massorca_ui::{ActiveTheme as _, Appearance, Theme};
use serde::{Deserialize, Serialize};

use crate::model::regions::{Region, RegionState};

/// The format version this build reads and writes; any other version gives the defaults.
const VERSION: u8 = 1;
/// A file larger than this is not read.
const MAX_BYTES: u64 = 64 * 1024;

/// What the desktop remembers between starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiState {
    pub theme: Appearance,
    pub open_regions: Vec<Region>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            theme: Appearance::Dark,
            open_regions: RegionState::default().open().to_vec(),
        }
    }
}

/// The profile whose state is kept, and what was last saved there, so a change is written only when it differs.
struct UiStateStore {
    profile: Profile,
    saved: UiState,
}

impl Global for UiStateStore {}

/// Keeps the state of `profile` from now on. `loaded` is what the profile held at start; each later theme change
/// is saved as it happens.
pub fn install(cx: &mut App, profile: Profile, loaded: UiState) {
    cx.set_global(UiStateStore {
        profile,
        saved: loaded,
    });
    cx.observe_global::<Theme>(|cx| {
        let appearance = cx.theme().appearance;
        remember(cx, |state| state.theme = appearance);
    })
    .detach();
}

/// Applies `change` to the kept state and saves it when that changes it. Does nothing when no profile is kept.
pub fn remember(cx: &mut App, change: impl FnOnce(&mut UiState)) {
    if !cx.has_global::<UiStateStore>() {
        return;
    }
    let store = cx.global_mut::<UiStateStore>();
    let mut next = store.saved.clone();
    change(&mut next);
    if next == store.saved {
        return;
    }
    store.saved = next.clone();
    // A failed save leaves the file as it was; the next change saves again.
    let _ = save(&store.profile, &next);
}

/// The profile's remembered state. A missing, unreadable, oversized or damaged file gives the defaults.
pub fn load(profile: &Profile) -> UiState {
    read_bounded(&profile.ui_state_path())
        .and_then(|text| decode(&text))
        .unwrap_or_default()
}

fn save(profile: &Profile, state: &UiState) -> io::Result<()> {
    let folder = open_desktop_state_dir(profile).map_err(io::Error::other)?;
    folder.write_file(UI_STATE_FILE, encode(state).as_bytes())
}

fn read_bounded(path: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(MAX_BYTES + 1)
        .read_to_string(&mut text)
        .ok()?;
    (u64::try_from(text.len()).ok()? <= MAX_BYTES).then_some(text)
}

/// The file as written: only the fixed words of the theme and regions, so nothing needs escaping.
#[derive(Serialize, Deserialize)]
struct Stored {
    version: u8,
    #[serde(default, deserialize_with = "lenient")]
    theme: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    open_regions: Option<Vec<serde_json::Value>>,
}

/// A member of the wrong type reads as missing, so the others still apply.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

fn encode(state: &UiState) -> String {
    let stored = Stored {
        version: VERSION,
        theme: Some(appearance_key(state.theme).to_owned()),
        open_regions: Some(
            state
                .open_regions
                .iter()
                .map(|region| serde_json::Value::from(region_key(*region)))
                .collect(),
        ),
    };
    let mut text = serde_json::to_string_pretty(&stored).unwrap_or_default();
    text.push('\n');
    text
}

/// The state a text describes, or `None` when it is not JSON of this version. A member that is missing or unknown
/// keeps its default.
fn decode(text: &str) -> Option<UiState> {
    let stored: Stored = serde_json::from_str(text).ok()?;
    if stored.version != VERSION {
        return None;
    }
    let defaults = UiState::default();
    let theme = stored
        .theme
        .as_deref()
        .and_then(appearance_from_key)
        .unwrap_or(defaults.theme);
    let open_regions = match stored.open_regions {
        Some(items) => {
            let mut open = Vec::new();
            for region in items
                .iter()
                .filter_map(|item| item.as_str().and_then(region_from_key))
            {
                if !open.contains(&region) {
                    open.push(region);
                }
            }
            open
        }
        None => defaults.open_regions,
    };
    Some(UiState {
        theme,
        open_regions,
    })
}

fn appearance_key(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    }
}

fn appearance_from_key(key: &str) -> Option<Appearance> {
    [Appearance::Dark, Appearance::Light]
        .into_iter()
        .find(|appearance| appearance_key(*appearance) == key)
}

fn region_key(region: Region) -> &'static str {
    match region {
        Region::SessionSummary => "session_summary",
        Region::ProjectSidebar => "project_sidebar",
        Region::Sidebar => "sidebar",
        Region::WorkPanel => "work_panel",
    }
}

fn region_from_key(key: &str) -> Option<Region> {
    Region::COLLAPSE_ORDER
        .into_iter()
        .find(|region| region_key(*region) == key)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use gpui::TestAppContext;

    use super::*;

    fn profile(home: &Path) -> Profile {
        Profile::resolve_with(Some(home.to_path_buf()), Some("main".to_owned()), |_| None)
            .expect("a profile")
    }

    fn write_raw(profile: &Profile, text: &str) {
        fs::create_dir_all(profile.desktop_state_dir()).expect("the state folder");
        fs::write(profile.ui_state_path(), text).expect("the file");
    }

    #[test]
    fn a_saved_state_reads_back_as_it_was_saved() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        let state = UiState {
            theme: Appearance::Light,
            open_regions: vec![Region::WorkPanel, Region::Sidebar],
        };

        save(&profile, &state).expect("saved");

        assert_eq!(load(&profile), state);
    }

    #[test]
    fn a_missing_or_damaged_file_gives_the_defaults() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        assert_eq!(load(&profile), UiState::default(), "missing");

        for text in [
            "",
            "{",
            "not json",
            "[]",
            "{\"theme\":\"light\"}",
            "{\"version\":2,\"theme\":\"light\"}",
            "{\"version\":1,\"theme\":7}",
        ] {
            write_raw(&profile, text);
            assert_eq!(load(&profile), UiState::default(), "{text:?}");
        }
    }

    #[test]
    fn a_damaged_member_keeps_its_default_and_the_others_still_apply() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());

        write_raw(
            &profile,
            r#"{"version": 1, "theme": "light", "open_regions": "sidebar"}"#,
        );
        assert_eq!(
            load(&profile),
            UiState {
                theme: Appearance::Light,
                open_regions: UiState::default().open_regions,
            }
        );

        write_raw(
            &profile,
            r#"{"version": 1, "open_regions": ["work_panel", "nope", "work_panel"]}"#,
        );
        assert_eq!(load(&profile).open_regions, vec![Region::WorkPanel]);
    }

    #[test]
    fn the_first_frame_uses_the_theme_saved_last() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        let state = UiState {
            theme: Appearance::Light,
            ..UiState::default()
        };
        save(&profile, &state).expect("saved");

        assert_eq!(load(&profile).theme, Appearance::Light);
    }

    #[gpui::test]
    fn a_theme_change_is_saved_as_it_happens(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            install(cx, profile.clone(), UiState::default());
        });

        cx.update(Theme::toggle);

        assert_eq!(load(&profile).theme, Appearance::Light);
    }

    #[gpui::test]
    fn only_a_real_change_is_written(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            install(cx, profile.clone(), UiState::default());
            remember(cx, |state| state.theme = Appearance::Dark);
        });
        assert!(!profile.ui_state_path().exists(), "nothing changed");

        cx.update(|cx| remember(cx, |state| state.open_regions = vec![Region::WorkPanel]));

        assert_eq!(load(&profile).open_regions, vec![Region::WorkPanel]);
    }
}
