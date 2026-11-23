//! The workspace start page (D-AG-01, J3): what the Canvas shows for a workspace with no session
//! open. It names the workspace's project and host, the facts the service reported, and the actions
//! that cannot run yet, drawn disabled with the reason. Opening a project from the heading or the
//! placement row uses the project menu of the create form; choosing a row opens that project's
//! workspace or the start page with no workspace.

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, SharedString, Subscription, WeakEntity, Window, div, px,
};
use massorca_protocol::methods::agents::AgentSession;
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::composer::{AccessMode, Composer, ComposerEvent};
use massorca_ui::components::kbd::Kbd;
use massorca_ui::components::model_picker::{
    AgentAvailability, AgentOption, Choice, ModelList, ModelOption, PickerState,
};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::settings_row::{SettingsRow, SettingsSection};
use massorca_ui::components::tooltip::{Tooltip, tooltip_view};
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, SPACE_1, SPACE_2, SPACE_4, SPACE_6, SPACE_8, TYPE_CAPTION,
    TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};
use std::io::Write;

use super::Shell;
use super::workspace_dialogs::{MenuTarget, project_menu};
use crate::launch_options::DebugFocus;
use crate::model::agent_kinds::{KindChoice, session_kind_choices};
use crate::model::agent_sessions::agent_of_kind;
use crate::model::permission_modes::{ModeChoice, start_choices};
use crate::model::regions::Region;
use crate::model::start_page::{
    GIT_LATER, KEY_HINTS, StartMode, StartingSession, TERMINAL_NOTE, VOICE_LATER, WorkspaceStart,
    send_reason, start_params,
};
use crate::model::work_panel::terminal_unavailable;
use crate::model::workspace_flow::ProjectChoice;
use crate::service::Refusal;

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
}

/// A workspace's start page composer, with the subscription that logs its drafts in a debug launch and
/// starts a session when the user sends.
#[derive(Debug)]
pub(super) struct StartComposer {
    pub entity: Entity<Composer>,
    pub mode: StartMode,
    _events: Subscription,
}

/// The start page for one workspace, centred in the Canvas.
pub(super) fn start_page(frame: StartFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let page = &frame.page;

    let project_link = {
        let shell = frame.shell.clone();
        div()
            .id("start-project-link")
            .debug_selector(|| "start-project-link".to_owned())
            .cursor_pointer()
            .underline()
            .child(page.project.clone())
            .on_click(move |_, window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_start_project_menu(window, cx));
            })
    };
    let heading = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .text_style(TYPE_HEADING)
        .text_color(p.text.primary)
        .child("What should we work on in ")
        .child(project_link);

    let placement = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .gap(px(SPACE_2))
        .child(
            Button::new("start-host", page.host)
                .variant(ButtonVariant::Ghost)
                .leading_icon(IconName::Server)
                .disabled(true)
                .tooltip("Hosts other than This PC come later."),
        )
        .child(
            Button::new("start-project", page.project.clone())
                .variant(ButtonVariant::Ghost)
                .trailing_icon(IconName::ChevronDown)
                .disabled(true)
                .tooltip("Change the project from the heading."),
        )
        .child(
            Button::new(
                "start-branch",
                page.branch
                    .clone()
                    .unwrap_or_else(|| "No branch".to_owned()),
            )
            .variant(ButtonVariant::Ghost)
            .leading_icon(IconName::GitBranch)
            .disabled(true)
            .tooltip(GIT_LATER),
        );

    // The real composer: the Chat / Terminal switch sits in its bottom row (A32).
    let composer = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(frame.composer.clone())
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
        .child(placement)
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

/// The start in flight, under the composer until the service answers (D-AG-01 States).
fn starting_line(status: String, theme: &Theme) -> impl IntoElement {
    div()
        .debug_selector(|| "start-status".to_owned())
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.tertiary)
        .child(status)
}

/// A kind's mode entries as the composer's chip shows them; a bypass mode carries the warning glyph.
pub(super) fn access_modes(choices: &[ModeChoice]) -> Vec<AccessMode> {
    choices
        .iter()
        .map(|choice| AccessMode {
            name: choice.name.clone().into(),
            icon: if choice.bypass {
                IconName::TriangleAlert
            } else {
                IconName::Shield
            },
            bypass: choice.bypass,
        })
        .collect()
}

/// The kinds as the picker's rail offers them: one agent each, with the kind as its only row, since the
/// service declares no models yet.
fn picker_agents(choices: &[KindChoice]) -> Vec<AgentOption> {
    choices
        .iter()
        .map(|choice| AgentOption {
            kind: super::rail::agent_kind(agent_of_kind(&choice.id)),
            availability: AgentAvailability::Ready,
            list: ModelList::Loaded,
            models: vec![ModelOption {
                id: choice.id.clone().into(),
                name: choice.name.clone().into(),
                efforts: Vec::new(),
                default_effort: 0,
                fast: false,
            }],
        })
        .collect()
}

/// The key hints under the composer: each key in a keycap, then what it does (D-AG-01).
fn key_hints(theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .gap(px(SPACE_4))
        .children(KEY_HINTS.iter().map(|(key, what)| {
            div()
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
        let mode = shell
            .upgrade()
            .map(|shell| shell.read(cx).start_mode(&workspace))
            .unwrap_or_default();
        let (shell, workspace) = (shell.clone(), workspace.clone());
        SegmentedControl::new(
            "start-mode",
            "start-mode-group",
            vec![
                Segment::icon_text(IconName::MessageSquare, "Chat"),
                Segment::icon_text(IconName::SquareTerminal, "Terminal"),
            ],
        )
        .selected(mode.index())
        .on_change(move |index, _, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.set_start_mode(&workspace, StartMode::from_index(index), cx);
            });
        })
        .into_any_element()
    }
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
        let entity = self
            .start_composers
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
                            shell.start_session(&workspace, prompt, composer.clone(), cx);
                        }
                        _ => {}
                    },
                );
                StartComposer {
                    entity,
                    mode: StartMode::default(),
                    _events: events,
                }
            })
            .entity
            .clone();
        self.refresh_start_composer(&entity, workspace_id, cx);
        entity
    }

    /// Where a session started from `workspace`'s page opens; Chat until the switch says otherwise.
    pub(super) fn start_mode(&self, workspace: &str) -> StartMode {
        self.start_composers
            .get(workspace)
            .map(|composer| composer.mode)
            .unwrap_or_default()
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
        &self,
        composer: &Entity<Composer>,
        workspace_id: &str,
        cx: &mut Context<Self>,
    ) {
        let (choices, connected, kinds) = {
            let state = self.store.read(cx).state();
            (
                session_kind_choices(&state.agent_kinds),
                state.connection.is_connected(),
                state.agent_kinds.clone(),
            )
        };
        composer.update(cx, |composer, cx| {
            composer.set_picker_agents(picker_agents(&choices), cx);
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
        let modes = kinds
            .iter()
            .find(|kind| kind.id.as_str() == chosen.as_ref())
            .map(|kind| access_modes(&start_choices(kind)))
            .unwrap_or_default();
        composer.update(cx, |composer, cx| {
            let keep = if composer.access_modes() == modes.as_slice() {
                composer.access_index()
            } else {
                0
            };
            composer.set_access_modes(modes, keep, cx);
        });
        let kind_chosen = choices
            .iter()
            .any(|choice| choice.id.as_str() == chosen.as_ref());
        let starting = self
            .start_pending
            .as_ref()
            .filter(|pending| pending.workspace == workspace_id);
        let reason = send_reason(connected, kind_chosen, starting).map(SharedString::from);
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
        let chosen = composer
            .read(cx)
            .picker()
            .read(cx)
            .state()
            .choice()
            .model
            .to_string();
        let choices = session_kind_choices(&self.store.read(cx).state().agent_kinds);
        let Some(kind) = choices.into_iter().find(|kind| kind.id == chosen) else {
            return;
        };
        let permission_mode = self
            .store
            .read(cx)
            .state()
            .agent_kinds
            .iter()
            .find(|declared| declared.id == kind.id)
            .and_then(|declared| {
                let index = composer.read(cx).access_index();
                start_choices(declared).get(index)?.id.clone()
            });
        let params = start_params(workspace, &kind.id, prompt, permission_mode);
        let mode = self.start_mode(workspace);
        self.start_pending = Some(StartingSession {
            workspace: workspace.to_owned(),
            kind_name: kind.name,
        });
        let prompt = prompt.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_start(params).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_start(&prompt, composer, mode, answer, cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// The service's answer to a start. In Chat the session opens once the Sidebar lists it; in
    /// Terminal its terminal opens in the Work panel and the page stays. A refusal gives the prompt
    /// back to the composer with its sentence.
    fn finish_start(
        &mut self,
        prompt: &str,
        composer: Entity<Composer>,
        mode: StartMode,
        answer: Result<AgentSession, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.start_pending = None;
        match answer {
            Ok(session) => {
                composer.update(cx, |composer, cx| composer.set_error(None, cx));
                match mode {
                    StartMode::Chat => self.model.open_started_session(session.id),
                    StartMode::Terminal => {
                        self.model.show_region(Region::WorkPanel);
                        if let Some(terminal) = &session.terminal_id {
                            self.reveal_terminal(&session.workspace_id, terminal, cx);
                        }
                    }
                }
            }
            Err(refusal) => composer.update(cx, |composer, cx| {
                composer.set_draft(prompt, cx);
                composer.set_error(Some(refusal.sentence().into()), cx);
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
