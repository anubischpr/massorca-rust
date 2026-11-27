//! Views: view documents placed in a UI slot. The view-schema major is declared on the manifest (`views`).

use serde::{Deserialize, Serialize};

use super::Manifest;
use super::check_path;
use super::contributes::{list, required};
use crate::error::{Problems, at};
use crate::ids;

/// A view document placed in a UI slot.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct View {
    pub id: String,
    pub slot: Slot,
    pub title: String,
    pub document: String,
}

/// The UI slots of `extensions.md` §7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Slot {
    ProjectSidebarTab,
    SidebarSection,
    WorkPanelTab,
    CanvasPage,
    SessionSummaryCard,
    ConversationCard,
    PaletteCommand,
    ContextMenu,
    ComposerAction,
    ModelPickerEntry,
    RailIcon,
    StatusItem,
    SettingsSection,
    Notification,
}

/// Checks the views: each id is unique, and each has a title and a safe document.
pub(crate) fn validate(manifest: &Manifest, problems: &mut Problems) {
    list(
        &manifest.contributes.views,
        "views",
        "id",
        ids::is_contribution_id,
        |item| item.id.as_str(),
        problems,
        |item, here, problems| {
            required(&item.title, &at(here, "title"), problems);
            check_path(&item.document, &at(here, "document"), problems);
        },
    );
}
