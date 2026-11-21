//! The Settings page (D-ST-01 to D-ST-12, section 6): a navigation column of twelve sections, a
//! search over every row, and the rows of the section chosen. A row with a backing is live; the
//! others are drawn off with the reason, and nothing claims a change the service has not confirmed.

use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, SharedString, WeakEntity, Window, div, px};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::components::search_field::SearchField;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::settings_row::{SettingsRow, SettingsSection as SettingsGroup};
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{
    LAYOUT_PAGE_SETTINGS, RADIUS_MD, SPACE_1, SPACE_2, SPACE_3, SPACE_6, SPACE_8, TYPE_BODY_SM,
};
use massorca_ui::{ActiveTheme, Icon, IconName, TextStyled, Theme};

use super::Shell;
use crate::model::settings::SettingsModel;
use crate::model::settings_sections::{
    CONVERSATIONS_FOLDER, Hit, NOT_AVAILABLE, Planned, RowBody, RowSpec, SettingsSection, hit_line,
    rows, search,
};
use crate::service::ConnectionState;

/// The navigation column's width (`layout.settings.nav`, 232 px, owner 2026-11-13).
const NAV_WIDTH: f32 = 232.0;

/// What the Settings page draws in one frame, read from the shell.
pub(super) struct SettingsFrame<'a> {
    pub model: &'a SettingsModel,
    pub search: &'a Entity<SearchField>,
    /// The state of This PC's service, for the Hosts section.
    pub connection: ConnectionState,
    pub shell: WeakEntity<Shell>,
}

/// The Settings page: navigation, search and the rows of the chosen section.
pub(super) fn settings_page(
    frame: SettingsFrame<'_>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let hits = search(&frame.model.query);
    let searching = !frame.model.query.trim().is_empty();

    let mut nav = div()
        .w(px(NAV_WIDTH))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .p(px(SPACE_3))
        .border_r_1()
        .border_color(p.border.subtle);
    for section in SettingsSection::ALL {
        nav = nav.child(nav_item(
            section,
            frame.model.section,
            &frame.shell,
            window,
            cx,
        ));
    }

    let content = div()
        .id("settings-content")
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_y_scroll()
        .bg(p.surface.canvas)
        .child(
            div()
                .max_w(px(LAYOUT_PAGE_SETTINGS))
                .w_full()
                .p(px(SPACE_6))
                .flex()
                .flex_col()
                .gap(px(SPACE_8))
                .child(frame.search.clone())
                .when(searching, |el| {
                    el.child(hit_summary(&hits, frame.model.section, &theme))
                })
                .children(sections(&frame, &hits, cx)),
        );

    div()
        .flex()
        .flex_row()
        .size_full()
        .min_h_0()
        .child(nav)
        .child(content)
        .into_any_element()
}

/// One section of the navigation. Selecting it shows its rows; Extensions opens Marketplace.
fn nav_item(
    section: SettingsSection,
    selected: SettingsSection,
    shell: &WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = SharedString::from(format!("settings-nav-{section:?}"));
    let focus = window
        .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();
    let mut item = ListRow::new(id, section.label())
        .leading(RowLeading::Icon(nav_icon(section)))
        .selected(section == selected)
        .focus_handle(focus)
        .tab_stop(true);
    if section.opens_elsewhere() {
        item = item.trailing(Icon::new(IconName::ChevronRight));
    }
    let shell = shell.clone();
    item.on_click(move |_, _, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.settings.section = section;
            cx.notify();
        });
    })
}

/// The Lucide icon of each navigation item (A69): person, bot, palette and so on.
fn nav_icon(section: SettingsSection) -> IconName {
    match section {
        SettingsSection::General => IconName::User,
        SettingsSection::Agents => IconName::Bot,
        SettingsSection::Appearance => IconName::Palette,
        SettingsSection::Input => IconName::Keyboard,
        SettingsSection::Terminal => IconName::Terminal,
        SettingsSection::Sources => IconName::Plug,
        SettingsSection::Hosts => IconName::Server,
        SettingsSection::Notifications => IconName::Bell,
        SettingsSection::Extensions => IconName::Puzzle,
        SettingsSection::Vault => IconName::Lock,
        SettingsSection::Privacy => IconName::Shield,
        SettingsSection::Routing => IconName::Route,
    }
}

/// The count under the search field, shown only while the field has text.
fn hit_summary(hits: &[Hit], selected: SettingsSection, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(hit_line(hits, selected))
}

/// The groups on the page: the chosen section, or every section with a hit while searching.
fn sections(frame: &SettingsFrame<'_>, hits: &[Hit], cx: &mut App) -> Vec<AnyElement> {
    if frame.model.query.trim().is_empty() {
        let section = frame.model.section;
        let listed: Vec<(usize, &'static RowSpec)> = rows(section).iter().enumerate().collect();
        return vec![group(section, &listed, frame, cx)];
    }
    if hits.is_empty() {
        return vec![
            EmptyState::new(
                "settings-no-match",
                format!("No setting matches \"{}\".", frame.model.query.trim()),
            )
            .note("Try a word from the label, such as theme or shortcut.")
            .into_any_element(),
        ];
    }
    let mut groups: Vec<(SettingsSection, Vec<(usize, &'static RowSpec)>)> = Vec::new();
    for hit in hits {
        match groups.last_mut() {
            Some((section, listed)) if *section == hit.section => {
                listed.push((hit.index, hit.row()));
            }
            _ => groups.push((hit.section, vec![(hit.index, hit.row())])),
        }
    }
    groups
        .into_iter()
        .map(|(section, listed)| group(section, &listed, frame, cx))
        .collect()
}

/// One section's group: its title and the rows, each with its control.
fn group(
    section: SettingsSection,
    listed: &[(usize, &'static RowSpec)],
    frame: &SettingsFrame<'_>,
    cx: &mut App,
) -> AnyElement {
    let mut group = SettingsGroup::new(
        SharedString::from(format!("settings-group-{section:?}")),
        section.label(),
    );
    for (index, spec) in listed {
        group = group.row(row(section, *index, spec, frame, cx));
    }
    group.into_any_element()
}

/// One row: the spec's name and helper, and the control its body calls for. Planned controls are
/// drawn off with "Not available yet"; the rows with a backing show their value.
fn row(
    section: SettingsSection,
    index: usize,
    spec: &RowSpec,
    frame: &SettingsFrame<'_>,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let id = SharedString::from(format!("settings-{section:?}-{index}"));
    let row = SettingsRow::new(id.clone(), spec.label).helper(spec.helper);
    let (row, control) = match spec.body {
        RowBody::Planned(Planned::Switch) => (
            row.disabled_reason(NOT_AVAILABLE),
            Switch::new(id.clone(), false)
                .disabled(true)
                .into_any_element(),
        ),
        RowBody::Planned(Planned::Choice(options)) => (
            row.disabled_reason(NOT_AVAILABLE),
            SegmentedControl::new(
                id.clone(),
                SharedString::from(format!("{id}-choice")),
                options
                    .iter()
                    .map(|option| Segment::text(*option))
                    .collect(),
            )
            .disabled(true)
            .into_any_element(),
        ),
        RowBody::Planned(Planned::Action(label)) => (
            row.disabled_reason(NOT_AVAILABLE),
            Button::new(id.clone(), label)
                .variant(ButtonVariant::Secondary)
                .disabled(true)
                .into_any_element(),
        ),
        RowBody::Planned(Planned::Link) => (
            row.disabled_reason(NOT_AVAILABLE),
            link_button(id.clone()).into_any_element(),
        ),
        RowBody::Planned(Planned::Text(value) | Planned::ReadOnly(value)) => {
            (row.disabled_reason(NOT_AVAILABLE), value_box(value, &theme))
        }
        RowBody::ConversationsFolder => (
            row.disabled_reason(NOT_AVAILABLE),
            value_box(CONVERSATIONS_FOLDER, &theme),
        ),
        RowBody::WorkspaceFolder => (
            row.disabled_reason(NOT_AVAILABLE),
            value_box("Not loaded", &theme),
        ),
        RowBody::ThisPc => (row, value_box(host_word(&frame.connection), &theme)),
        RowBody::SetupChecklist => (
            row.disabled_reason(NOT_AVAILABLE),
            link_button(id.clone()).into_any_element(),
        ),
    };
    row.control(control).into_any_element()
}

/// The disabled "Open" button of a link row.
fn link_button(id: SharedString) -> Button {
    Button::new(id, "Open")
        .variant(ButtonVariant::Ghost)
        .trailing_icon(IconName::ChevronRight)
        .disabled(true)
}

/// A read-only value in a sunken box, as a row shows what it holds.
fn value_box(text: impl Into<SharedString>, theme: &Theme) -> AnyElement {
    div()
        .px(px(SPACE_2))
        .py(px(SPACE_1))
        .rounded(px(RADIUS_MD))
        .bg(theme.palette.surface.sunken)
        .text_body()
        .text_color(theme.palette.text.secondary)
        .child(text.into())
        .into_any_element()
}

/// This PC's service in words, in the Hosts section (D-ST-07, honest state).
fn host_word(connection: &ConnectionState) -> &'static str {
    match connection {
        ConnectionState::Connecting => "Connecting",
        ConnectionState::Connected { .. } => "Connected",
        ConnectionState::Reconnecting { .. } => "Reconnecting",
        ConnectionState::Unreachable { .. } => "Unreachable",
    }
}
