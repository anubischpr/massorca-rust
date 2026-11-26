//! The workspace start page (D-AG-01, J3): what the Canvas shows for a workspace with no session
//! open. It names the workspace's project and host, the facts the service reported, and the actions
//! that cannot run yet, drawn disabled with the reason. Opening a project from the heading or the
//! placement row uses the project menu of the create form; choosing a row opens that project's
//! workspace or the start page with no workspace.

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, SharedString, Subscription, WeakEntity, Window, div, px,
};
use massorca_protocol::methods::agents::AgentKind;
use massorca_protocol::methods::extensions::ExtensionItem;
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::composer::{AccessMode, Composer, ComposerEvent};
use massorca_ui::components::invocation_banner::hex_colour;
use massorca_ui::components::kbd::Kbd;
use massorca_ui::components::model_picker::{
    AgentAvailability, AgentBrand, AgentOption, Choice, ModelList, ModelOption, PickerState,
};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl, SegmentedStyle};
use massorca_ui::components::settings_row::{SettingsRow, SettingsSection};
use massorca_ui::components::tooltip::{Tooltip, tooltip_view};
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, SPACE_1, SPACE_2, SPACE_3, SPACE_6, SPACE_8, TYPE_CAPTION,
    TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};
use std::collections::BTreeMap;
use std::io::Write;

use super::Shell;
use super::start_placement::{host_pill, off_pill, pill, pill_focus, placement_card};
use super::workspace_dialogs::{MenuTarget, project_menu};
use crate::launch_options::DebugFocus;
use crate::model::agent_kinds::{KindChoice, session_kind_choices};
use crate::model::agent_models::{choice_kind, declared_model, declared_models, preselected_model};
use crate::model::agent_sessions::agent_of_kind;
use crate::model::home_start::{ADD_A_PROJECT, HOME_START, home_choice};
use crate::model::marketplace::ItemKind;
use crate::model::navigation::MarketplacePage;
use crate::model::permission_modes::{DEFAULT_TAG, ModeChoice, remembered_mode, start_choices};
use crate::model::start_page::{
    CHOOSE_AN_AGENT, ChatAvailability, GIT_LATER, KEY_HINTS, NO_AGENTS_INSTALLED, StartMode,
    StartOptions, StartingSession, TERMINAL_NOTE, VOICE_LATER, WorkspaceStart, fast_reason,
    send_reason, start_options, start_params, start_refusal_sentence,
};
use crate::model::work_panel::terminal_unavailable;
use crate::model::workspace_flow::ProjectChoice;
use crate::service::{Refusal, StartedSession};
use crate::ui_state;
use massorca_protocol::codes;
use massorca_protocol::methods::chat::RunType;

/// What the start page draws: the workspace, its composer, the shell to act through, and whether
/// the service is connected, which "Open terminal" needs.
pub(super) struct StartFrame {
    pub page: WorkspaceStart,
    pub composer: Entity<Composer>,
    pub shell: WeakEntity<Shell>,
    pub connected: bool,
    /// The start in flight for this workspace, shown under the composer.
    pub starting: Option<String>,
    /// Where the session the composer starts opens.
    pub mode: StartMode,
    /// The service refused because the data folder is not secured.
    pub profile_unsafe: bool,
}

/// The banner a data folder that is not secured gets: the service refuses every call until the
/// owner runs the CLI, since no protocol method secures it (`profile.unsafe_location`).
pub(super) const PROFILE_UNSAFE_TEXT: &str = "This data folder is not secured, so the service refuses every call. Run `massorca profile secure --yes` in a terminal, then try again.";

/// The banner on both start pages while the last refusal was `profile.unsafe_location`.
pub(super) fn profile_banner() -> impl IntoElement {
    div()
        .debug_selector(|| "start-profile-banner".to_owned())
        .child(
            Banner::new(
                "start-profile-banner",
                BannerVariant::Warning,
                PROFILE_UNSAFE_TEXT,
            )
            .title("Secure the data folder"),
        )
}

/// A workspace's start page composer, with the subscription that logs its drafts in a debug launch and
/// starts a session when the user sends.
#[derive(Debug)]
pub(super) struct StartComposer {
    pub entity: Entity<Composer>,
    pub mode: StartMode,
    /// The (kind, mode) the person last picked on the chip; the only mode a start can send.
    pub picked: Option<(String, String)>,
    /// Whether Chat is available for the picked agent, as the page's last frame read it (decision 0101, D11).
    pub chat: ChatAvailability,
    _events: Subscription,
}

/// The start page for one workspace, centred in the Canvas.
pub(super) fn start_page(frame: StartFrame, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let page = &frame.page;

    let project_link = {
        let shell = frame.shell.clone();
        project_title_link(
            "start-project-link",
            page.project.clone(),
            &theme,
            move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_start_project_menu(window, cx));
            },
        )
    };
    let heading = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .text_style(TYPE_HEADING)
        .text_color(p.text.primary)
        .child("What should we work on in ")
        .child(project_link)
        .child("?");

    let host = host_pill(
        "start-host",
        page.host,
        &theme,
        pill_focus("start-host", window, cx),
    );
    let project = {
        let shell = frame.shell.clone();
        pill(
            "start-project",
            IconName::Folder,
            page.project.clone(),
            None,
            true,
            &theme,
            pill_focus("start-project", window, cx),
        )
        .on_click(move |_, window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_start_project_menu(window, cx));
        })
    };
    let branch = off_pill(
        "start-branch",
        IconName::GitBranch,
        page.branch
            .clone()
            .unwrap_or_else(|| "No branch".to_owned()),
        None,
        GIT_LATER,
        &theme,
        pill_focus("start-branch", window, cx),
    );
    let placement = placement_card(host, project, branch, &theme);
    // The real composer: the Chat / Terminal switch sits in its bottom row (A32). The placement card joins its top edge.
    let composer = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .child(placement)
                .child(frame.composer.clone()),
        )
        .when(frame.mode == StartMode::Terminal, |column| {
            column.child(
                div()
                    .debug_selector(|| "start-terminal-note".to_owned())
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.secondary)
                    .child(TERMINAL_NOTE),
            )
        })
        .child(key_hints(&theme))
        .when_some(frame.starting.clone(), |column, status| {
            column.child(starting_line(status, &theme))
        });

    let facts = SettingsSection::new("start-facts", "Workspace")
        .row(fact_row("start-name", "Name", page.name.clone()))
        .row(fact_row(
            "start-project-fact",
            "Project",
            page.project.clone(),
        ))
        .row(fact_row("start-path", "Path", page.path.clone()))
        .row(fact_row(
            "start-branch-fact",
            "Branch",
            page.branch.clone().unwrap_or_else(|| "None".to_owned()),
        ))
        .row(fact_row("start-state", "State", page.state.to_owned()));

    let mut column = div()
        .debug_selector(|| "start-column".to_owned())
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .child(div().flex().justify_center().child(rose_logo()))
        .child(heading)
        .when(frame.profile_unsafe, |column| {
            column.child(profile_banner())
        })
        .child(composer);
    if page.missing {
        column = column.child(
            Banner::new(
                "start-missing",
                BannerVariant::Warning,
                "This folder is missing from disk.",
            )
            .detail(page.path.clone()),
        );
    }
    column = column.child(facts).child(actions(&frame));

    div()
        .id("start-page")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .p(px(SPACE_8))
        .child(column)
        .into_any_element()
}

/// The project's name in a page heading: dotted underlined, and clickable to change the project (D-AG-01).
pub(super) fn project_title_link(
    id: &'static str,
    name: String,
    theme: &Theme,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .cursor_pointer()
        .border_b_1()
        .border_dashed()
        .border_color(theme.palette.text.tertiary)
        .child(name)
        .on_click(move |_, window, cx| on_click(window, cx))
}

/// The start in flight, under the composer until the service answers (D-AG-01 States).
pub(super) fn starting_line(status: String, theme: &Theme) -> impl IntoElement {
    div()
        .debug_selector(|| "start-status".to_owned())
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.tertiary)
        .child(status)
}

/// A kind's mode entries as the composer's chip shows them; a bypass mode carries the warning glyph.
/// The mode a start sends: the entry the chip shows, and only when the chip was built from exactly these choices.
/// A chip showing another list, or an index past its end, sends no mode, so the vendor CLI starts in its default.
pub(super) fn sent_mode(
    choices: &[ModeChoice],
    shown: &[AccessMode],
    index: usize,
) -> Option<String> {
    if access_modes(choices).as_slice() != shown {
        return None;
    }
    choices.get(index)?.id.clone()
}

pub(super) fn access_modes(choices: &[ModeChoice]) -> Vec<AccessMode> {
    choices
        .iter()
        .map(|choice| AccessMode {
            name: choice.name.clone().into(),
            icon: choice.icon,
            bypass: choice.bypass,
        })
        .collect()
}

/// The secondary text of each of a kind's mode entries, in the same order as `access_modes`.
pub(super) fn access_details(choices: &[ModeChoice]) -> Vec<Option<SharedString>> {
    choices
        .iter()
        .map(|choice| choice.detail.clone().map(SharedString::from))
        .collect()
}

/// The tag of each of a kind's mode entries, in the same order as `access_modes`: the declared default's row only.
pub(super) fn access_tags(choices: &[ModeChoice]) -> Vec<Option<SharedString>> {
    choices
        .iter()
        .map(|choice| choice.default.then(|| SharedString::from(DEFAULT_TAG)))
        .collect()
}

/// The installed kinds as the picker's rail offers them: each kind's declared models with the levels each accepts. A
/// kind that declares no models is its own one row, with the levels the kind declares. Each agent carries the brand
/// colours its Marketplace listing gives.
pub(super) fn picker_agents(
    kinds: &[AgentKind],
    installed: &[KindChoice],
    brands: &BTreeMap<String, AgentBrand>,
) -> Vec<AgentOption> {
    installed
        .iter()
        .map(|choice| {
            let kind = kinds.iter().find(|kind| kind.id == choice.id);
            let declared = kind.map(declared_models).unwrap_or_default();
            let models = if declared.is_empty() {
                let (efforts, default_effort) = kind.map_or_else(|| (Vec::new(), 0), kind_levels);
                vec![ModelOption {
                    id: choice.id.clone().into(),
                    name: choice.name.clone().into(),
                    efforts,
                    default_effort,
                    fast: false,
                }]
            } else {
                declared
                    .into_iter()
                    .map(|model| ModelOption {
                        id: model.id.into(),
                        name: model.label.into(),
                        efforts: model
                            .efforts
                            .into_iter()
                            .map(|effort| SharedString::from(effort.label))
                            .collect(),
                        default_effort: model.default_effort,
                        fast: false,
                    })
                    .collect()
            };
            AgentOption {
                kind: super::rail::agent_kind(agent_of_kind(&choice.id)),
                availability: AgentAvailability::Ready,
                list: ModelList::Loaded,
                models,
                brand: brands.get(&choice.id).copied(),
            }
        })
        .collect()
}

/// A kind's own levels as its picker row lists them, in declared order, and the index of its declared default.
fn kind_levels(kind: &AgentKind) -> (Vec<SharedString>, usize) {
    let Some(effort) = &kind.effort else {
        return (Vec::new(), 0);
    };
    let default = effort
        .default
        .as_deref()
        .and_then(|id| effort.levels.iter().position(|level| level.id == id))
        .unwrap_or(0);
    let labels = effort
        .levels
        .iter()
        .map(|level| SharedString::from(level.label.clone()))
        .collect();
    (labels, default)
}

/// The brand colours of each agent kind, from the Marketplace listing of the package that declares it, the installed
/// package before the catalogue's. The first listed colour starts the fill and the second, when given, ends it.
pub(super) fn agent_brands(
    installed: &[ExtensionItem],
    catalogue: &[ExtensionItem],
) -> BTreeMap<String, AgentBrand> {
    let mut brands = BTreeMap::new();
    for entry in installed.iter().chain(catalogue) {
        if entry.listing.category != "agents" {
            continue;
        }
        let (Some(kind), Some(banner)) = (entry.agents.first(), entry.listing.banner.as_ref())
        else {
            continue;
        };
        let Some(first) = banner.colors.first().and_then(|text| hex_colour(text)) else {
            continue;
        };
        let second = banner.colors.get(1).and_then(|text| hex_colour(text));
        brands
            .entry(kind.id.clone())
            .or_insert(AgentBrand { first, second });
    }
    brands
}

/// The model and effort the picker chose, as `agent.start` takes them: the declared id of the chosen model and of the
/// level at the chosen index. A row no kind declares as a model sends neither.
pub(super) fn picked_options(
    picker: &PickerState,
    offered: bool,
    kinds: &[AgentKind],
) -> StartOptions {
    let chosen = picker.choice().effort;
    if let Some((_, model)) = declared_model(kinds, &picker.choice().model) {
        let effort = chosen
            .and_then(|index| model.efforts.get(index))
            .map(|effort| effort.id.as_str());
        return start_options(offered, Some(&model.id), effort);
    }
    // A kind that declares levels but no model offers them on its own row, and sends the level without a model.
    let Some(kind) = kinds.iter().find(|kind| kind.id == picker.choice().model) else {
        return StartOptions::default();
    };
    let effort = kind
        .effort
        .as_ref()
        .zip(chosen)
        .and_then(|(effort, index)| effort.levels.get(index))
        .map(|level| level.id.as_str());
    start_options(offered, None, effort)
}

/// The key hints under the composer, left-aligned at its edge: each key in a keycap, then what it does (D-AG-01).
pub(super) fn key_hints(theme: &Theme) -> impl IntoElement {
    div()
        .debug_selector(|| "start-key-hints".to_owned())
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(SPACE_3))
        .children(KEY_HINTS.iter().map(|(key, what)| {
            let selector = format!("start-key-hint-{key}");
            div()
                .debug_selector(move || selector.clone())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(SPACE_1))
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child(Kbd::new(*key))
                .child(*what)
        }))
}

/// The Chat / Terminal switch of the composer's bottom row: where the session it starts opens.
fn start_mode_switch(
    shell: WeakEntity<Shell>,
    workspace: String,
) -> impl Fn(&mut Window, &mut App) -> AnyElement + 'static {
    move |_: &mut Window, cx: &mut App| -> AnyElement {
        let (mode, chat) = shell
            .upgrade()
            .map(|shell| {
                let shell = shell.read(cx);
                (
                    shell.start_mode(&workspace, cx),
                    shell.chat_availability(&workspace, cx),
                )
            })
            .unwrap_or((StartMode::Terminal, ChatAvailability::new(false, None)));
        let (shell, workspace) = (shell.clone(), workspace.clone());
        SegmentedControl::new("start-mode", "start-mode-group", mode_segments(&chat))
            .selected(mode.index())
            .style(SegmentedStyle::Sunken)
            .on_change(move |index, _, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.set_start_mode(&workspace, StartMode::from_index(index), cx);
                });
            })
            .into_any_element()
    }
}

/// The switch's two segments. Chat is off, with the reason `chat` gives, while Chat is not available.
pub(super) fn mode_segments(chat: &ChatAvailability) -> Vec<Segment> {
    let segment = Segment::icon_text(IconName::MessageSquare, "Chat");
    let segment = match chat.reason() {
        Some(reason) => segment.disabled(true).tooltip(reason.to_owned()),
        None => segment,
    };
    vec![
        segment,
        Segment::icon_text(IconName::SquareTerminal, "Terminal"),
    ]
}

/// One read-only fact of the workspace, as a settings row with its value on the right.
fn fact_row(id: &'static str, label: &'static str, value: String) -> SettingsRow {
    SettingsRow::new(id, label).control(fact_value(id, &value))
}

/// A read-only value inside the control column. It shrinks to that column rather than widening the
/// row: a path gives way in the middle and keeps its last segment; the full text is the tooltip
/// (SettingsRow.md, long content).
fn fact_value(id: &'static str, value: &str) -> impl IntoElement {
    let (head, tail) = split_last_segment(value);
    div()
        .id(SharedString::from(format!("{id}-value")))
        .debug_selector(move || format!("{id}-value"))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .justify_end()
        .text_body()
        .tooltip(tooltip_view(Tooltip::new(value.to_owned())))
        .child(
            div()
                .debug_selector(move || format!("{id}-value-head"))
                .min_w_0()
                .truncate()
                .child(head.to_owned()),
        )
        .child(
            div()
                .debug_selector(move || format!("{id}-value-tail"))
                .flex_none()
                .max_w_full()
                .truncate()
                .child(tail.to_owned()),
        )
}

/// Splits a value before its last path segment: the head may shrink, the tail (the last segment
/// with its separator) stays whole. A value without a separator is all tail.
pub(super) fn split_last_segment(value: &str) -> (&str, &str) {
    value.split_at(value.rfind(['/', '\\']).unwrap_or(0))
}

/// "Open terminal": shows the Work panel with a new terminal in this workspace. Off, with its
/// reason, while the service is not connected.
fn open_terminal_button(frame: &StartFrame) -> impl IntoElement {
    let (shell, workspace) = (frame.shell.clone(), frame.page.id.clone());
    let button = Button::new("start-terminal", "Open terminal")
        .variant(ButtonVariant::Secondary)
        .leading_icon(IconName::SquareTerminal);
    let button = match terminal_unavailable(true, frame.connected) {
        Some(reason) => button.disabled(true).tooltip(reason),
        None => button.on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.open_terminal(workspace.clone(), window, cx);
            });
        }),
    };
    div()
        .debug_selector(|| "start-terminal".to_owned())
        .child(button)
}

/// The actions the page will have. Removing the workspace uses the existing flow; the rest wait
/// for the layer they need.
fn actions(frame: &StartFrame) -> AnyElement {
    let shell = frame.shell.clone();
    let id = frame.page.id.clone();
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(SPACE_2))
        .child(open_terminal_button(frame))
        .child({
            let shell = shell.clone();
            Button::new("start-source-control", "Source control")
                .variant(ButtonVariant::Secondary)
                .leading_icon(IconName::GitBranch)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.open_source_control(cx));
                })
        })
        .child(
            Button::new("start-remove", "Delete workspace…")
                .variant(ButtonVariant::Ghost)
                .leading_icon(IconName::Trash2)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.open_remove(&id, cx));
                }),
        )
        .into_any_element()
}

impl Shell {
    /// The project menu of the start page: the rows the create form uses, with this workspace's
    /// project checked. It opens where the pointer is.
    pub(super) fn open_start_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state().clone();
        let current = self
            .model
            .selected_workspace()
            .and_then(|id| crate::model::start_page::workspace_start(&state, id))
            .map(|page| ProjectChoice::Project(page.project_id));
        let (entries, choices) = project_menu(&state.projects, current.as_ref());
        self.open_menu(
            entries,
            MenuTarget::StartProject(choices),
            OverlayLayer::Dialog,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// Choosing a project opens its first workspace; "No project" opens the start page with no
    /// workspace. Nothing is created.
    pub(super) fn open_start_project(&mut self, choice: ProjectChoice, cx: &mut Context<Self>) {
        let workspace = match choice {
            ProjectChoice::Project(id) => {
                crate::model::start_page::first_workspace_of(self.store.read(cx).state(), &id)
            }
            ProjectChoice::NoProject => None,
        };
        self.model.select_workspace(workspace);
        cx.notify();
    }

    /// "New session" in a workspace's menu: that workspace's start page, with no session open.
    pub(super) fn open_new_session(&mut self, workspace: &str, cx: &mut Context<Self>) {
        self.model.close_session();
        self.model.open_workspace(workspace);
        cx.notify();
    }

    /// The composer of a workspace's start page, created on first use and then kept, so the
    /// draft stays while the page redraws or the workspace is shown again. Each frame keeps its
    /// agents and Send in step with the store and the start in flight.
    pub(super) fn start_composer(
        &mut self,
        workspace_id: &str,
        cx: &mut Context<Self>,
    ) -> Entity<Composer> {
        let log_drafts = self.debug_focus == Some(DebugFocus::Composer);
        let entity =
            self.start_composers
                .entry(workspace_id.to_owned())
                .or_insert_with(|| {
                    let workspace = workspace_id.to_owned();
                    let switch = start_mode_switch(cx.entity().downgrade(), workspace.clone());
                    let picker = PickerState::new(
                        Vec::new(),
                        Choice {
                            model: SharedString::default(),
                            effort: None,
                            fast: false,
                        },
                    );
                    let entity = cx.new(|cx| {
                        Composer::new(picker, Vec::new(), cx)
                            .leading_control(switch)
                            .voice_unavailable(VOICE_LATER)
                            .sends_empty()
                            .joined_top(true)
                    });
                    let events = cx.subscribe(
                        &entity,
                        move |shell, composer, event: &ComposerEvent, cx| match event {
                            ComposerEvent::DraftChanged if log_drafts => {
                                let draft = composer.read(cx).draft(cx);
                                let _ = writeln!(
                                    std::io::stderr(),
                                    "massorca-desktop: debug composer draft={draft:?}"
                                );
                            }
                            ComposerEvent::Send(prompt) => {
                                // The start page clears the prompt as it sends and gives it back on a refusal.
                                composer.update(cx, |composer, cx| composer.set_draft("", cx));
                                if workspace == HOME_START {
                                    shell.start_from_home(prompt, composer.clone(), cx);
                                } else {
                                    shell.start_session(&workspace, prompt, composer.clone(), cx);
                                }
                            }
                            ComposerEvent::OpenMarketplace => shell
                                .open_marketplace(MarketplacePage::Category(ItemKind::Agent), cx),
                            ComposerEvent::AccessChanged(index) => {
                                shell.record_start_pick(&workspace, *index, composer.clone(), cx);
                                shell.remember_start_mode(&composer, *index, cx);
                            }
                            ComposerEvent::ModelChanged(_) => {
                                // Chat follows the picked agent: the page refreshes it, the composer redraws.
                                composer.update(cx, |_, cx| cx.notify());
                                cx.notify();
                            }
                            _ => {}
                        },
                    );
                    StartComposer {
                        entity,
                        mode: StartMode::default(),
                        picked: None,
                        // Set by `refresh_start_composer`, which runs right after this composer is created.
                        chat: ChatAvailability::Available,
                        _events: events,
                    }
                })
                .entity
                .clone();
        self.refresh_start_composer(&entity, workspace_id, cx);
        entity
    }

    /// Whether Chat is available for `workspace`'s picked agent, as the page's last frame read it (decision 0101).
    pub(super) fn chat_availability(&self, workspace: &str, cx: &App) -> ChatAvailability {
        self.start_composers.get(workspace).map_or_else(
            || ChatAvailability::new(self.store.read(cx).state().agent_chat, None),
            |start| start.chat.clone(),
        )
    }

    /// Where a session started from `workspace`'s page opens: the switch's choice (Chat until it says otherwise),
    /// or Terminal while Chat is not available for the picked agent.
    pub(super) fn start_mode(&self, workspace: &str, cx: &App) -> StartMode {
        self.start_composers
            .get(workspace)
            .map(|composer| composer.mode)
            .unwrap_or_default()
            .offered(self.chat_availability(workspace, cx).is_available())
    }

    /// The switch's choice for `workspace`, kept with its composer so it survives redraws.
    pub(super) fn set_start_mode(
        &mut self,
        workspace: &str,
        mode: StartMode,
        cx: &mut Context<Self>,
    ) {
        if let Some(composer) = self.start_composers.get_mut(workspace) {
            composer.mode = mode;
            composer.entity.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    /// Keeps a start composer in step with the store: the kinds it offers, and why Send is off.
    fn refresh_start_composer(
        &mut self,
        composer: &Entity<Composer>,
        workspace_id: &str,
        cx: &mut Context<Self>,
    ) {
        let (choices, connected, kinds, agent_fast, brands) = {
            let state = self.store.read(cx).state();
            (
                session_kind_choices(&state.agent_kinds),
                state.connection.is_connected(),
                state.agent_kinds.clone(),
                state.agent_fast,
                {
                    let (installed, catalogue) = state.extension_lists();
                    agent_brands(installed, catalogue)
                },
            )
        };
        let installed_kinds: Vec<AgentKind> = kinds
            .iter()
            .filter(|kind| choices.iter().any(|choice| choice.id == kind.id))
            .cloned()
            .collect();
        let preselect = preselected_model(&installed_kinds).map(SharedString::from);
        composer.update(cx, |composer, cx| {
            composer.set_picker_agents(picker_agents(&kinds, &choices, &brands), preselect, cx);
            // The picker says so and links to the Marketplace; a list not read yet is not "none installed".
            composer.set_no_agents_installed(connected && choices.is_empty(), cx);
            // Fast is off on every model until the service offers its speed (`AGENT_FAST_CAPABILITY`).
            composer.set_picker_fast_reason(fast_reason(agent_fast).map(SharedString::from), cx);
        });
        let chosen = composer
            .read(cx)
            .picker()
            .read(cx)
            .state()
            .choice()
            .model
            .clone();
        // The mode chip follows the chosen kind; a pick stays while the kind does.
        let chosen_kind = choice_kind(&kinds, chosen.as_ref());
        let (modes, details, tags, remembered) =
            chosen_kind.map_or_else(Default::default, |kind| {
                let choices = start_choices(kind);
                // The person's last explicit pick for this kind comes back, while the kind still offers it.
                let saved = crate::ui_state::saved_start_mode(cx, &kind.id);
                // A bypass mode is never restored from memory: it is sent only when picked on this composer.
                let remembered = remembered_mode(kind, saved.as_deref()).and_then(|id| {
                    let index = choices
                        .iter()
                        .position(|choice| choice.id.as_deref() == Some(id) && !choice.bypass)?;
                    Some((index, (kind.id.clone(), id.to_owned())))
                });
                (
                    access_modes(&choices),
                    access_details(&choices),
                    access_tags(&choices),
                    remembered,
                )
            });
        let changed = composer.update(cx, |composer, cx| {
            let changed = composer.access_modes() != modes.as_slice();
            let keep = if changed {
                remembered.as_ref().map_or(0, |(index, _)| *index)
            } else {
                composer.access_index()
            };
            composer.set_access_modes(modes, keep, cx);
            composer.set_access_details(details, cx);
            composer.set_access_tags(tags, cx);
            changed
        });
        // A restored chip is the person's pick, so the chip and the start agree; a reset chip sends no mode.
        if changed && let Some(start) = self.start_composers.get_mut(workspace_id) {
            start.picked = remembered.map(|(_, picked)| picked);
        }
        // Chat follows the picked agent: its chat support, or no agent picked yet.
        let chat = {
            let state = self.store.read(cx).state();
            let agent = choice_kind(&state.agent_kinds, chosen.as_ref());
            ChatAvailability::new(
                state.agent_chat,
                agent.map(|kind| (kind.name.as_str(), state.agent_kind_chat.get(&kind.id))),
            )
        };
        if let Some(start) = self.start_composers.get_mut(workspace_id) {
            start.chat = chat;
        }
        let kind_chosen =
            chosen_kind.is_some_and(|kind| choices.iter().any(|choice| choice.id == kind.id));
        let starting = self
            .start_pending
            .as_ref()
            .filter(|pending| pending.workspace == workspace_id);
        let no_project = workspace_id == HOME_START
            && home_choice(
                &self.store.read(cx).state().projects,
                self.home_project.as_ref(),
            )
            .is_none();
        let reason = send_reason(connected, !choices.is_empty(), kind_chosen, starting)
            .or_else(|| no_project.then(|| ADD_A_PROJECT.to_owned()))
            .map(SharedString::from);
        composer.update(cx, |composer, cx| composer.set_send_unavailable(reason, cx));
    }

    /// Sends the first prompt of a new session with the chosen kind. Nothing shows as started until the
    /// service answers.
    fn start_session(
        &mut self,
        workspace: &str,
        prompt: &str,
        composer: Entity<Composer>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let (kind, permission_mode) = match self.chosen_start(&composer, cx) {
            Ok(chosen) => chosen,
            Err(refusal) => return refuse_send(prompt, &composer, refusal, cx),
        };
        let options = self.chosen_options(&composer, cx);
        let grid = self.start_grid(cx);
        let params = start_params(
            Some(workspace),
            &kind.id,
            prompt,
            permission_mode,
            options,
            grid,
        );
        let chat = self.start_mode(workspace, cx) == StartMode::Chat;
        self.start_pending = Some(StartingSession {
            workspace: workspace.to_owned(),
            kind_name: kind.name,
        });
        let prompt = prompt.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.start_session(params, chat).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_start(&prompt, composer, answer, cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// Remembers the explicit mode picked on the chip of `composer`'s kind; picking the CLI's default clears it.
    fn remember_start_mode(
        &self,
        composer: &Entity<Composer>,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let chosen = composer
            .read(cx)
            .picker()
            .read(cx)
            .state()
            .choice()
            .model
            .to_string();
        let Some(declared) = choice_kind(&self.store.read(cx).state().agent_kinds, &chosen) else {
            return;
        };
        let mode = start_choices(declared)
            .get(index)
            .and_then(|choice| choice.id.clone());
        let kind = declared.id.clone();
        ui_state::remember_start_mode(cx, &kind, mode.as_deref());
    }

    /// The model and effort `composer`'s picker chose, sent only when the service offers them.
    pub(super) fn chosen_options(&self, composer: &Entity<Composer>, cx: &App) -> StartOptions {
        let state = self.store.read(cx).state();
        picked_options(
            composer.read(cx).picker().read(cx).state(),
            state.agent_options,
            &state.agent_kinds,
        )
    }

    /// The composer's chosen kind, and the permission mode its access chip picked; or the refusal Send
    /// shows when no agent is installed or none is chosen.
    pub(super) fn chosen_start(
        &self,
        composer: &Entity<Composer>,
        cx: &App,
    ) -> Result<(KindChoice, Option<String>), String> {
        let chosen = composer
            .read(cx)
            .picker()
            .read(cx)
            .state()
            .choice()
            .model
            .to_string();
        let kinds = &self.store.read(cx).state().agent_kinds;
        let installed = session_kind_choices(kinds);
        if installed.is_empty() {
            return Err(NO_AGENTS_INSTALLED.to_owned());
        }
        let kind = choice_kind(kinds, &chosen)
            .and_then(|declared| installed.into_iter().find(|kind| kind.id == declared.id))
            .ok_or_else(|| CHOOSE_AN_AGENT.to_owned())?;
        // `session_kind_choices` lists only kinds that can start, so an unavailable kind is refused above.
        let shown = kinds
            .iter()
            .find(|declared| declared.id == kind.id)
            .and_then(|declared| {
                let composer = composer.read(cx);
                sent_mode(
                    &start_choices(declared),
                    composer.access_modes(),
                    composer.access_index(),
                )
            });
        // A mode goes out only when the person picked it for this kind and the chip still shows it.
        let picked = self
            .start_composers
            .values()
            .find(|start| start.entity == *composer)
            .and_then(|start| start.picked.as_ref())
            .filter(|(picked_kind, _)| *picked_kind == kind.id)
            .map(|(_, mode)| mode.clone());
        let permission_mode = shown.filter(|mode| picked.as_ref() == Some(mode));
        Ok((kind, permission_mode))
    }

    /// Records the chip entry the person chose on a start composer, by mode id; the CLI's default clears it.
    fn record_start_pick(
        &mut self,
        workspace: &str,
        index: usize,
        composer: Entity<Composer>,
        cx: &mut Context<Self>,
    ) {
        let chosen = composer
            .read(cx)
            .picker()
            .read(cx)
            .state()
            .choice()
            .model
            .to_string();
        let picked =
            choice_kind(&self.store.read(cx).state().agent_kinds, &chosen).and_then(|kind| {
                let mode = start_choices(kind).get(index)?.id.clone()?;
                Some((kind.id.clone(), mode))
            });
        if let Some(start) = self.start_composers.get_mut(workspace) {
            start.picked = picked;
        }
    }

    /// Keeps the data-folder banner in step with the last answer: on after `profile.unsafe_location`,
    /// off after any call goes through.
    pub(super) fn note_refusal(&mut self, refusal: Option<&Refusal>) {
        match refusal {
            None => self.profile_unsafe = false,
            Some(refusal) if refusal.code() == Some(codes::PROFILE_UNSAFE_LOCATION) => {
                self.profile_unsafe = true;
            }
            Some(_) => {}
        }
    }

    /// The service's answer to a start. The session opens in the Canvas once the Sidebar lists it; a terminal session's
    /// terminal then takes the keyboard, and a chat session's composer does. A chat the service could not run started
    /// as a terminal session, which says why above it. A refusal gives the prompt back to the composer with its sentence.
    pub(super) fn finish_start(
        &mut self,
        prompt: &str,
        composer: Entity<Composer>,
        answer: Result<StartedSession, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.start_pending = None;
        self.note_refusal(answer.as_ref().err());
        match answer {
            Ok(started) => {
                composer.update(cx, |composer, cx| composer.set_error(None, cx));
                // Both arms open the started session in the Canvas (owner ruling 2026-11-25): the session the person
                // started is what they see.
                let session = started.session;
                if session.run == RunType::Chat {
                    self.focus_started_chat = Some(session.id.clone());
                } else {
                    self.focus_started_terminal = session.terminal_id.clone();
                }
                if let Some(why) = started.fell_back {
                    self.session_notices.insert(
                        session.id.clone(),
                        format!("Started a terminal session instead of a chat: {why}"),
                    );
                }
                self.model.open_started_session(session.id);
            }
            Err(refusal) => composer.update(cx, |composer, cx| {
                composer.set_draft(prompt, cx);
                composer.set_error(Some(start_refusal_sentence(&refusal).into()), cx);
            }),
        }
        cx.notify();
    }

    /// Puts the focus in the selected workspace's start page composer (the debug focus).
    pub(super) fn focus_start_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.model.selected_workspace().map(str::to_owned) else {
            return;
        };
        let composer = self.start_composer(&id, cx);
        composer.update(cx, |composer, cx| composer.focus(window, cx));
    }
}

/// Send was refused before any call: the prompt goes back with the reason above the composer.
pub(super) fn refuse_send(
    prompt: &str,
    composer: &Entity<Composer>,
    refusal: String,
    cx: &mut Context<Shell>,
) {
    composer.update(cx, |composer, cx| {
        composer.set_draft(prompt, cx);
        composer.set_error(Some(refusal.into()), cx);
    });
    cx.notify();
}
