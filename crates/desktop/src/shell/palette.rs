//! The command palette: a scrim and a centred panel of grouped, filtered rows (CommandPalette
//! spec, D-CP-01). The model owns the query and the active row; this draws them.

use super::Shell;
use super::actions::{PaletteDown, PaletteUp};
use super::rail::agent_kind;
use super::sidebar_rows::glyph_state;
use crate::model::palette::{
    PaletteGroup, PaletteItem, PaletteMatch, PaletteModel, highlight_ranges,
};
use gpui::{
    AnyElement, Context, Div, Entity, HighlightStyle, IntoElement, MouseButton, MouseDownEvent,
    Stateful, StyledText, WeakEntity, div, prelude::*, px,
};
use massorca_ui::components::agent_icon::{AgentIcon, AgentIconSize};
use massorca_ui::components::agent_status::{AgentStatus, AgentStatusSize};
use massorca_ui::components::kbd::Kbd;
use massorca_ui::components::text_field::TextField;
use massorca_ui::style::Surface;
use massorca_ui::theme::box_shadows;
use massorca_ui::tokens::{
    BORDER_WIDTH_THICK, LAYOUT_PALETTE_MAX_HEIGHT, LAYOUT_PALETTE_TOP, LAYOUT_PALETTE_WIDTH,
    RADIUS_MD, RADIUS_XL, RADIUS_XS, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_4, TYPE_BODY,
    TYPE_BODY_SM, TYPE_CAPTION, TYPE_OVERLINE,
};
use massorca_ui::{ActiveTheme, Icon, IconName, IconSize, TextStyled, Theme};

/// The palette over the window: a scrim that a click closes, and the panel in front of it.
pub(super) fn palette(
    palette: &PaletteModel,
    input: &Entity<TextField>,
    cx: &mut Context<Shell>,
) -> Div {
    let theme = *cx.theme();
    let shell = cx.entity().downgrade();
    let panel = panel(palette, input, &shell, cx, &theme);
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(theme.palette.scrim)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|shell, _: &MouseDownEvent, window, cx| shell.close_palette(window, cx)),
        )
        .child(
            div()
                .absolute()
                .top(px(LAYOUT_PALETTE_TOP))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(panel),
        )
}

/// The dialog: the query field, then the rows grouped as the model orders them.
fn panel(
    palette: &PaletteModel,
    input: &Entity<TextField>,
    shell: &WeakEntity<Shell>,
    cx: &mut Context<Shell>,
    theme: &Theme,
) -> Stateful<Div> {
    let mut results = div()
        .id("palette-results")
        .flex()
        .flex_col()
        .max_h(px(LAYOUT_PALETTE_MAX_HEIGHT))
        .overflow_y_scroll()
        .px(px(SPACE_1))
        .pb(px(SPACE_1));
    let mut group = None;
    for (index, found) in palette.matches().iter().enumerate() {
        let item = &palette.items()[found.item];
        if group != Some(item.group) {
            results = results.child(group_header(item.group, theme));
            group = Some(item.group);
        }
        let active = index == palette.active_index();
        results = results.child(row(index, item, found, active, shell, theme));
    }
    if let Some(text) = palette.no_match_text() {
        results = results.child(
            div()
                .px(px(SPACE_2))
                .py(px(SPACE_1))
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(text),
        );
    }
    div()
        .id("command-palette")
        .key_context("CommandPalette")
        .aria_label("Command palette")
        .on_action(cx.listener(|shell, _: &PaletteUp, _, cx| {
            shell.model.move_palette(-1);
            cx.notify();
        }))
        .on_action(cx.listener(|shell, _: &PaletteDown, _, cx| {
            shell.model.move_palette(1);
            cx.notify();
        }))
        // A click on the panel itself must not reach the scrim and close it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .flex()
        .flex_col()
        .w(px(LAYOUT_PALETTE_WIDTH))
        .overflow_hidden()
        .bg(theme.palette.surface.overlay)
        .border_1()
        .border_color(theme.palette.border.default)
        .rounded(px(RADIUS_XL))
        .shadow(box_shadows(theme.shadows.e3))
        .child(div().p(px(SPACE_2)).child(input.clone()))
        .child(results)
}

fn group_header(group: PaletteGroup, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(SPACE_4))
        .px(px(SPACE_2))
        .text_style(TYPE_OVERLINE)
        .text_color(theme.palette.text.tertiary)
        .child(group.header())
}

/// One result: the leading icon, the title with its matches highlighted, the location of a
/// session, and the trailing state or shortcut. The active row has the selected fill and a
/// 2 px accent bar (CommandPalette spec).
fn row(
    index: usize,
    item: &PaletteItem,
    found: &PaletteMatch,
    active: bool,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> AnyElement {
    let fills = Surface::Overlay.fills(theme);
    let shell = shell.clone();
    let mut row = div()
        .id(("palette-row", index))
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPACE_2))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .text_style(TYPE_BODY)
        .text_color(theme.palette.text.primary)
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.activate_palette_row(index, window, cx)
            });
        });
    row = if active {
        row.bg(fills.selected).child(accent_bar(theme))
    } else {
        row.hover(move |style| style.bg(fills.hover))
    };
    row = row
        .child(leading(item, theme))
        .child(title(item, found, theme));
    if !item.detail.is_empty() {
        row = row.child(
            div()
                .flex_none()
                .min_w_0()
                .truncate()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child(item.detail.clone()),
        );
    }
    if let Some(trailing) = trailing(item) {
        row = row.child(div().flex_none().ml_auto().child(trailing));
    }
    row.into_any_element()
}

/// A session starts with its agent's mark (A13); a workspace with a folder; a command with the
/// command glyph.
fn leading(item: &PaletteItem, theme: &Theme) -> AnyElement {
    if let Some(agent) = item.agent {
        return AgentIcon::new(agent_kind(agent))
            .size(AgentIconSize::Small)
            .into_any_element();
    }
    let name = match item.group {
        PaletteGroup::Workspaces => IconName::Folder,
        PaletteGroup::Commands => IconName::Command,
        PaletteGroup::Sessions | PaletteGroup::Tabs => IconName::Square,
    };
    Icon::new(name)
        .size(IconSize::Small)
        .color(theme.palette.text.secondary)
        .into_any_element()
}

/// The title, with the matched characters on the match background.
fn title(item: &PaletteItem, found: &PaletteMatch, theme: &Theme) -> impl IntoElement {
    let highlight = HighlightStyle {
        background_color: Some(theme.palette.r#match.bg.into()),
        ..HighlightStyle::default()
    };
    let runs = highlight_ranges(&item.title, &found.positions)
        .into_iter()
        .map(|range| (range, highlight));
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .child(StyledText::new(item.title.clone()).with_highlights(runs))
}

/// A session's state glyph, or a command's shortcut, at the trailing edge.
fn trailing(item: &PaletteItem) -> Option<AnyElement> {
    if let Some(state) = item.state {
        return Some(
            AgentStatus::new(glyph_state(state))
                .size(AgentStatusSize::XSmall)
                .into_any_element(),
        );
    }
    item.shortcut
        .map(|shortcut| Kbd::new(shortcut).into_any_element())
}

/// The 2 px accent bar on the active row's left edge.
fn accent_bar(theme: &Theme) -> impl IntoElement {
    div()
        .absolute()
        .left_0()
        .top(px(SPACE_1))
        .bottom(px(SPACE_1))
        .w(px(BORDER_WIDTH_THICK))
        .rounded(px(RADIUS_XS))
        .bg(theme.palette.accent.fg)
}
