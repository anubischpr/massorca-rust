//! The desktop shell: the window frame, its regions, and the command palette (D-SH-01).

mod actions;
mod add_host;
mod app_theme;
mod bring_back;
mod canvas;
mod close_confirm;
mod debug_focus;
mod debug_page;
mod delegate;
mod diff_page;
mod home_start;
mod marketplace;
mod pairing;
mod palette;
mod project_sidebar;
mod rail;
mod resize_edges;
mod secret_requests;
mod service_banner;
mod session_view;
mod settings;
mod settings_remove_dialog;
mod settings_secret_dialog;
mod setup;
mod sidebar;
mod sidebar_rows;
mod source_control;
mod source_control_dialogs;
mod start_page;
mod status_bar;
mod teams;
mod title_bar;
mod tooltip;
mod work_panel;
mod workspace_dialog_views;
mod workspace_dialogs;
mod zones;

use crate::data::DataSource;
use crate::launch_options::{DebugFocus, DebugPage};
use crate::model::canvas::Canvas;
use crate::model::close_confirm::CloseRisk;
use crate::model::devices::RevokeFlow;
use crate::model::marketplace::{
    AccountId, MarketplaceSource, SampleMarketplaceSource, ServiceMarketplace,
};
use crate::model::navigation::Page;
use crate::model::pairing::PairingState;
use crate::model::palette::{Command, PaletteAction, PaletteMode};
use crate::model::region_cycle::Zone;
use crate::model::regions::Region;
use crate::model::settings::SettingsModel;
use crate::model::shell_state::ShellModel;
use crate::model::sidebar::SidebarFolds;
use crate::model::teams::{TeamsSource, TeamsState, UnavailableTeams};
use crate::service::{ServiceBridge, ServiceHandle};
use crate::store::{ClientDataSource, ServiceStore};
use crate::ui_state;
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
use massorca_ui::components::select::SelectView;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::{ActiveTheme, IconName, TextStyled};
use rail::RailState;
use source_control::SourceControl;
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;
use workspace_dialogs::WorkspaceDialogs;
use zones::ZoneFocus;

#[cfg(test)]
mod add_host_tests;
#[cfg(test)]
mod app_theme_tests;
#[cfg(test)]
mod bring_back_tests;
#[cfg(test)]
mod close_confirm_tests;
#[cfg(test)]
mod debug_focus_tests;
#[cfg(test)]
mod debug_page_tests;
#[cfg(test)]
mod delegate_tests;
#[cfg(test)]
mod home_start_tests;
#[cfg(test)]
mod marketplace_accounts_tests;
#[cfg(test)]
mod pairing_tests;
#[cfg(test)]
mod seen_tests;
#[cfg(test)]
mod service_tests;
#[cfg(test)]
mod session_view_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod sidebar_fold_tests;
#[cfg(test)]
mod source_control_tests;
#[cfg(test)]
mod start_page_tests;
#[cfg(test)]
mod status_bar_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod title_bar_tests;
#[cfg(test)]
mod work_panel_tests;

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
    /// The vault page's search over names; the text it settles on becomes `settings.vault_query`.
    vault_search: Entity<SearchField>,
    _vault_search_changes: Subscription,
    /// The workspace folder's text field, shown on the Settings page.
    settings_folder: Entity<TextField>,
    /// The disabled state the field was last set to, so a frame does not notify it again.
    settings_folder_disabled: bool,
    /// Whether the paired hosts were asked for since the Settings page was last shown.
    settings_hosts_asked: bool,
    /// Whether the service link was connected at the last store update, to see a reconnect.
    service_connected: bool,
    /// The Pairing page's state: the offer this computer makes and the pairing it accepts (D-PR-01).
    pairing: PairingState,
    /// The offer another host shows, typed or pasted, and the optional name for that host.
    pairing_offer: Entity<TextField>,
    pairing_name: Entity<TextField>,
    _pairing_changes: Subscription,
    /// Whether the paired devices were asked for since the Pairing page was last shown.
    devices_asked: bool,
    /// The revoke confirm on screen, if one is open (D-PR-02).
    revoke: Option<RevokeFlow>,
    /// The Add host dialog on screen, if one is open (D-HO-02, SSH path).
    add_host: Option<add_host::AddHostFlow>,
    /// The `.ssh` folder whose key files the Add host dialog lists.
    ssh_dir: std::path::PathBuf,
    _settings_folder_changes: Subscription,
    _settings_folder_blur: Subscription,
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
    /// Keeps the debug focus's query log alive; only a debug launch sets it.
    _debug_input: Option<Subscription>,
    /// The debug focus the launch asked for, if any; the start page composer logs for it.
    debug_focus: Option<DebugFocus>,
    /// The page or dialog `--page` asks for, until it opens or falls back.
    debug_page: Option<DebugPage>,
    /// Each workspace's start page composer, kept so its draft survives redraws.
    start_composers: HashMap<String, start_page::StartComposer>,
    /// The session a start is asking the service for, until the service answers.
    start_pending: Option<crate::model::start_page::StartingSession>,
    /// The project the start page with no workspace creates in; `None` stands for the first one.
    home_project: Option<String>,
    /// Each session's composer, kept so its draft survives redraws and switches between sessions.
    session_composers: HashMap<String, session_view::SessionComposer>,
    /// The call in flight for each session, if any: one at a time per session.
    session_calls: HashMap<String, session_view::SessionCall>,
    /// Why the last stop or resume of each session was refused, shown above its composer.
    session_notices: HashMap<String, String>,
    dialogs: WorkspaceDialogs,
    /// The Source control panel's state: its tab, its watch and the commit box (D-DF-02).
    source_control: SourceControl,
    /// The New team or Add bot dialog on screen, if any (screens 4.5, D-AG-33).
    team_dialog: Option<teams::TeamDialog>,
    /// The confirm before a project is deleted, if one is open.
    team_confirm: Option<teams::TeamConfirm>,
    /// The menu of a team card, if one is open (screen 4.5).
    team_menu: Option<teams::OpenTeamMenu>,
    /// The Add or Rotate secret dialog on screen, if any (X-VT-01, decision 0071).
    vault_dialog: Option<settings_secret_dialog::VaultDialog>,
    /// The delete confirm for an owned secret on screen, if any (D-VT-02, decision 0071).
    vault_remove: Option<settings_remove_dialog::RemoveDialog>,
    /// The last refused answer to a secret request, with the request it concerns (decision 0071).
    request_notice: Option<(String, String)>,
    /// The Marketplace data source: the sample, or none while no service answers (decision 0059).
    marketplace: Box<dyn MarketplaceSource>,
    /// The catalogue's search field; its settled text becomes the Marketplace query.
    marketplace_search: Entity<SearchField>,
    _marketplace_search_changes: Subscription,
    /// The code field of the account sign-in dialog.
    marketplace_code: Entity<TextField>,
    /// The last refused change, with the account it concerns (None for the whole Connection part).
    marketplace_error: Option<(Option<AccountId>, String)>,
    /// The uninstall confirm on screen, if any.
    marketplace_uninstall: Option<marketplace::UninstallConfirm>,
    /// The install review of a catalogue package on screen, if any (decision 0062).
    marketplace_review: Option<marketplace::InstallReview>,
    /// The last refused change to an installed package, with the item it concerns.
    marketplace_item_error: Option<(crate::model::marketplace::MarketplaceItemId, String)>,
    /// The update checks of the installed packages, and the review each checked package returned (decision 0062).
    marketplace_updates: marketplace::UpdateChecks,
    /// What the Connection part knows beyond the service's account lists.
    marketplace_accounts: marketplace::AccountsUi,
    /// The host the Marketplace is aimed at, and the answer it waits for.
    marketplace_pick: marketplace::HostPick,
    /// The hosts the Marketplace's picker offers, in its order.
    marketplace_offers: Vec<crate::model::marketplace::HostTarget>,
    /// The Marketplace's host picker, and the subscription that picks the host chosen in it.
    marketplace_host_select: Entity<SelectView>,
    _marketplace_host_changes: Subscription,
    /// The terminal views of the Work panel, by terminal id; a view exists while its pane does.
    terminal_views: BTreeMap<String, work_panel::TerminalView>,
    /// The terminal whose closing waits for the stop question.
    stop_prompt: Option<String>,
    /// A refused terminal call, as its title and its reason, under the Work panel's strip.
    terminal_notice: Option<(String, String)>,
    /// The bring-back confirm on screen, if any (decision 0055).
    bring_back: Option<bring_back::BringBackConfirm>,
    /// The confirm before the window closes, with the sessions it names, if one is on screen (PLAT-10).
    close_confirm: Option<CloseRisk>,
    /// The delegate form under the open session, if any (X-CH-06).
    delegate: Option<delegate::DelegateForm>,
    /// The last refused theme write, as the service's sentence, above the Canvas until dismissed.
    theme_notice: Option<String>,
    /// Follows the operating system's appearance, which the `system` choice shows.
    _appearance_changes: Subscription,
    /// The workspaces whose folded sessions the Sidebar shows unfolded (WS-33). Not saved.
    sidebar_folds: SidebarFolds,
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
        let marketplace = Box::new(SampleMarketplaceSource::default());
        Self::with_store(store, None, None, teams.teams(), marketplace, window, cx)
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
            Box::new(ServiceMarketplace::default()),
            window,
            cx,
        )
    }

    /// Opens the regions the last run left open (SET-02); called before the first frame.
    pub fn restore_open_regions(&mut self, open: Vec<Region>) {
        self.model.restore_open_regions(open);
    }

    fn with_store(
        store: Entity<ServiceStore>,
        service: Option<ServiceHandle>,
        bridge: Option<ServiceBridge>,
        teams: TeamsState,
        marketplace: Box<dyn MarketplaceSource>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let width = f32::from(window.viewport_size().width);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        // A close the OS asks for waits on the confirm while a session runs or waits for the person (PLAT-10).
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |shell, cx| shell.window_may_close(window, cx))
                .unwrap_or(true)
        });
        let snapshot = store.read(cx).snapshot(Instant::now());
        let appearance_changes =
            cx.observe_window_appearance(window, |shell, _, cx| shell.follow_app_theme(cx));
        let store_changes = cx.observe(&store, |shell, store, cx| {
            let snapshot = store.read(cx).snapshot(Instant::now());
            let connected = store.read(cx).state().connection.is_connected();
            if connected && !shell.service_connected {
                // A reconnect asks for the paired hosts and devices again while those pages are on screen.
                shell.settings_hosts_asked = false;
                shell.devices_asked = false;
            }
            shell.service_connected = connected;
            shell.follow_app_theme(cx);
            shell.follow_service_folder(store.read(cx).state());
            if connected {
                shell.settings.folder.forget_read_failure();
            }
            shell.prune_terminal_views(store.read(cx).state());
            shell.model.set_snapshot(snapshot);
            let hosts =
                crate::model::hosts::host_rows(&store.read(cx).state().hosts, Instant::now());
            shell.model.set_remote_hosts(hosts);
            if shell.service.is_some() {
                let hosts = crate::model::hosts::marketplace_hosts(store.read(cx).state());
                shell.model.set_marketplace_hosts(hosts);
            }
            let offers = match shell.service {
                Some(_) => crate::model::hosts::offered_targets(store.read(cx).state()),
                None => vec![crate::model::marketplace::HostTarget::Local],
            };
            shell.follow_host_offers(offers, cx);
            if let Some(teams) = store.read(cx).teams() {
                shell.model.refresh_teams(teams);
            }
            if !store.read(cx).is_sample() {
                let state = store.read(cx).state();
                shell.marketplace.follow_agent_kinds(&state.agent_kinds);
                let (installed, catalogue) = state.extension_lists();
                shell.marketplace.follow_extensions(installed, catalogue);
                let checks = state
                    .extensions
                    .as_ref()
                    .map(|read| read.number)
                    .and_then(|read| {
                        shell
                            .marketplace_updates
                            .begin(read, installed)
                            .map(|checks| (read, checks))
                    });
                shell.follow_service_accounts(state);
                shell.show_marketplace(cx);
                if let Some((read, checks)) = checks {
                    shell.start_update_checks(read, checks, cx);
                }
            }
            cx.notify();
        });
        // Every change of the shell is checked for a new set of open regions, so the next start opens the same ones.
        cx.observe_self(|shell, cx| {
            let open = shell.model.regions().open().to_vec();
            ui_state::remember(cx, |state| state.open_regions = open);
        })
        .detach();
        let rail = RailState::new(snapshot.usage.len(), cx);
        let zones = ZoneFocus::new(cx);
        let search = cx.new(|cx| SearchField::new("Search", cx));
        let mut model = ShellModel::new(snapshot, width);
        model.set_teams(teams);
        model.set_remote_hosts(crate::model::hosts::host_rows(
            &store.read(cx).state().hosts,
            Instant::now(),
        ));
        model.set_marketplace(marketplace.snapshot());
        if service.is_some() {
            model.set_marketplace_hosts(crate::model::hosts::marketplace_hosts(
                store.read(cx).state(),
            ));
        }
        let marketplace_search = cx.new(|cx| SearchField::new("Search the catalogue", cx));
        let marketplace_search_changes = cx.subscribe(
            &marketplace_search,
            |shell, _, event: &SearchFieldEvent, cx| {
                if let SearchFieldEvent::Search(query) = event {
                    shell.set_marketplace_query(query.as_ref(), cx);
                }
            },
        );
        let (marketplace_host_select, marketplace_host_changes) = marketplace::host_picker_for(
            &[crate::model::marketplace::HostTarget::Local],
            &crate::model::marketplace::HostTarget::Local,
            cx,
        );
        let marketplace_code = cx.new(|cx| {
            TextField::new(cx)
                .label("Code")
                .placeholder("Code from the sign-in page")
        });
        let settings_search = cx.new(|cx| SearchField::new("Search settings", cx));
        let vault_search = cx.new(|cx| SearchField::new("Search names", cx));
        let vault_search_changes =
            cx.subscribe(&vault_search, |shell, _, event: &SearchFieldEvent, cx| {
                if let SearchFieldEvent::Search(query) = event {
                    shell.set_vault_query(query.as_ref(), cx);
                }
            });
        let settings_folder =
            cx.new(|cx| TextField::new(cx).placeholder("Full folder path").mono());
        let pairing_offer = cx.new(|cx| TextField::new(cx).placeholder("Paste the offer").mono());
        let pairing_name = cx.new(|cx| TextField::new(cx).placeholder("Name"));
        // Typing in the offer field redraws the page, which enables Pair once there is an offer.
        let pairing_changes =
            cx.subscribe(&pairing_offer, |_, _, _: &TextFieldEvent, cx| cx.notify());
        let folder_changes = cx.subscribe(
            &settings_folder,
            |shell, field, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed => {
                    let text = field.read(cx).text().to_owned();
                    shell.settings.folder.typed(&text);
                    cx.notify();
                }
                TextFieldEvent::Submitted => shell.save_folder(cx),
            },
        );
        let folder_focus = settings_folder.read(cx).focus_handle(cx);
        let folder_blur = cx.on_blur(&folder_focus, window, |shell, window, cx| {
            shell.save_folder_on_blur(window, cx);
        });
        let settings_search_changes = cx.subscribe(
            &settings_search,
            |shell, _, event: &SearchFieldEvent, cx| {
                if let SearchFieldEvent::Search(query) = event {
                    shell.settings.query = query.to_string();
                    cx.notify();
                }
            },
        );
        let commit_field = cx.new(|cx| {
            TextField::new(cx)
                .label("Message")
                .placeholder("Commit message")
        });
        let commit_changes = cx.subscribe(&commit_field, |shell, _, event: &TextFieldEvent, cx| {
            if matches!(event, TextFieldEvent::Changed) {
                shell.read_commit_message(cx);
            }
        });
        Self {
            model,
            focus,
            rail,
            zones,
            search,
            settings: SettingsModel::default(),
            settings_search,
            _settings_search_changes: settings_search_changes,
            vault_search,
            _vault_search_changes: vault_search_changes,
            settings_folder,
            settings_folder_disabled: false,
            settings_hosts_asked: false,
            service_connected: false,
            pairing: PairingState::default(),
            pairing_offer,
            pairing_name,
            _pairing_changes: pairing_changes,
            devices_asked: false,
            revoke: None,
            add_host: None,
            ssh_dir: add_host::default_ssh_dir(),
            _settings_folder_changes: folder_changes,
            _settings_folder_blur: folder_blur,
            palette_input: None,
            palette_restore: None,
            palette_events: None,
            store,
            service,
            _bridge: bridge,
            _store_changes: store_changes,
            _debug_input: None,
            debug_focus: None,
            debug_page: None,
            start_composers: HashMap::new(),
            start_pending: None,
            home_project: None,
            session_composers: HashMap::new(),
            session_calls: HashMap::new(),
            session_notices: HashMap::new(),
            dialogs: WorkspaceDialogs::default(),
            source_control: SourceControl::new(commit_field, commit_changes),
            team_dialog: None,
            team_confirm: None,
            team_menu: None,
            vault_dialog: None,
            vault_remove: None,
            request_notice: None,
            marketplace,
            marketplace_search,
            _marketplace_search_changes: marketplace_search_changes,
            marketplace_code,
            marketplace_error: None,
            marketplace_uninstall: None,
            marketplace_review: None,
            marketplace_item_error: None,
            marketplace_updates: marketplace::UpdateChecks::default(),
            marketplace_accounts: marketplace::AccountsUi::default(),
            marketplace_pick: marketplace::HostPick::restored(
                ui_state::saved_marketplace_host(cx).unwrap_or_default(),
            ),
            marketplace_offers: vec![crate::model::marketplace::HostTarget::Local],
            marketplace_host_select,
            _marketplace_host_changes: marketplace_host_changes,
            terminal_views: BTreeMap::new(),
            stop_prompt: None,
            terminal_notice: None,
            bring_back: None,
            close_confirm: None,
            delegate: None,
            theme_notice: None,
            _appearance_changes: appearance_changes,
            sidebar_folds: SidebarFolds::default(),
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
            PaletteAction::Command(Command::ToggleTheme) => self.toggle_app_theme(cx),
            PaletteAction::OpenSession(id) => self.open_session(&id, cx),
            other => self.model.run_palette_action(&other),
        }
        cx.notify();
    }

    /// Opens a session and tells the service the person has seen what it reported. The store follows the service's
    /// event; a refusal changes nothing, so the session just stays unread.
    pub(crate) fn open_session(&mut self, id: &str, cx: &mut Context<Self>) {
        self.model.open_session(id);
        let Some(service) = self.service.clone() else {
            return;
        };
        let state = self.store.read(cx).state();
        let Some(session) = state.agents.iter().find(|session| session.id == id) else {
            return;
        };
        if !crate::model::agent_sessions::is_unread(session, state.agent_seen) {
            return;
        }
        // Name the activity shown, so a newer one that arrives meanwhile stays unread.
        let at_ms = session.activity.as_ref().map(|activity| activity.at_ms);
        let params = massorca_protocol::methods::agents::AgentMarkSeenParams {
            session: id.to_owned(),
            at_ms,
        };
        cx.spawn(async move |_, _| {
            let _ = service.agent_mark_seen(params).await;
        })
        .detach();
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
    // The composer binds its text area and the text fields it holds: Enter, Backspace, arrows.
    massorca_ui::components::composer::bind_keys(cx);
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
        self.drive_debug_page(window, cx);
        let shell = cx.entity().downgrade();
        let show_controls = title_bar::draws_window_controls(
            window.window_decorations(),
            cfg!(target_os = "linux"),
        );
        let title_bar = title_bar::title_bar(
            &self.model,
            &shell,
            window.is_maximized(),
            show_controls,
            &theme,
        )
        .track_focus(self.zones.get(Zone::TitleBar));
        let rail = rail::rail(&self.model, &self.rail, cx).track_focus(self.zones.get(Zone::Rail));
        let resize_grabs = resize_edges::resize_grabs(window);
        self.refresh_folder(cx);
        self.refresh_settings_hosts();
        self.refresh_devices(cx);
        self.sync_folder_field(window, cx);
        self.sync_pairing_field(window, cx);
        self.sync_source_control(window, cx);
        let connected = self.store.read(cx).state().connection.is_connected();
        // The start page's composer is created before the frames below borrow the Shell's fields.
        let start = self.model.selected_workspace().and_then(|id| {
            crate::model::start_page::workspace_start(self.store.read(cx).state(), id)
        });
        let start = start.map(|page| {
            let composer = self.start_composer(&page.id, cx);
            let starting = self
                .start_pending
                .as_ref()
                .filter(|pending| pending.workspace == page.id)
                .map(|pending| pending.status());
            let mode = self.start_mode(&page.id);
            let kind_notice = self.kind_notice(&composer, cx);
            start_page::StartFrame {
                kind_notice,
                page,
                composer,
                shell: shell.clone(),
                connected,
                starting,
                mode,
            }
        });
        let home = (self.model.canvas()
            == crate::model::canvas::Canvas::Home(crate::model::navigation::Mode::Workspaces))
        .then(|| self.home_frame(cx))
        .flatten();
        let session = self.session_frame(cx);
        let connection = self.store.read(cx).state().connection.clone();
        let hosts = self
            .store
            .read(cx)
            .state()
            .hosts
            .iter()
            .map(|entry| {
                (
                    entry.record.name.clone(),
                    crate::model::hosts::host_state(entry, Instant::now()),
                )
            })
            .collect();
        let settings = settings::SettingsFrame {
            model: &self.settings,
            search: &self.settings_search,
            vault_search: &self.vault_search,
            connection,
            hosts,
            shell: shell.clone(),
            folder: &self.settings.folder,
            folder_field: &self.settings_folder,
            app_theme: self.stored_app_theme(cx),
            agents: crate::model::marketplace::installed_agent_links(self.model.marketplace()),
            vault: {
                let state = self.store.read(cx).state();
                crate::model::secret_vault::VaultPage::from_state(&state.secrets, &state.projects)
            },
            requests: {
                let state = self.store.read(cx).state();
                crate::model::secret_requests::request_rows(&state.secrets, &state.projects)
            },
            request_notice: self.request_notice.clone(),
        };
        let pairing = pairing::PairingFrame {
            state: self.pairing.clone(),
            offer_field: self.pairing_offer.clone(),
            name_field: self.pairing_name.clone(),
            devices: crate::model::devices::devices_view(
                self.store.read(cx).state().devices.as_ref(),
                crate::model::pairing::now_ms(),
            ),
            shell: shell.clone(),
        };
        let setup_rows = crate::model::setup::checklist(
            self.store.read(cx).state(),
            if theme.is_dark() { "Dark" } else { "Light" },
        );
        let work_panel = self
            .model
            .region_visible(Region::WorkPanel)
            .then(|| self.work_frame(shell.clone(), cx));
        let diff = self
            .source_control
            .open
            .is_some()
            .then(|| diff_page::diff_page(self, &shell, cx));
        let project = self
            .model
            .region_visible(Region::ProjectSidebar)
            .then(|| project_sidebar::project_body(self, &shell, cx));
        let marketplace = match self.model.canvas() {
            Canvas::Page(Page::Marketplace(page)) => {
                Some(marketplace::frame(self, page, shell.clone(), cx))
            }
            _ => None,
        };
        let canvas = canvas::canvas(
            &self.model,
            &self.zones,
            canvas::PageFrames {
                settings,
                start,
                home,
                setup_rows,
                work_panel,
                project,
                diff,
                marketplace,
                pairing,
                session,
            },
            window,
            cx,
        );
        let status = status_bar::status_bar(
            &self.model.snapshot().status,
            self.model.snapshot().service_status.as_ref(),
            self.model.status_items(),
            self.model.navigation().mode,
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
        let theme_banner = self
            .theme_notice
            .as_deref()
            .map(|notice| app_theme::theme_notice_banner(notice, &shell));
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
                        &self.sidebar_folds,
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
                    .when_some(theme_banner, |el, banner| el.child(banner))
                    .child(canvas)
                    .child(status),
            )
            .when(self.model.sidebar_overlay(), |el| {
                el.child(
                    sidebar::sidebar(
                        &self.model,
                        &self.sidebar_folds,
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
        let discard = self
            .source_control
            .discard
            .as_ref()
            .map(|flow| source_control_dialogs::discard_dialog(flow, &shell, cx));
        let team_dialog = self
            .team_dialog
            .as_ref()
            .map(|dialog| teams::team_dialog_view(dialog, self.model.teams().hosts(), &shell, cx));
        let team_confirm = self
            .team_confirm
            .as_ref()
            .map(|confirm| teams::team_confirm_view(confirm, &shell, cx));
        let team_menu = self
            .team_menu
            .as_ref()
            .map(|open| menu_overlay(&open.menu, MenuOrigin::At(open.at), cx));
        let revoke = self
            .revoke
            .as_ref()
            .map(|flow| pairing::revoke_dialog(flow, &shell));
        let add_host = self
            .add_host
            .as_ref()
            .map(|flow| add_host::add_host_dialog(flow, &shell, cx));
        let vault_dialog = self.vault_dialog.as_ref().map(|dialog| {
            let connected = self.store.read(cx).state().connection.is_connected();
            settings_secret_dialog::secret_dialog(dialog, &shell, connected, cx)
        });
        let vault_remove = self.vault_remove.as_ref().map(|dialog| {
            let connected = self.store.read(cx).state().connection.is_connected();
            settings_remove_dialog::remove_dialog_view(dialog, &shell, connected, cx)
        });
        let marketplace_dialog = marketplace::dialog(self, &shell, cx);
        let bring_back_dialog = bring_back::dialog(self, &shell, cx);
        let delegate_dialog = delegate::dialog(self, &shell, cx);
        let close_dialog = close_confirm::dialog(self, &shell, cx);
        let stop_terminal = self
            .stop_prompt
            .is_some()
            .then(|| work_panel::stop_dialog(&shell, cx));
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.palette.surface.frame)
            .text_color(theme.palette.text.primary)
            .text_body()
            .track_focus(&self.focus)
            // Escape closes the palette first, then an overlay region. The palette's own
            // Escape binding loses to this one, so the palette is handled here. With nothing
            // of the shell open it propagates, so a focused Search field clears.
            .on_action(cx.listener(|shell, _: &CloseOverlay, window, cx| {
                if shell.model.palette().is_some() {
                    shell.close_palette(window, cx);
                } else if shell.model.usage_popover().is_some() {
                    shell.model.close_usage_popover();
                    cx.notify();
                } else if shell.settings.folder.dirty() {
                    shell.revert_folder(window, cx);
                } else if shell.model.has_overlay() {
                    shell.model.close_overlay();
                    cx.notify();
                } else {
                    cx.propagate();
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
            .when_some(resize_grabs, |el, grabs| el.child(grabs))
            .when_some(palette, |el, palette| el.child(palette))
            .when_some(create, |el, dialog| el.child(dialog))
            .when_some(add_project, |el, dialog| el.child(dialog))
            .when_some(remove, |el, dialog| el.child(dialog))
            .when_some(discard, |el, dialog| el.child(dialog))
            .when_some(menu, |el, menu| el.child(menu))
            .when_some(team_menu, |el, menu| el.child(menu))
            .when_some(team_dialog, |el, dialog| el.child(dialog))
            .when_some(team_confirm, |el, dialog| el.child(dialog))
            .when_some(revoke, |el, dialog| el.child(dialog))
            .when_some(add_host, |el, dialog| el.child(dialog))
            .when_some(vault_dialog, |el, dialog| el.child(dialog))
            .when_some(vault_remove, |el, dialog| el.child(dialog))
            .when_some(marketplace_dialog, |el, dialog| el.child(dialog))
            .when_some(bring_back_dialog, |el, dialog| el.child(dialog))
            .when_some(delegate_dialog, |el, dialog| el.child(dialog))
            .when_some(stop_terminal, |el, dialog| el.child(dialog))
            .when_some(close_dialog, |el, dialog| el.child(dialog))
    }
}
