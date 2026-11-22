//! The Settings page (D-ST-01 to D-ST-12, section 6): a navigation column of twelve sections, a
//! search over every row, and the rows of the section chosen. A row with a backing is live; the
//! others are drawn off with the reason, and nothing claims a change the service has not confirmed.

use gpui::prelude::*;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Entity, Focusable, KeyDownEvent, SharedString, WeakEntity, Window,
    div, px,
};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::components::search_field::SearchField;
use massorca_ui::components::settings_row::RowStatus;
use massorca_ui::components::settings_row::{SettingsRow, SettingsSection as SettingsGroup};
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{
    LAYOUT_PAGE_SETTINGS, RADIUS_MD, SPACE_1, SPACE_2, SPACE_3, SPACE_6, SPACE_8, TYPE_BODY_SM,
};
use massorca_ui::{ActiveTheme, Icon, IconName, TextStyled, Theme};

use super::Shell;
use crate::model::hosts::HostState;
use crate::model::marketplace::AgentLink;
use crate::model::navigation::{MarketplacePage, Page};
use crate::model::secret_vault::VaultPage;
use crate::model::settings::{
    FolderSetting, FolderStatus, FolderValue, SettingsModel, folder_reason, folder_refusal_text,
    folder_request,
};
use crate::model::settings_sections::{
    CONVERSATIONS_FOLDER, Hit, NOT_AVAILABLE, RowBody, RowSpec, SettingsSection,
    WORKSPACE_FOLDER_KEY, hit_line, rows, search,
};
use crate::service::ConnectionState;
use massorca_protocol::methods::config::ConfigSet;

#[path = "settings_planned.rs"]
mod settings_planned;
use settings_planned::planned_row;

#[path = "settings_vault.rs"]
mod settings_vault;
use settings_vault::{secret_scopes, secret_status_row};

/// The navigation column's width (`layout.settings.nav`, 232 px, owner 2026-11-13).
const NAV_WIDTH: f32 = 232.0;

/// What the Settings page draws in one frame, read from the shell.
pub(super) struct SettingsFrame<'a> {
    pub model: &'a SettingsModel,
    pub search: &'a Entity<SearchField>,
    /// The state of This PC's service, for the Hosts section.
    pub connection: ConnectionState,
    /// Each paired remote host by name, with the state its last check left (D-HO-07).
    pub hosts: Vec<(String, HostState)>,
    pub shell: WeakEntity<Shell>,
    pub folder: &'a FolderSetting,
    pub folder_field: &'a Entity<TextField>,
    /// The installed agents, each linking to its Marketplace page (decision 0059).
    pub agents: Vec<AgentLink>,
    /// The secret vault as the store holds it: its key, and the names by scope (decision 0071).
    pub vault: VaultPage,
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
    div()
        .debug_selector(move || format!("settings-nav-{section:?}"))
        .child(item.on_click(move |_, _, cx| {
            let _ = shell.update(cx, |shell, cx| {
                if section.opens_elsewhere() {
                    shell.open_marketplace(MarketplacePage::Installed, cx);
                } else {
                    shell.settings.section = section;
                    cx.notify();
                }
            });
        }))
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
        let mut groups = vec![group(section, &listed, frame, cx)];
        if section == SettingsSection::Agents {
            groups.push(installed_agents(frame));
        }
        if section == SettingsSection::Vault {
            groups.extend(secret_scopes(frame));
        }
        return groups;
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
    if let Some(note) = section.note() {
        group = group.description(note);
    }
    for (index, spec) in listed {
        if spec.body == RowBody::PairedHosts {
            let theme = *cx.theme();
            for (name, state) in &frame.hosts {
                group = group.row(host_row(name, state, &theme));
            }
            continue;
        }
        group = group.row(row(section, *index, spec, frame, cx));
    }
    group.into_any_element()
}

/// One paired host: its name, the line its state leaves under it, and the state word as the control.
fn host_row(name: &str, state: &HostState, theme: &Theme) -> AnyElement {
    let selector = format!("settings-host-{name}");
    let row = SettingsRow::new(SharedString::from(selector.clone()), name.to_owned());
    let row = match &state.detail {
        Some(detail) => row.helper(detail.clone()),
        None => row,
    };
    div()
        .debug_selector(move || selector.clone())
        .child(row.control(value_box(state.word, theme)))
        .into_any_element()
}

/// The installed agents, each with a button to its Marketplace page. Settings lists what is
/// installed and never installs anything (decision 0059).
fn installed_agents(frame: &SettingsFrame<'_>) -> AnyElement {
    let mut group = SettingsGroup::new(
        SharedString::from("settings-installed-agents"),
        "Installed agents",
    )
    .description(
        "Installed from the Marketplace. Sign-in, accounts and settings are on each agent's page.",
    );
    if frame.agents.is_empty() {
        group = group.row(
            SettingsRow::new("settings-agents-none", "No agents installed")
                .helper("Install an agent from the Marketplace. Settings never installs anything."),
        );
    }
    for agent in &frame.agents {
        let item = agent.item;
        let shell = frame.shell.clone();
        let open = Button::new(
            SharedString::from(format!("settings-agent-open-{}", item.0)),
            "Open",
        )
        .variant(ButtonVariant::Ghost)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.open_marketplace(MarketplacePage::Item(item), cx);
            });
        });
        group = group.row(
            SettingsRow::new(
                SharedString::from(format!("settings-agent-{}", item.0)),
                agent.name.clone(),
            )
            .helper(agent.state)
            .control(open),
        );
    }
    group.into_any_element()
}

/// One row: the spec's name and helper, and the control its body calls for. Planned settings keep
/// the spec's control, drawn off with its status; the rows with a backing show their value.
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
        RowBody::Planned(planned) => planned_row(row, planned, &id, spec.label, &theme),
        RowBody::ConversationsFolder => (
            row.status(RowStatus::Unavailable(NOT_AVAILABLE.into())),
            Some(value_box(CONVERSATIONS_FOLDER, &theme)),
        ),
        RowBody::WorkspaceFolder => {
            let (row, control) = folder_row(row, frame);
            (row, Some(control))
        }
        RowBody::ThisPc => (row, Some(value_box(host_word(&frame.connection), &theme))),
        // group() draws one row per paired host in place of this marker.
        RowBody::PairedHosts => (row, None),
        RowBody::SetupChecklist => (
            row,
            Some(page_link(
                id.clone(),
                Page::Setup,
                "settings-setup-open",
                frame,
            )),
        ),
        RowBody::AddHost => (row, Some(add_host_button(id.clone(), frame))),
        RowBody::PairingLink => (
            row,
            Some(page_link(
                id.clone(),
                Page::Pairing,
                "settings-pairing-open",
                frame,
            )),
        ),
        RowBody::SecretVault => secret_status_row(row, frame, &theme),
    };
    match control {
        Some(control) => row.control(control).into_any_element(),
        None => row.into_any_element(),
    }
}

/// The "Add host" button of its row: it opens the dialog on this page (D-HO-02).
fn add_host_button(id: SharedString, frame: &SettingsFrame<'_>) -> AnyElement {
    let shell = frame.shell.clone();
    let button = Button::new(id, "Add host")
        .variant(ButtonVariant::Secondary)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_add_host(cx));
        });
    div()
        .debug_selector(|| "settings-add-host".to_owned())
        .child(button)
        .into_any_element()
}

/// A row that opens a full page (the setup checklist, D-OB-12, or Pairing, D-PR-01) with an "Open" button.
fn page_link(
    id: SharedString,
    page: Page,
    selector: &'static str,
    frame: &SettingsFrame<'_>,
) -> AnyElement {
    let shell = frame.shell.clone();
    let button = Button::new(id, "Open")
        .variant(ButtonVariant::Ghost)
        .trailing_icon(IconName::ChevronRight)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.model.open_page(page);
                cx.notify();
            });
        });
    div()
        .debug_selector(move || selector.to_owned())
        .child(button)
        .into_any_element()
}

/// A read-only value in a sunken box, as a row shows what it holds.
pub(super) fn value_box(text: impl Into<SharedString>, theme: &Theme) -> AnyElement {
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

/// This PC's service in words, in the Hosts section (D-ST-07, honest state). It reads the store's
/// connection, the same state the status bar reports.
pub(super) fn host_word(connection: &ConnectionState) -> &'static str {
    match connection {
        ConnectionState::Connecting => "Connecting",
        ConnectionState::Connected { .. } => "Connected",
        ConnectionState::Reconnecting { .. } => "Reconnecting",
        ConnectionState::Unreachable { .. } => "Unreachable",
    }
}

/// The workspace folder: the field the service's value fills, with the row's reason, status and reset.
fn folder_row(row: SettingsRow, frame: &SettingsFrame<'_>) -> (SettingsRow, AnyElement) {
    let mut row = row;
    if let Some(reason) = folder_reason(frame.connection.is_connected(), frame.folder) {
        row = row.disabled_reason(reason);
    }
    if let Some(status) = folder_status(frame.folder) {
        row = row.status(status);
    }
    if frame.folder.modified() {
        let shell = frame.shell.clone();
        row = row.reset(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.reset_folder(cx));
        });
    }
    // Escape puts the typed text back to the stored folder; the key reaches here from the field.
    let shell = frame.shell.clone();
    let control = div()
        .debug_selector(|| "settings-folder".to_owned())
        .w_full()
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                let _ = shell.update(cx, |shell, cx| shell.revert_folder(window, cx));
            }
        })
        .child(frame.folder_field.clone())
        .into_any_element();
    (row, control)
}

/// The status line the row shows for the folder's last change.
fn folder_status(folder: &FolderSetting) -> Option<RowStatus> {
    match folder.status.as_ref()? {
        FolderStatus::Saving => Some(RowStatus::Saving),
        FolderStatus::Saved => Some(RowStatus::Saved),
        FolderStatus::Failed(message) => Some(RowStatus::Error(message.clone().into())),
    }
}

/// How long "Saved" stays on the row (SettingsRow.md: about two seconds).
const SAVED_FOR: Duration = Duration::from_secs(2);

impl Shell {
    /// Asks for the paired hosts each time the Settings page is shown, and again after the service
    /// reconnects. The answer reaches the rows through the store, not through this call.
    pub(super) fn refresh_settings_hosts(&mut self) {
        if self.model.navigation().page != Page::Settings {
            self.settings_hosts_asked = false;
            return;
        }
        if self.settings_hosts_asked {
            return;
        }
        if let Some(service) = &self.service {
            self.settings_hosts_asked = true;
            service.refresh_hosts();
        }
    }

    /// Reads the workspace folder once the service is connected and the value is not known yet.
    pub(super) fn refresh_folder(&mut self, cx: &mut Context<Self>) {
        // The value is read when the page is on screen, not on every connection.
        if self.model.navigation().page != Page::Settings {
            return;
        }
        let connected = self.store.read(cx).state().connection.is_connected();
        if !connected || self.settings.folder.stored.is_some() || self.settings.folder.loading {
            return;
        }
        if matches!(self.settings.folder.status, Some(FolderStatus::Failed(_))) {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.settings.folder.loading = true;
        cx.spawn(async move |this, cx| {
            let result = service.config_list().await;
            let _ = this.update(cx, |shell, cx| {
                shell.settings.folder.loading = false;
                match result {
                    Ok(list) => match list
                        .values
                        .iter()
                        .find(|value| value.key == WORKSPACE_FOLDER_KEY)
                    {
                        Some(value) => shell
                            .settings
                            .folder
                            .loaded(FolderValue::from_config(value)),
                        None => shell
                            .settings
                            .folder
                            .refused("This setting is not available in this version."),
                    },
                    Err(refusal) => shell.settings.folder.refused(folder_refusal_text(&refusal)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Enter in the field: check the typed folder, then ask the service to store it.
    pub(super) fn save_folder(&mut self, cx: &mut Context<Self>) {
        match folder_request(&self.settings.folder.draft) {
            Ok(path) => self.send_folder(path, cx),
            Err(message) => {
                self.settings.folder.refused(message);
                cx.notify();
            }
        }
    }

    /// Leaving the field applies what was typed, as the Settings apply model says.
    pub(super) fn save_folder_on_blur(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.folder.dirty() {
            self.save_folder(cx);
        }
    }

    /// Reset to default: the service stores the default folder it reported.
    pub(super) fn reset_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(default) = self
            .settings
            .folder
            .stored
            .as_ref()
            .map(|value| value.default.clone())
        {
            self.send_folder(default, cx);
        }
    }

    /// Escape: the field shows the stored folder again and the row forgets its last refusal.
    pub(super) fn revert_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.folder.revert();
        let draft = self.settings.folder.draft.clone();
        let field = self.settings_folder.clone();
        field.update(cx, |field, cx| field.set_text(&draft, window, cx));
        cx.notify();
    }

    /// Keeps the field in step with the model: off while the setting cannot change, and showing the
    /// stored folder when the user is not typing in it.
    pub(super) fn sync_folder_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let connected = self.store.read(cx).state().connection.is_connected();
        let reason = folder_reason(connected, &self.settings.folder);
        let draft = self.settings.folder.draft.clone();
        let field = self.settings_folder.clone();
        // set_disabled notifies the field, so it is called only when the state changes.
        let disabled = reason.is_some();
        let changed = disabled != self.settings_folder_disabled;
        self.settings_folder_disabled = disabled;
        field.update(cx, |field, cx| {
            if changed {
                field.set_disabled(disabled, cx);
            }
            if !field.focus_handle(cx).is_focused(window) && field.text() != draft {
                field.set_text(&draft, window, cx);
            }
        });
    }

    /// Stores the folder with the service. The row says "Saved" only once the service answers so.
    fn send_folder(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.settings
                .folder
                .refused("The service is not connected. Nothing was saved.");
            cx.notify();
            return;
        };
        self.settings.folder.begin_save();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = service
                .config_set(ConfigSet {
                    owner: None,
                    key: WORKSPACE_FOLDER_KEY.to_owned(),
                    value: path.into(),
                })
                .await;
            let _ = this.update(cx, |shell, cx| {
                match result {
                    Ok(value) => {
                        shell
                            .settings
                            .folder
                            .saved(FolderValue::from_config(&value));
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(SAVED_FOR).await;
                            let _ = this.update(cx, |shell, cx| {
                                if shell.settings.folder.status == Some(FolderStatus::Saved) {
                                    shell.settings.folder.status = None;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                    }
                    Err(refusal) => shell.settings.folder.refused(folder_refusal_text(&refusal)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
