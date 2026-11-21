//! The desktop shell: the window frame, its regions, and the command palette (D-SH-01).

mod actions;
mod canvas;
mod palette;
mod rail;
mod sidebar;
mod sidebar_rows;
mod status_bar;
mod title_bar;
mod tooltip;
mod zones;

use crate::data::DataSource;
use crate::model::navigation::Page;
use crate::model::palette::{Command, PaletteAction};
use crate::model::region_cycle::Zone;
use crate::model::shell_state::ShellModel;
use actions::{
    CloseOverlay, CycleRegionBackward, CycleRegionForward, OpenSettings, ToggleCommandPalette,
};
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, MouseButton, MouseDownEvent, Render,
    Subscription, WeakEntity, Window, div, prelude::*,
};
use massorca_ui::components::search_field::SearchField;
use massorca_ui::components::text_field::{self, TextField, TextFieldEvent};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};
use rail::RailState;
use zones::ZoneFocus;

#[cfg(test)]
mod tests;

/// The root view of the window. Views read only its view model (decision 0023).
#[derive(Debug)]
pub struct Shell {
    model: ShellModel,
    /// Holds keyboard focus when no control does, so shell keys reach the root.
    focus: FocusHandle,
    rail: RailState,
    /// One focus handle per frame zone, which F6 moves between.
    zones: ZoneFocus,
    search: Entity<SearchField>,
    /// The palette's query field, present while the palette is open.
    palette_input: Option<Entity<TextField>>,
    /// The control that had focus when the palette opened; focus returns to it.
    palette_restore: Option<FocusHandle>,
    palette_events: Option<Subscription>,
}

impl Shell {
    /// Fills the view model from the data source; the window width is read once here.
    pub fn new(data: &impl DataSource, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let width = f32::from(window.viewport_size().width);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let snapshot = data.snapshot();
        let rail = RailState::new(snapshot.usage.len(), cx);
        let zones = ZoneFocus::new(cx);
        let search = cx.new(|cx| SearchField::new("Search", cx));
        Self {
            model: ShellModel::new(snapshot, width),
            focus,
            rail,
            zones,
            search,
            palette_input: None,
            palette_restore: None,
            palette_events: None,
        }
    }

    pub fn model(&self) -> &ShellModel {
        &self.model
    }

    /// Mod+K: opens the palette, or closes it when it is open.
    pub fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.palette().is_some() {
            self.close_palette(window, cx);
            return;
        }
        self.palette_restore = window.focused(cx);
        self.model.toggle_palette();
        let input = cx.new(|cx| {
            TextField::new(cx)
                .placeholder("Search workspaces, tabs and commands")
                .leading_icon(IconName::Search)
        });
        self.palette_events = Some(cx.subscribe_in(
            &input,
            window,
            |shell, input, event: &TextFieldEvent, window, cx| match event {
                TextFieldEvent::Changed => {
                    let query = input.read(cx).text().to_owned();
                    shell.model.set_palette_query(&query);
                    cx.notify();
                }
                TextFieldEvent::Submitted => shell.confirm_palette(window, cx),
            },
        ));
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.palette_input = Some(input);
        cx.notify();
    }

    /// Closes the palette without running a row, and returns focus to the opener.
    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model.close_palette();
        self.dismiss_palette(window, cx);
    }

    /// Runs the active row and closes the palette. A disabled row keeps it open.
    fn confirm_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.model.confirm_palette() else {
            return;
        };
        self.dismiss_palette(window, cx);
        match action {
            PaletteAction::Command(Command::ToggleTheme) => Theme::toggle(cx),
            other => self.model.run_palette_action(&other),
        }
        cx.notify();
    }

    /// Clears the query field and gives focus back to the control that opened the palette.
    fn dismiss_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_input = None;
        self.palette_events = None;
        let focus = self
            .palette_restore
            .take()
            .unwrap_or_else(|| self.focus.clone());
        window.focus(&focus, cx);
        cx.notify();
    }

    /// A click on a palette row makes it active and runs it.
    fn activate_palette_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.model.select_palette_row(index);
        self.confirm_palette(window, cx);
    }
}

/// Registers the key bindings of the shell and of the widgets it uses.
pub fn bind_keys(cx: &mut App) {
    text_field::bind_keys(cx);
    actions::bind_keys(cx);
}

/// Applies a control's change to the shell and redraws it. A closed window does nothing.
fn change(shell: &WeakEntity<Shell>, cx: &mut App, apply: impl FnOnce(&mut Shell)) {
    let _ = shell.update(cx, |shell, cx| {
        apply(shell);
        cx.notify();
    });
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        self.model
            .set_window_width(f32::from(window.viewport_size().width));
        let shell = cx.entity().downgrade();
        let title_bar = title_bar::title_bar(&self.model, &shell, window.is_maximized(), &theme)
            .track_focus(self.zones.get(Zone::TitleBar));
        let rail = rail::rail(&self.model, &self.rail, cx).track_focus(self.zones.get(Zone::Rail));
        let canvas = canvas::canvas(&self.model, &self.zones, cx);
        let status = status_bar::status_bar(&self.model.snapshot().status, &theme)
            .track_focus(self.zones.get(Zone::StatusBar));
        // Inline Sidebar, canvas, then an overlay Sidebar on top. A click on the body outside
        // an overlay closes it; the overlays stop their own clicks from reaching the body.
        let body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_row()
            .when(self.model.sidebar_inline(), |el| {
                el.child(
                    sidebar::sidebar(&self.model, &self.search, false, window, cx)
                        .track_focus(self.zones.get(Zone::Sidebar)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(canvas)
                    .child(status),
            )
            .when(self.model.sidebar_overlay(), |el| {
                el.child(
                    sidebar::sidebar(&self.model, &self.search, true, window, cx)
                        .track_focus(self.zones.get(Zone::Sidebar)),
                )
            })
            .when(self.model.overlay().is_some(), |el| {
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _: &MouseDownEvent, _, cx| {
                        shell.model.close_overlay();
                        cx.notify();
                    }),
                )
            });
        let palette = match (self.model.palette(), self.palette_input.clone()) {
            (Some(palette), Some(input)) => Some(palette::palette(palette, &input, cx)),
            _ => None,
        };
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.palette.surface.frame)
            .text_color(theme.palette.text.primary)
            .text_body()
            .track_focus(&self.focus)
            // Escape closes the palette first, then an overlay region. The palette's own
            // Escape binding loses to this one, so the palette is handled here.
            .on_action(cx.listener(|shell, _: &CloseOverlay, window, cx| {
                if shell.model.palette().is_some() {
                    shell.close_palette(window, cx);
                } else {
                    shell.model.close_overlay();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|shell, _: &CycleRegionForward, window, cx| {
                shell.cycle_region(false, window, cx);
            }))
            .on_action(cx.listener(|shell, _: &CycleRegionBackward, window, cx| {
                shell.cycle_region(true, window, cx);
            }))
            .on_action(cx.listener(|shell, _: &ToggleCommandPalette, window, cx| {
                shell.toggle_palette(window, cx);
            }))
            .on_action(cx.listener(|shell, _: &OpenSettings, _, cx| {
                shell.model.open_page(Page::Settings);
                cx.notify();
            }))
            .child(rail)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(title_bar)
                    .child(body),
            )
            .when_some(palette, |el, palette| el.child(palette))
    }
}
