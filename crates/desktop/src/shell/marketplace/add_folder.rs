//! The Add extension dialog: a package in a folder on This computer, reviewed like a catalogue install before anything
//! installs. A git URL is listed as not available yet.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, WeakEntity, div, px};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{SPACE_1, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use crate::model::marketplace::HostTarget;
use crate::shell::Shell;
use crate::shell::path_field::{PickKind, path_field};

/// Said when Review is pressed with no folder.
const NO_FOLDER: &str = "Enter the folder that holds the package.";
/// Said for a folder that is not a full path: the service would read a relative one from its own folder.
const NOT_FULL_PATH: &str = "Enter the full path of the folder, such as C:/Users/me/kit.";
/// Said when a paired host is picked: the folder is read on This computer only.
const NOT_THIS_COMPUTER: &str = "Pick This computer to add a package from a folder on it.";

/// The Add extension dialog while it is open: the folder field and the reason to show in it.
#[derive(Debug)]
pub(in crate::shell) struct AddFolderDialog {
    field: Entity<TextField>,
    error: Option<String>,
}

impl AddFolderDialog {
    #[cfg(test)]
    pub(in crate::shell) fn field(&self) -> &Entity<TextField> {
        &self.field
    }
}

/// The `dir:` source of a folder on This computer, or why that folder cannot be reviewed.
fn folder_source(path: &str, on_this_computer: bool) -> Result<String, &'static str> {
    if !on_this_computer {
        return Err(NOT_THIS_COMPUTER);
    }
    let path = path.trim();
    if path.is_empty() {
        return Err(NO_FOLDER);
    }
    if !std::path::Path::new(path).is_absolute() {
        return Err(NOT_FULL_PATH);
    }
    Ok(format!("dir:{path}"))
}

impl Shell {
    /// Opens the Add extension dialog with an empty folder field.
    pub(super) fn open_add_extension(&mut self, cx: &mut Context<Self>) {
        let field = cx.new(|cx| {
            TextField::new(cx)
                .label("Folder")
                .placeholder("Full path of the folder that holds the package")
                .mono()
        });
        self.marketplace_add = Some(AddFolderDialog { field, error: None });
        cx.notify();
    }

    pub(super) fn close_add_extension(&mut self, cx: &mut Context<Self>) {
        self.marketplace_add = None;
        cx.notify();
    }

    /// Reviews the folder the field holds with the catalogue install's review; nothing installs until its button is pressed.
    pub(super) fn submit_add_extension(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.marketplace_add.as_ref() else {
            return;
        };
        let path = dialog.field.read(cx).text().to_owned();
        let on_this_computer = matches!(self.marketplace_pick.target(), HostTarget::Local);
        match folder_source(&path, on_this_computer) {
            Ok(source) => {
                self.marketplace_add = None;
                self.open_folder_review(source, cx);
            }
            Err(reason) => {
                if let Some(dialog) = self.marketplace_add.as_mut() {
                    dialog.error = Some(reason.to_owned());
                }
                cx.notify();
            }
        }
    }
}

/// The Add extension dialog, if it is open.
pub(super) fn dialog(shell: &Shell, weak: &WeakEntity<Shell>, cx: &App) -> Option<AnyElement> {
    let add = shell.marketplace_add.as_ref()?;
    let theme = *cx.theme();
    let (submit, dismiss) = (weak.clone(), weak.clone());
    let mut dialog = Dialog::new("marketplace-add-extension-dialog", "Add extension")
        .open(true)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(
                    "Adds a package from a folder on this computer. Nothing installs until you review it and allow the install.",
                ),
        )
        .child(path_field(
            "marketplace-add-browse",
            "marketplace-add-field",
            weak.clone(),
            &add.field,
            PickKind::Folder,
            matches!(shell.marketplace_pick.target(), HostTarget::Local),
            true,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(theme.palette.text.secondary)
                        .child("From a git URL"),
                )
                .child(
                    Button::new("marketplace-add-git-url", "Install from a git URL")
                        .variant(ButtonVariant::Secondary)
                        .disabled(true)
                        .tooltip("Not available yet"),
                ),
        )
        .action("Review")
        .cancel_label("Cancel")
        .on_action(move |_, cx| {
            let _ = submit.update(cx, |shell, cx| shell.submit_add_extension(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = dismiss.update(cx, |shell, cx| shell.close_add_extension(cx));
        });
    if let Some(error) = &add.error {
        dialog = dialog.error(error.clone());
    }
    Some(dialog.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::folder_source;

    /// A full path on the platform the test runs on.
    fn full_path() -> &'static str {
        if cfg!(windows) {
            "C:/Users/me/kit"
        } else {
            "/Users/me/kit"
        }
    }

    #[test]
    fn a_full_path_on_this_computer_names_the_folder_source() {
        assert_eq!(
            folder_source(full_path(), true),
            Ok(format!("dir:{}", full_path()))
        );
    }

    #[test]
    fn a_relative_or_empty_path_and_a_paired_host_are_refused() {
        assert!(folder_source("kit", true).is_err());
        assert!(folder_source("   ", true).is_err());
        assert!(folder_source(full_path(), false).is_err());
    }
}
