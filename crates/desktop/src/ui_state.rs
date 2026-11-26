//! The desktop's remembered UI state (SET-02): the theme last applied, the regions left open and the Calendar's Show
//! done switch. It is saved in
//! `desktop/state/ui-state.json` of the profile, so the first frame of a start already uses the saved theme.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use gpui::{App, Global};
use massorca_profile::{Profile, UI_STATE_FILE, open_desktop_state_dir};
use massorca_ui::{ActiveTheme as _, Appearance, Theme};
use serde::{Deserialize, Serialize};

use crate::model::marketplace::HostTarget;
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
    /// The Marketplace host last picked; None until a pick, so the Marketplace opens on This computer.
    pub marketplace_host: Option<HostTarget>,
    /// The explicit permission mode last picked on a start page, by agent kind id; picking the CLI's default clears it.
    pub start_permission_modes: BTreeMap<String, String>,
    /// The Calendar's Show done switch as the viewer last left it; off until it is switched on.
    pub calendar_show_done: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            theme: Appearance::Dark,
            open_regions: RegionState::default().open().to_vec(),
            marketplace_host: None,
            start_permission_modes: BTreeMap::new(),
            calendar_show_done: false,
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

/// The Marketplace host the profile's state last saved, read at start so the pick survives a restart.
pub fn saved_marketplace_host(cx: &App) -> Option<HostTarget> {
    cx.has_global::<UiStateStore>()
        .then(|| cx.global::<UiStateStore>().saved.marketplace_host.clone())
        .flatten()
}

/// The explicit permission mode last picked on a start page for agent kind `kind`, if any.
pub fn saved_start_mode(cx: &App, kind: &str) -> Option<String> {
    cx.has_global::<UiStateStore>()
        .then(|| {
            cx.global::<UiStateStore>()
                .saved
                .start_permission_modes
                .get(kind)
                .cloned()
        })
        .flatten()
}

/// Remembers `mode` as the explicit pick of agent kind `kind` on a start page; `None` clears it. Does nothing when no
/// profile is kept.
pub fn remember_start_mode(cx: &mut App, kind: &str, mode: Option<&str>) {
    let kind = kind.to_owned();
    let mode = mode.map(str::to_owned);
    remember(cx, move |state| match mode {
        Some(mode) => {
            state.start_permission_modes.insert(kind, mode);
        }
        None => {
            state.start_permission_modes.remove(&kind);
        }
    });
}

/// Whether the Calendar's Show done switch was last left on, so the page opens as it was. Off without a kept profile.
pub fn saved_calendar_show_done(cx: &App) -> bool {
    cx.has_global::<UiStateStore>() && cx.global::<UiStateStore>().saved.calendar_show_done
}

/// Remembers whether the Calendar's Show done switch is on for this viewer. Does nothing when no profile is kept.
pub fn remember_calendar_show_done(cx: &mut App, show: bool) {
    remember(cx, move |state| state.calendar_show_done = show);
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
    /// The host's stable key, such as `runtime:vps`; absent until a pick is saved.
    #[serde(
        default,
        deserialize_with = "lenient",
        skip_serializing_if = "Option::is_none"
    )]
    marketplace_host: Option<String>,
    /// The start page's explicit mode pick per kind id; absent while none is picked.
    #[serde(
        default,
        deserialize_with = "lenient",
        skip_serializing_if = "Option::is_none"
    )]
    start_permission_modes: Option<BTreeMap<String, String>>,
    /// The Calendar's Show done switch, written only while it is on.
    #[serde(
        default,
        deserialize_with = "lenient",
        skip_serializing_if = "Option::is_none"
    )]
    calendar_show_done: Option<bool>,
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
        marketplace_host: state.marketplace_host.as_ref().map(HostTarget::key),
        start_permission_modes: (!state.start_permission_modes.is_empty())
            .then(|| state.start_permission_modes.clone()),
        calendar_show_done: state.calendar_show_done.then_some(true),
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
    // An unknown or damaged key is no saved host, so This computer shows.
    let marketplace_host = stored
        .marketplace_host
        .as_deref()
        .and_then(HostTarget::from_key);
    Some(UiState {
        theme,
        open_regions,
        marketplace_host,
        start_permission_modes: stored.start_permission_modes.unwrap_or_default(),
        calendar_show_done: stored.calendar_show_done.unwrap_or(false),
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
            start_permission_modes: BTreeMap::from([("claude".to_owned(), "plan".to_owned())]),
            calendar_show_done: true,
            ..UiState::default()
        };

        save(&profile, &state).expect("saved");

        assert_eq!(load(&profile), state);
    }

    #[test]
    fn a_damaged_start_mode_map_gives_no_pick_and_the_others_still_apply() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());

        write_raw(
            &profile,
            r#"{"version": 1, "theme": "light", "start_permission_modes": "plan"}"#,
        );
        assert_eq!(
            load(&profile),
            UiState {
                theme: Appearance::Light,
                ..UiState::default()
            }
        );
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
                ..UiState::default()
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

    #[test]
    fn a_picked_marketplace_host_is_saved_by_its_stable_key() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        let state = UiState {
            marketplace_host: Some(HostTarget::Paired("vps".to_owned())),
            ..UiState::default()
        };

        save(&profile, &state).expect("saved");

        assert_eq!(load(&profile), state);
        let text = fs::read_to_string(profile.ui_state_path()).expect("the file");
        assert!(text.contains("\"runtime:vps\""), "{text}");
    }

    #[test]
    fn a_missing_or_unknown_marketplace_host_key_saves_no_host_and_the_theme_still_applies() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        assert_eq!(load(&profile).marketplace_host, None, "missing");

        write_raw(
            &profile,
            r#"{"version": 1, "theme": "light", "marketplace_host": "ssh:box"}"#,
        );
        assert_eq!(
            load(&profile),
            UiState {
                theme: Appearance::Light,
                ..UiState::default()
            }
        );

        write_raw(&profile, r#"{"version": 1, "marketplace_host": 7}"#);
        assert_eq!(load(&profile).marketplace_host, None, "wrong type");
    }

    #[gpui::test]
    fn a_picked_marketplace_host_is_saved_and_read_back_at_start(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            install(cx, profile.clone(), UiState::default());
            assert_eq!(saved_marketplace_host(cx), None, "nothing picked yet");
            remember(cx, |state| {
                state.marketplace_host = Some(HostTarget::Paired("vps".to_owned()));
            });
        });

        assert_eq!(
            load(&profile).marketplace_host,
            Some(HostTarget::Paired("vps".to_owned()))
        );
        cx.update(|cx| {
            assert_eq!(
                saved_marketplace_host(cx),
                Some(HostTarget::Paired("vps".to_owned()))
            );
        });
    }

    #[gpui::test]
    fn a_start_mode_pick_is_remembered_per_kind_and_cleared_by_the_default(
        cx: &mut TestAppContext,
    ) {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            install(cx, profile.clone(), UiState::default());
            remember_start_mode(cx, "claude", Some("plan"));
            remember_start_mode(cx, "codex", Some("read-only"));
        });
        assert_eq!(
            load(&profile).start_permission_modes,
            BTreeMap::from([
                ("claude".to_owned(), "plan".to_owned()),
                ("codex".to_owned(), "read-only".to_owned()),
            ])
        );

        cx.update(|cx| remember_start_mode(cx, "claude", None));

        assert_eq!(
            load(&profile).start_permission_modes,
            BTreeMap::from([("codex".to_owned(), "read-only".to_owned())])
        );
    }

    #[test]
    fn the_calendar_show_done_switch_is_saved_and_read_back() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        let state = UiState {
            calendar_show_done: true,
            ..UiState::default()
        };

        save(&profile, &state).expect("saved");

        assert_eq!(load(&profile), state);
        let text = fs::read_to_string(profile.ui_state_path()).expect("the file");
        assert!(text.contains("\"calendar_show_done\": true"), "{text}");
    }

    #[test]
    fn a_switch_left_off_is_not_written_and_an_older_or_damaged_file_reads_it_off() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        save(&profile, &UiState::default()).expect("saved");
        let text = fs::read_to_string(profile.ui_state_path()).expect("the file");
        assert!(!text.contains("calendar_show_done"), "{text}");

        write_raw(&profile, r#"{"version": 1, "theme": "light"}"#);
        assert!(
            !load(&profile).calendar_show_done,
            "a file from before the switch"
        );

        write_raw(
            &profile,
            r#"{"version": 1, "theme": "light", "calendar_show_done": "yes"}"#,
        );
        assert_eq!(
            load(&profile),
            UiState {
                theme: Appearance::Light,
                ..UiState::default()
            }
        );
    }

    #[gpui::test]
    fn the_show_done_switch_is_remembered_for_this_viewer(cx: &mut TestAppContext) {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = profile(temp.path());
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            install(cx, profile.clone(), UiState::default());
            assert!(!saved_calendar_show_done(cx), "off until it is switched on");
            remember_calendar_show_done(cx, true);
            assert!(saved_calendar_show_done(cx));
        });
        assert!(load(&profile).calendar_show_done);

        cx.update(|cx| remember_calendar_show_done(cx, false));

        assert!(!load(&profile).calendar_show_done);
    }
}
