//! The desktop shell: the window frame, its regions, and the command palette (D-SH-01).

mod actions;
mod canvas;
mod palette;
mod rail;
mod service_banner;
mod settings;
mod sidebar;
mod sidebar_rows;
mod status_bar;
mod teams;
mod title_bar;
mod tooltip;
mod workspace_dialog_views;
mod workspace_dialogs;
mod zones;

use crate::data::DataSource;
use crate::model::navigation::Page;
use crate::model::palette::{Command, PaletteAction, PaletteMode};
use crate::model::region_cycle::Zone;
use crate::model::settings::SettingsModel;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{TeamsSource, TeamsState, UnavailableTeams};
use crate::service::{ServiceBridge, ServiceHandle};
use crate::store::{ClientDataSource, ServiceStore};
use actions::{
    CloseOverlay, CycleRegionBackward, CycleRegionForward, OpenQuickOpen, OpenSettings,
    ToggleCommandPalette,
};
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, MouseButton, MouseDownEvent, Render,
    Subscription, WeakEntity, Window, div, prelude::*,
};
use massorca_ui::components::menu::{MenuOrigin, menu_overlay};
use massorca_ui::components::search_field::{SearchField, SearchFieldEvent};
use massorca_ui::components::text_field::{self, TextField, TextFieldEvent};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};
use rail::RailState;
use std::time::Instant;
use workspace_dialogs::WorkspaceDialogs;
use zones::ZoneFocus;

#[cfg(test)]
mod service_tests;
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
    /// The Settings page: its section, its search text and the workspace folder.
    settings: SettingsModel,
    /// The Settings search field; the text it settles on becomes `settings.query`.
    settings_search: Entity<SearchField>,
    _settings_search_changes: Subscription,
    /// The palette's query field, present while the palette is open.
    palette_input: Option<Entity<TextField>>,
    /// The control that had focus when the palette opened; focus returns to it.
    palette_restore: Option<FocusHandle>,
    palette_events: Option<Subscription>,
    /// What the Sidebar, status bar and dialogs derive from (decision 0028).
    store: Entity<ServiceStore>,
    /// The way to ask the service for something. None on sample data.
    service: Option<ServiceHandle>,
    /// Keeps the bridge's runtime alive for as long as the shell lives.
    _bridge: Option<ServiceBridge>,
    _store_changes: Subscription,
    dialogs: WorkspaceDialogs,
    /// The New team or Add bot dialog on screen, if any (screens 4.5, D-AG-33).
    team_dialog: Option<teams::TeamDialog>,
    /// The confirm before a project is deleted, if one is open.
    team_confirm: Option<teams::TeamConfirm>,
}

impl Shell {
    /// Fills the store from sample data, once; nothing changes it afterwards. The Bot Teams data
    /// comes from `teams`, the same way (`--sample` selects both).
    pub fn new(
        data: &impl DataSource,
        teams: &impl TeamsSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let store = cx.new(|_| ServiceStore::sample(data));
        Self::with_store(store, None, None, teams.teams(), window, cx)
    }

    /// Follows a running service through the bridge of `source`.
    pub fn with_client(
        source: ClientDataSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let handle = source.handle();
        let (bridge, updates) = source.into_parts();
        let store = cx.new(|cx| ServiceStore::following(updates, cx));
        Self::with_store(
            store,
            Some(handle),
            Some(bridge),
            UnavailableTeams.teams(),
            window,
            cx,
        )
    }

    fn with_store(
        store: Entity<ServiceStore>,
        service: Option<ServiceHandle>,
        bridge: Option<ServiceBridge>,
        teams: TeamsState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let width = f32::from(window.viewport_size().width);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let snapshot = store.read(cx).snapshot(Instant::now());
        let store_changes = cx.observe(&store, |shell, store, cx| {
            let snapshot = store.read(cx).snapshot(Instant::now());
            shell.model.set_snapshot(snapshot);
            cx.notify();
        });
        let rail = RailState::new(snapshot.usage.len(), cx);
        let zones = ZoneFocus::new(cx);
        let search = cx.new(|cx| SearchField::new("Search", cx));
        let mut model = ShellModel::new(snapshot, width);
        model.set_teams(teams);
        let settings_search = cx.new(|cx| SearchField::new("Search settings", cx));
        let settings_search_changes = cx.subscribe(
            &settings_search,
            |shell, _, event: &SearchFieldEvent, cx| {
                if let SearchFieldEvent::Search(query) = event {
                    shell.settings.query = query.to_string();
                    cx.notify();
                }
            },
        );
        Self {
            model,
            focus,
            rail,
            zones,
            search,
            settings: SettingsModel::default(),
            settings_search,
            _settings_search_changes: settings_search_changes,
            palette_input: None,
            palette_restore: None,
            palette_events: None,
            store,
            service,
            _bridge: bridge,
            _store_changes: store_changes,
            dialogs: WorkspaceDialogs::default(),
            team_dialog: None,
            team_confirm: None,
        }
    }

    pub fn model(&self) -> &ShellModel {
        &self.model
    }

    /// "Retry now" on the service banner: resets the backoff and connects at once.
    fn retry_service_now(&self) {
        if let Some(handle) = &self.service {
            handle.retry_now();
        }
    }

    /// Mod+K: opens the palette in the commands mode, or closes it when it is open.
    pub fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.palette().is_some() {
            self.close_palette(window, cx);
            return;
        }
        self.open_palette(PaletteMode::Commands, window, cx);
    }

    /// Mod+P: opens the palette in "Go to file" mode (D-CP-02). Pressed again it closes; over the
    /// commands it switches to files.
    pub fn toggle_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let quick_open = self
            .model
            .palette()
            .is_some_and(|palette| palette.mode() == PaletteMode::GoToFile);
        if quick_open {
            self.close_palette(window, cx);
        } else {
            self.open_palette(PaletteMode::GoToFile, window, cx);
        }
    }

    /// Opens the palette in `mode`, replacing one open in another mode. The control that had focus
    /// when the first palette opened stays the one focus returns to.
    fn open_palette(&mut self, mode: PaletteMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.palette().is_some() {
            self.palette_input = None;
            self.palette_events = None;
        } else {
            self.palette_restore = window.focused(cx);
        }
        self.model.open_palette(mode);
        let placeholder = match mode {
            PaletteMode::Commands => "Search workspaces, tabs and commands",
            PaletteMode::GoToFile => "Search files",
        };
        let input = cx.new(|cx| {
            TextField::new(cx)
                .placeholder(placeholder)
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
        let connection = self.store.read(cx).state().connection.clone();
        let settings = settings::SettingsFrame {
            model: &self.settings,
            search: &self.settings_search,
            connection,
            shell: shell.clone(),
        };
        let canvas = canvas::canvas(&self.model, &self.zones, settings, window, cx);
        let status = status_bar::status_bar(
            &self.model.snapshot().status,
            self.model.snapshot().service_status.as_ref(),
            self.model.status_items(),
            &shell,
            &theme,
        )
        .track_focus(self.zones.get(Zone::StatusBar));
        let banner = self
            .model
            .snapshot()
            .service_notice
            .as_ref()
            .map(|notice| service_banner::service_banner(notice, &shell));
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
                    sidebar::sidebar(
                        &self.model,
                        &self.search,
                        false,
                        self.rail.morph_generation(),
                        window,
                        cx,
                    )
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
                    .when_some(banner, |el, banner| el.child(banner))
                    .child(canvas)
                    .child(status),
            )
            .when(self.model.sidebar_overlay(), |el| {
                el.child(
                    sidebar::sidebar(
                        &self.model,
                        &self.search,
                        true,
                        self.rail.morph_generation(),
                        window,
                        cx,
                    )
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
        let projects = self.store.read(cx).state().projects.clone();
        let create = self
            .dialogs
            .create
            .as_ref()
            .map(|dialog| workspace_dialog_views::create_dialog(dialog, &projects, &shell, cx));
        let add_project = self
            .dialogs
            .add_project
            .as_ref()
            .map(|dialog| workspace_dialog_views::add_project_dialog(dialog, &shell, cx));
        let remove = self
            .dialogs
            .remove
            .as_ref()
            .map(|flow| workspace_dialog_views::remove_dialog(flow, &shell, cx));
        let menu = self
            .dialogs
            .menu
            .as_ref()
            .map(|open| menu_overlay(&open.menu, MenuOrigin::At(open.at), cx));
        let team_dialog = self
            .team_dialog
            .as_ref()
            .map(|dialog| teams::team_dialog_view(dialog, self.model.teams().hosts(), &shell, cx));
        let team_confirm = self
            .team_confirm
            .as_ref()
            .map(|confirm| teams::team_confirm_view(confirm, &shell, cx));
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
                } else if shell.model.usage_popover().is_some() {
                    shell.model.close_usage_popover();
                    cx.notify();
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
            .on_action(cx.listener(|shell, _: &OpenQuickOpen, window, cx| {
                shell.toggle_quick_open(window, cx);
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
            .when_some(create, |el, dialog| el.child(dialog))
            .when_some(add_project, |el, dialog| el.child(dialog))
            .when_some(remove, |el, dialog| el.child(dialog))
            .when_some(menu, |el, menu| el.child(menu))
            .when_some(team_dialog, |el, dialog| el.child(dialog))
            .when_some(team_confirm, |el, dialog| el.child(dialog))
    }
}
