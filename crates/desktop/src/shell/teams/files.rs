//! The team files (D-ED-13, screens 4.9): the team folder and the files of linked projects in a
//! 288 px tree, with the editor beside it. The editor is not built yet, so the pane says so.

use super::team_pill;
use crate::model::teams::{Team, TeamFile};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, div, prelude::*, px};
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::tokens::{SPACE_2, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_CAPTION, TYPE_HEADING};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// The width of the file tree (screens 4.9).
const TREE_WIDTH: f32 = 288.0;

/// The Files page of one team.
pub(in crate::shell) fn files_page(team: &Team, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let folder: Vec<&TeamFile> = team
        .files
        .iter()
        .filter(|file| file.linked_project.is_none())
        .collect();
    let linked: Vec<&TeamFile> = team
        .files
        .iter()
        .filter(|file| file.linked_project.is_some())
        .collect();
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("Files"),
        )
        .child(team_pill(&team.name, team.character, &theme));
    let tree = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .w(px(TREE_WIDTH))
        .flex_none()
        .child(caption("Team folder", &theme))
        .child(if folder.is_empty() {
            note(
                "No files yet. Files a bot creates for the team show here.",
                &theme,
            )
            .into_any_element()
        } else {
            rows(&folder).into_any_element()
        })
        .child(caption("Linked project files", &theme))
        .child(rows(&linked));
    let editor = div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .child(note(
            "Opening a file needs the editor, which is not built yet.",
            &theme,
        ));
    div()
        .id("files-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .p(px(SPACE_6))
        .size_full()
        .child(header)
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .gap(px(SPACE_4))
                .child(tree)
                .child(editor),
        )
        .into_any_element()
}

/// One row per file. A linked file names its project under its path.
fn rows(files: &[&TeamFile]) -> impl IntoElement {
    let mut list = div().flex().flex_col().gap(px(SPACE_2));
    for file in files {
        let (icon, secondary) = match &file.linked_project {
            Some(project) => (IconName::Link, Some(project.clone())),
            None => (IconName::FileText, None),
        };
        let mut row = ListRow::new(format!("file-{}", file.id.0), file.path.clone())
            .leading(RowLeading::Icon(icon));
        if let Some(text) = secondary {
            row = row.secondary(text);
        }
        list = list.child(row);
    }
    list
}

fn caption(text: &'static str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.secondary)
        .child(text)
}

fn note(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}
