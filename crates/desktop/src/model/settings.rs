//! The Settings page as plain state (D-ST-01 to D-ST-12, D-CP-04): the workspace folder the
//! service holds, what the user has typed since, and the rules a typed folder must pass before it
//! reaches the service. Free of GPUI, so the rules test without a window.

use std::path::Path;

use massorca_protocol::methods::config::ConfigValue;

use super::settings_sections::SettingsSection;

/// The reason the folder row is off while the service is not connected.
pub const NOT_CONNECTED: &str = "Connect to the service to change this.";
/// The reason the folder row is off while the service has not answered yet.
pub const LOADING: &str = "Loading from the service.";

/// The trimmed folder the service may be asked to store, or why it must not be sent.
pub fn folder_request(draft: &str) -> Result<String, &'static str> {
    let path = draft.trim();
    if path.is_empty() {
        return Err("Enter a folder path.");
    }
    if !Path::new(path).is_absolute() {
        return Err("Enter a full folder path.");
    }
    Ok(path.to_owned())
}

/// Why the folder row is off right now, if it is.
pub fn folder_reason(connected: bool, folder: &FolderSetting) -> Option<&'static str> {
    if !connected {
        Some(NOT_CONNECTED)
    } else if folder.stored.is_none() {
        Some(LOADING)
    } else {
        None
    }
}

/// What the folder row says after its last change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderStatus {
    Saving,
    /// The service stored the value.
    Saved,
    /// The service refused it. The message is the words the row shows.
    Failed(String),
}

/// The workspace folder as the service holds it: the path in effect and the default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderValue {
    pub path: String,
    pub default: String,
    pub is_default: bool,
}

impl FolderValue {
    /// The folder a `config.list` or `config.set` answer carries. A value that is not text reads
    /// as empty, which the row then shows as an empty field.
    pub fn from_config(config: &ConfigValue) -> Self {
        Self {
            path: config.value.as_str().unwrap_or_default().to_owned(),
            default: config.default.as_str().unwrap_or_default().to_owned(),
            is_default: config.is_default,
        }
    }
}

/// The workspace folder: the value the service reports, and what the user has typed since.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderSetting {
    /// The value the service last reported or stored. `None` until `config.list` answers.
    pub stored: Option<FolderValue>,
    /// What the text field holds. It differs from the stored value while the user types.
    pub draft: String,
    pub status: Option<FolderStatus>,
}

impl FolderSetting {
    fn stored_path(&self) -> &str {
        self.stored.as_ref().map_or("", |value| value.path.as_str())
    }

    /// The service answered: the value becomes the stored one and the draft follows it.
    pub fn loaded(&mut self, value: FolderValue) {
        self.draft.clone_from(&value.path);
        self.stored = Some(value);
        self.status = None;
    }

    /// The service stored the value. Only now does the row say so.
    pub fn saved(&mut self, value: FolderValue) {
        self.loaded(value);
        self.status = Some(FolderStatus::Saved);
    }

    /// The service refused the draft: the draft stays, the stored value stays in effect.
    pub fn refused(&mut self, message: impl Into<String>) {
        self.status = Some(FolderStatus::Failed(message.into()));
    }

    /// Escape: the draft goes back to the stored value.
    pub fn revert(&mut self) {
        self.draft = self.stored_path().to_owned();
        self.status = None;
    }

    /// Whether the draft differs from what the service holds.
    pub fn dirty(&self) -> bool {
        self.draft.trim() != self.stored_path()
    }

    /// Whether the stored value differs from the default, so the row offers a reset.
    pub fn modified(&self) -> bool {
        self.stored.as_ref().is_some_and(|value| !value.is_default)
    }

    /// The default folder, which the reset returns to.
    pub fn default_path(&self) -> &str {
        self.stored
            .as_ref()
            .map_or("", |value| value.default.as_str())
    }
}

/// Everything the Settings page keeps between renders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsModel {
    pub section: SettingsSection,
    /// The search field's text; a non-empty query shows the hits across every section.
    pub query: String,
    pub folder: FolderSetting,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = "/home/user/massorca/workspaces";

    fn value(path: &str, is_default: bool) -> FolderValue {
        FolderValue {
            path: path.to_owned(),
            default: DEFAULT.to_owned(),
            is_default,
        }
    }

    #[test]
    fn a_folder_is_trimmed_and_must_be_a_full_path() {
        let absolute = if cfg!(windows) {
            r"  D:\projects\massorca  "
        } else {
            "  /srv/massorca  "
        };
        assert_eq!(folder_request(absolute), Ok(absolute.trim().to_owned()));
        assert_eq!(folder_request("   "), Err("Enter a folder path."));
        assert_eq!(
            folder_request("workspaces"),
            Err("Enter a full folder path.")
        );
    }

    #[test]
    fn the_folder_row_is_off_until_the_service_answers_and_while_disconnected() {
        let mut folder = FolderSetting::default();
        assert_eq!(folder_reason(true, &folder), Some(LOADING));
        assert_eq!(folder_reason(false, &folder), Some(NOT_CONNECTED));
        folder.loaded(value("/srv/a", true));
        assert_eq!(folder_reason(true, &folder), None);
    }

    #[test]
    fn a_loaded_value_fills_the_draft_and_only_a_saved_one_says_saved() {
        let mut folder = FolderSetting::default();
        folder.loaded(value("/srv/a", true));
        assert_eq!(folder.draft, "/srv/a");
        assert!(!folder.modified(), "the default is in effect");
        folder.draft = "/srv/b".into();
        assert!(folder.dirty());
        folder.refused("Enter a full folder path.");
        assert_eq!(
            folder.status,
            Some(FolderStatus::Failed("Enter a full folder path.".into()))
        );
        assert_eq!(folder.stored, Some(value("/srv/a", true)));
        folder.saved(value("/srv/b", false));
        assert_eq!(folder.status, Some(FolderStatus::Saved));
        assert!(folder.modified());
        assert!(!folder.dirty());
    }

    #[test]
    fn escape_puts_the_stored_value_back_and_clears_the_status() {
        let mut folder = FolderSetting::default();
        folder.loaded(value("/srv/a", false));
        folder.draft = "/srv/typed".into();
        folder.refused("Enter a full folder path.");
        folder.revert();
        assert_eq!(folder.draft, "/srv/a");
        assert_eq!(folder.status, None);
        assert_eq!(folder.default_path(), DEFAULT);
    }
}
