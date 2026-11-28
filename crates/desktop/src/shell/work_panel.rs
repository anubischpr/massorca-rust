//! The Work panel's strip and panes on the Canvas (D-SH-17, WorkPanelTabs.md, TerminalSurface.md).
//! The strip lists the terminals the store holds for the selected workspace. A selected terminal
//! with a view shows its TerminalSurface, attached through the bridge. Closing a running terminal's
//! tab asks first; "Keep running" drops the view and the process goes on in the service (decision
//! 0033), so its tab stays and "Show terminal" attaches it again.

use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, AsyncApp, Context, Entity, Task, WeakEntity, Window, div, px};
use massorca_client::AttachUpdate;
use massorca_protocol::methods::terminals::Terminal;
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::components::terminal_surface::TerminalSurface;
use massorca_ui::components::terminal_surface::feed::FeedOutcome;
use massorca_ui::components::terminal_surface::grid_paint;
use massorca_ui::components::work_panel_tabs::{
    WorkPanelTabs, WorkTab, WorkTabKind, WorkTabsAction,
};
use massorca_ui::tokens::{SIZE_CONTROL_DEFAULT, SPACE_2, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled, Theme};
use tokio::sync::watch;

use super::Shell;
use super::session_grid::pane_grid;
use crate::model::regions::Region;
use crate::model::session_ended::is_agent_run_terminal;
use crate::model::terminals::{new_terminal_params, shell_setting};
use crate::model::work_panel::{
    BROWSER_LATER, CloseStep, MARKDOWN_LATER, MOBILE_LATER, NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS,
    TerminalStrip, close_step, detached_text, new_terminal_grid, status_banner, terminal_strip,
    terminal_unavailable,
};
use crate::service::{
    AttachRole, ConnectionState, Refusal, ServiceHandle, StoreState, StreamEnd, TerminalStream,
    TerminalStreamItem, feed_events,
};

/// The shell new terminals start (SET-01); empty uses the host default.
const TERMINAL_SHELL_KEY: &str = "terminal.shell";

/// One terminal's view: its surface, and the task that follows it through the bridge. Dropping
/// the view detaches; the terminal keeps running.
#[derive(Debug)]
pub(super) struct TerminalView {
    pub(super) surface: Entity<TerminalSurface>,
    _follow: Task<()>,
    /// Why the attach was refused; the pane offers Retry until an attach works.
    pub(super) failure: Option<String>,
}

/// What the Work panel draws this frame, gathered by the shell.
pub(super) struct WorkFrame {
    shell: WeakEntity<Shell>,
    tabs: Vec<WorkTab>,
    selected: usize,
    unavailable: Vec<(WorkTabKind, &'static str)>,
    body: WorkBody,
    /// A refused call, under the strip: its title and its reason.
    notice: Option<(String, String)>,
}

/// How many frames a new terminal waits for the Work panel's pane to paint before it opens at the fixed size.
pub(super) const PANE_WAIT_FRAMES: u8 = 3;

/// The body under the strip, for the selected tab.
enum WorkBody {
    /// No terminal to show: the strip alone, with its empty state when it has no tabs.
    Empty,
    /// A new terminal waits for the pane to paint its size: a blank pane, measured by the probe.
    Measuring { size: Rc<Cell<Option<(f32, f32)>>> },
    /// The selected terminal has no view; "Show terminal" attaches it.
    Detached {
        workspace: String,
        terminal: String,
        text: &'static str,
        banner: Option<&'static str>,
    },
    /// The selected terminal's pane, or the refusal of its attach.
    Pane {
        workspace: String,
        terminal: String,
        surface: Entity<TerminalSurface>,
        banner: Option<&'static str>,
        failure: Option<String>,
        /// Where the pane records its size as it paints (`Shell::work_panel_size`).
        size: Rc<Cell<Option<(f32, f32)>>>,
    },
}

/// What one attach update left to do.
pub(super) enum Step {
    Continue,
    /// The emulator lost its place (a gap or a broken item): attach again without an offset.
    Reattach,
}

/// How one attach ended.
enum Flow {
    /// The link dropped: attach again once it is back.
    Attach,
    Done,
}

impl Shell {
    /// The Work panel as this frame draws it: the selected workspace's strip, and the body of its
    /// selected terminal.
    pub(super) fn work_frame(&self, shell: WeakEntity<Shell>, cx: &App) -> WorkFrame {
        let state = self.store.read(cx).state();
        let workspace = self.model.selected_workspace().map(str::to_owned);
        let strip = match &workspace {
            Some(id) => terminal_strip(state, id, self.model.terminal_selection()),
            None => TerminalStrip::default(),
        };
        let mut unavailable = Vec::new();
        if let Some(reason) =
            terminal_unavailable(workspace.is_some(), state.connection.is_connected())
        {
            unavailable.push((WorkTabKind::Terminal, reason));
        }
        unavailable.extend([
            (WorkTabKind::Browser, BROWSER_LATER),
            (WorkTabKind::Markdown, MARKDOWN_LATER),
            (WorkTabKind::MobileEmulator, MOBILE_LATER),
        ]);
        let body = if self.pane_waits > 0 {
            WorkBody::Measuring {
                size: self.work_panel_size.clone(),
            }
        } else {
            match (
                workspace,
                strip.selected.and_then(|index| strip.tabs.get(index)),
            ) {
                (Some(workspace), Some(tab)) => match self.terminal_views.get(&tab.id) {
                    Some(view) => WorkBody::Pane {
                        workspace,
                        terminal: tab.id.clone(),
                        surface: view.surface.clone(),
                        banner: status_banner(tab.status),
                        failure: view.failure.clone(),
                        size: self.work_panel_size.clone(),
                    },
                    None => WorkBody::Detached {
                        workspace,
                        terminal: tab.id.clone(),
                        text: detached_text(tab.status),
                        banner: status_banner(tab.status),
                    },
                },
                _ => WorkBody::Empty,
            }
        };
        WorkFrame {
            shell,
            tabs: strip
                .tabs
                .iter()
                .map(|tab| WorkTab::new(WorkTabKind::Terminal, tab.title.clone()))
                .collect(),
            selected: strip.selected.unwrap_or(0),
            unavailable,
            body,
            notice: self.terminal_notice.clone(),
        }
    }

    /// The start page's "Open terminal": shows the Work panel and opens a terminal in `workspace`.
    pub(super) fn open_terminal(
        &mut self,
        workspace: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model.show_region(Region::WorkPanel);
        self.create_terminal(workspace, window, cx);
        cx.notify();
    }

    /// "Open in Work panel" (title bar): shows the Work panel with `terminal` of `workspace` selected, leaving the
    /// focus where it is.
    pub(super) fn open_session_terminal(
        &mut self,
        workspace: &str,
        terminal: &str,
        cx: &mut Context<Self>,
    ) {
        self.model.show_region(Region::WorkPanel);
        self.reveal_terminal(workspace, terminal, cx);
    }

    /// `terminal.create` in `workspace`. The new terminal is selected and focused once the service
    /// answers; a refusal says why under the strip.
    ///
    /// The grid comes from the pane's measured size, which only a paint records. Until the Work
    /// panel has painted once, the create waits for that frame: a ConPTY that grows after its first
    /// draw leaves a gap under a TUI, so the terminal must open at the grid its pane will show.
    fn create_terminal(&mut self, workspace: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.service.is_none() {
            return;
        }
        if self.work_panel_size.get().is_some() {
            self.send_create(workspace, window, cx);
        } else {
            self.pane_waits += 1;
            self.await_pane(workspace, PANE_WAIT_FRAMES, window, cx);
        }
    }

    /// Runs on the next frame, before it draws: sends the create once the pane has a size, or once
    /// `frames` more frames have passed without one (the panel is hidden), which opens it at the fallback grid.
    fn await_pane(
        &mut self,
        workspace: String,
        frames: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let shell = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| {
                if shell.work_panel_size.get().is_some() || frames == 0 {
                    shell.pane_waits -= 1;
                    shell.send_create(workspace, window, cx);
                } else {
                    shell.await_pane(workspace, frames - 1, window, cx);
                }
                cx.notify();
            });
        });
    }

    /// `terminal.create` in `workspace` at the grid `work_panel_grid` gives now.
    fn send_create(&mut self, workspace: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let (cols, rows) = self.work_panel_grid(cx);
        let shell_program = shell_setting(
            self.store
                .read(cx)
                .state()
                .config_value(TERMINAL_SHELL_KEY)
                .map(|config| &config.value),
        );
        let params = new_terminal_params(&workspace, cols, rows, shell_program);
        let window_handle = window.window_handle();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.create_terminal(params).await;
            let _ = cx.update_window(window_handle, |_, window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.show_created_terminal(&workspace, answer, window, cx);
                });
            });
        })
        .detach();
    }

    /// The grid a new Work panel terminal opens at (`new_terminal_grid`): the grid its pane fits now, else the grid a
    /// Work panel pane last applied, else the fixed size.
    pub(super) fn work_panel_grid(&self, cx: &App) -> (u16, u16) {
        let measured = self.work_panel_size.get().and_then(|(width, height)| {
            let cell = grid_paint::cell_metrics(cx)?;
            Some(pane_grid(width, height, cell))
        });
        new_terminal_grid(measured, self.work_panel_applied)
    }

    /// Remembers the grid a Work panel pane applied, the fallback for a new terminal's grid.
    fn note_work_panel_grid(&mut self, cols: u16, rows: u16) {
        self.work_panel_applied = Some((cols, rows));
    }

    fn show_created_terminal(
        &mut self,
        workspace: &str,
        answer: Result<Terminal, Refusal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(terminal) => self.select_terminal(workspace, &terminal.id, window, cx),
            Err(refusal) => {
                self.terminal_notice =
                    Some(("The terminal did not open.".to_owned(), refusal.message()));
            }
        }
        cx.notify();
    }

    /// Selects `terminal` in `workspace`'s strip, attaches its view when it has none, and focuses
    /// its pane.
    pub(super) fn select_terminal(
        &mut self,
        workspace: &str,
        terminal: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_terminal(workspace, terminal, cx);
        if let Some(view) = self.terminal_views.get(terminal) {
            let focus = view.surface.read(cx).focus_handle().clone();
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Selects `terminal` in `workspace`'s strip and attaches its view when it has none, without
    /// moving the focus. The tab shows once the store lists the terminal.
    pub(super) fn reveal_terminal(
        &mut self,
        workspace: &str,
        terminal: &str,
        cx: &mut Context<Self>,
    ) {
        self.model.select_terminal(workspace, terminal);
        self.terminal_notice = None;
        if !self.terminal_views.contains_key(terminal) {
            self.attach_view(workspace, terminal, cx);
        }
        cx.notify();
    }

    /// Creates the view of `terminal` and starts following it through the bridge.
    pub(super) fn attach_view(&mut self, workspace: &str, terminal: &str, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let shell = cx.entity().downgrade();
        // An agent run's exit is answered by its session view, so its pane offers no Restart.
        let restart = !is_agent_run_terminal(&self.store.read(cx).state().agents, terminal);
        let surface = new_surface(
            shell,
            service.clone(),
            workspace.to_owned(),
            terminal.to_owned(),
            restart,
            cx,
        );
        let (pane, id) = (surface.clone(), terminal.to_owned());
        let follow = cx.spawn(async move |this, cx| {
            follow_terminal(this, pane, service, id, cx).await;
        });
        self.terminal_views.insert(
            terminal.to_owned(),
            TerminalView {
                surface,
                _follow: follow,
                failure: None,
            },
        );
    }

    /// The Work panel's strip asks for an action on one of its tabs (WorkPanelTabs.md).
    pub(super) fn work_action(
        &mut self,
        action: WorkTabsAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.model.selected_workspace().map(str::to_owned) else {
            return;
        };
        let strip = terminal_strip(
            self.store.read(cx).state(),
            &workspace,
            self.model.terminal_selection(),
        );
        match action {
            WorkTabsAction::Select(index) => {
                if let Some(tab) = strip.tabs.get(index) {
                    let terminal = tab.id.clone();
                    self.select_terminal(&workspace, &terminal, window, cx);
                }
            }
            WorkTabsAction::Close(index) => {
                if let Some(tab) = strip.tabs.get(index) {
                    match close_step(tab.status) {
                        CloseStep::Forget => self.close_terminal(&tab.id, cx),
                        CloseStep::AskWhetherToStop => self.stop_prompt = Some(tab.id.clone()),
                    }
                }
            }
            WorkTabsAction::Add | WorkTabsAction::New(WorkTabKind::Terminal) => {
                self.create_terminal(workspace, window, cx);
            }
            WorkTabsAction::New(_) | WorkTabsAction::Move { .. } => {}
        }
        cx.notify();
    }

    /// `terminal.close`: the process ends and the terminal is forgotten, so its tab and view go.
    fn close_terminal(&mut self, terminal: &str, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let terminal = terminal.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.close_terminal(&terminal).await;
            let _ = shell.update(cx, |shell, cx| {
                match answer {
                    Ok(_) => {
                        shell.terminal_views.remove(&terminal);
                        shell.model.forget_terminal(&terminal);
                    }
                    Err(refusal) => {
                        shell.terminal_notice =
                            Some(("The terminal did not close.".to_owned(), refusal.message()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The stop question's answer. "Stop process" closes the terminal; "Keep running" drops its
    /// view and leaves the process in the service.
    fn stop_prompt_answered(&mut self, stop: bool, cx: &mut Context<Self>) {
        let Some(terminal) = self.stop_prompt.take() else {
            return;
        };
        if stop {
            self.close_terminal(&terminal, cx);
        } else {
            self.terminal_views.remove(&terminal);
        }
        cx.notify();
    }

    /// Attaches again after a refused attach, from the pane's Retry.
    fn retry_terminal(
        &mut self,
        workspace: &str,
        terminal: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal_views.remove(terminal);
        self.select_terminal(workspace, terminal, window, cx);
    }

    /// Attaches again after a refused attach, from a Retry that leaves the Work panel's selection alone.
    pub(super) fn retry_attach(&mut self, workspace: &str, terminal: &str, cx: &mut Context<Self>) {
        self.terminal_views.remove(terminal);
        self.attach_view(workspace, terminal, cx);
        cx.notify();
    }

    /// Drops the views of terminals the store no longer lists, and a stop question about one.
    pub(super) fn prune_terminal_views(&mut self, state: &StoreState) {
        // A session's terminal stays listed for its view, which the session answer can come before the store.
        let listed = |id: &str| {
            state.terminals.iter().any(|terminal| terminal.id == id)
                || state
                    .agents
                    .iter()
                    .any(|session| session.terminal_id.as_deref() == Some(id))
        };
        self.terminal_views.retain(|id, _| listed(id));
        if self.stop_prompt.as_deref().is_some_and(|id| !listed(id)) {
            self.stop_prompt = None;
        }
    }
}

/// The element ids of a refused attach's banner, so each pane's banner keeps its own test ids.
#[derive(Clone, Copy)]
pub(super) struct AttachFailedIds {
    pub(super) selector: &'static str,
    pub(super) banner: &'static str,
    pub(super) retry: &'static str,
}

pub(super) const WORK_ATTACH_FAILED: AttachFailedIds = AttachFailedIds {
    selector: "work-attach-failed",
    banner: "work-attach-failed-banner",
    retry: "work-retry-attach",
};

pub(super) const SESSION_ATTACH_FAILED: AttachFailedIds = AttachFailedIds {
    selector: "session-attach-failed",
    banner: "session-attach-failed-banner",
    retry: "session-retry-attach",
};

/// The banner of a terminal whose attach was refused, with Retry; `retry` runs when the user asks again.
pub(super) fn attach_failed_banner(
    ids: AttachFailedIds,
    detail: String,
    retry: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .debug_selector(move || ids.selector.to_owned())
        .child(
            Banner::new(
                ids.banner,
                BannerVariant::Danger,
                "This terminal could not be shown.",
            )
            .detail(detail)
            .action(
                Button::new(ids.retry, "Retry")
                    .variant(ButtonVariant::Secondary)
                    .on_click(move |window, cx| retry(window, cx)),
            ),
        )
        .into_any_element()
}

/// A pane for `terminal`: keys and resizes go to the service. With `restart`, the Restart of its exit row creates
/// a new terminal in the same workspace; an agent run has none, since its session view answers its exit.
fn new_surface(
    shell: WeakEntity<Shell>,
    service: ServiceHandle,
    workspace: String,
    terminal: String,
    restart: bool,
    cx: &mut App,
) -> Entity<TerminalSurface> {
    cx.new(|cx| {
        let (typing, typed_into) = (service.clone(), terminal.clone());
        let (sizing, sized) = (service, terminal);
        let noting = shell.clone();
        let surface = TerminalSurface::new(NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS, cx)
            .on_input(move |bytes, _, _| typing.send_terminal_input(&typed_into, bytes.to_vec()))
            .on_resize(move |cols, rows, _, cx| {
                let known = noting
                    .update(cx, |shell, cx| {
                        shell.note_terminal_grid(&sized, cols, rows, cx);
                        shell.note_work_panel_grid(cols, rows);
                        // A pane that fits to the grid the service already reports needs no resize request.
                        shell.terminal_has_grid(&sized, cols, rows, cx)
                    })
                    .unwrap_or(false);
                if known {
                    return;
                }
                let (service, terminal) = (sizing.clone(), sized.clone());
                cx.spawn(async move |_| {
                    let _ = service.resize_terminal(&terminal, cols, rows).await;
                })
                .detach();
            });
        if !restart {
            return surface;
        }
        surface.on_restart(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.create_terminal(workspace.clone(), window, cx);
            });
        })
    })
}

/// Follows one terminal until its stream finishes, its view goes, or its attach is refused. A lost
/// link waits for the service to connect again, then attaches without an offset, which gives a
/// snapshot (0034). A gap in the output attaches again at once the same way.
async fn follow_terminal(
    this: WeakEntity<Shell>,
    surface: Entity<TerminalSurface>,
    service: ServiceHandle,
    terminal: String,
    cx: &mut AsyncApp,
) {
    let mut link = service.watch_connection();
    loop {
        if !wait_until_connected(&mut link).await {
            return;
        }
        // The pane forwards the replies its emulator owes the program, so it declares that it
        // answers the queries; the service then leaves them to it (decision 0036).
        let params = AttachRole::AnswersQueries.attach_params(&terminal, None);
        let mut stream = match service.attach_terminal(params).await {
            Ok(stream) => stream,
            // The call met a link that was not up. Ask again only after the link changes, so a
            // stale "connected" never turns into a loop of refused calls.
            Err(Refusal::Unavailable { .. }) => {
                if link.changed().await.is_err() {
                    return;
                }
                continue;
            }
            Err(refusal) => {
                refuse_view(&this, &terminal, refusal.message(), cx);
                return;
            }
        };
        let flow = read_attach(&mut stream, &this, &surface, &service, &terminal, cx).await;
        if matches!(flow, Flow::Done) {
            return;
        }
    }
}

/// Reads one attach until it ends. Its updates feed the pane; the replies they owe go back to the
/// terminal as input.
async fn read_attach(
    stream: &mut TerminalStream,
    this: &WeakEntity<Shell>,
    surface: &Entity<TerminalSurface>,
    service: &ServiceHandle,
    terminal: &str,
    cx: &mut AsyncApp,
) -> Flow {
    while let Some(item) = stream.next().await {
        match item {
            TerminalStreamItem::Update(update) => {
                if let Step::Reattach = apply_update(surface, service, terminal, &update, cx) {
                    return Flow::Attach;
                }
            }
            TerminalStreamItem::Ended(
                StreamEnd::Lost | StreamEnd::Failed(Refusal::Unavailable { .. }),
            ) => return Flow::Attach,
            TerminalStreamItem::Ended(StreamEnd::Finished) => return Flow::Done,
            TerminalStreamItem::Ended(StreamEnd::Failed(refusal)) => {
                refuse_view(this, terminal, refusal.message(), cx);
                return Flow::Done;
            }
        }
    }
    Flow::Done
}

/// Feeds one attach update to the pane, then forwards the replies it owes the program to the terminal.
pub(super) fn apply_update(
    surface: &Entity<TerminalSurface>,
    service: &ServiceHandle,
    terminal: &str,
    update: &AttachUpdate,
    cx: &mut AsyncApp,
) -> Step {
    apply_update_with(surface, update, cx, |replies| {
        service.send_terminal_input(terminal, replies);
    })
}

/// Feeds one attach update to the pane, then hands the replies it owes the program to `input`: the service's terminal
/// for a local attach, a paired host's for a remote one.
pub(super) fn apply_update_with(
    surface: &Entity<TerminalSurface>,
    update: &AttachUpdate,
    cx: &mut AsyncApp,
    input: impl FnOnce(Vec<u8>),
) -> Step {
    surface.update(cx, |view, cx| {
        let step = feed_update(view, update, cx);
        forward_replies(view, input);
        step
    })
}

fn feed_update(
    view: &mut TerminalSurface,
    update: &AttachUpdate,
    cx: &mut gpui::Context<TerminalSurface>,
) -> Step {
    for event in feed_events(update) {
        match view.apply(event, cx) {
            Ok(FeedOutcome::Gap { .. }) | Err(_) => return Step::Reattach,
            Ok(FeedOutcome::Applied | FeedOutcome::Skipped) => {}
        }
    }
    Step::Continue
}

/// Sends the replies the emulator owes the program to the terminal as input (decision 0034). Every
/// attach passes its replies through here, so the declaration that this client answers the
/// program's queries goes in one place once the service asks for it.
fn forward_replies(view: &mut TerminalSurface, input: impl FnOnce(Vec<u8>)) {
    let replies = view.take_replies();
    if !replies.is_empty() {
        input(replies);
    }
}

/// Shows why a terminal's attach was refused, with Retry, until the user tries again.
fn refuse_view(this: &WeakEntity<Shell>, terminal: &str, detail: String, cx: &mut AsyncApp) {
    let _ = this.update(cx, |shell, cx| {
        if let Some(view) = shell.terminal_views.get_mut(terminal) {
            view.failure = Some(detail);
        }
        cx.notify();
    });
}

/// Waits until the link to the service is connected. False once the bridge has stopped for good.
pub(super) async fn wait_until_connected(link: &mut watch::Receiver<ConnectionState>) -> bool {
    loop {
        if link.borrow_and_update().is_connected() {
            return true;
        }
        if link.changed().await.is_err() {
            return false;
        }
    }
}

/// The Work panel's body: a notice, the strip (or its empty state), and the selected tab's body.
pub(super) fn work_panel(frame: WorkFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let WorkFrame {
        shell,
        tabs,
        selected,
        unavailable,
        body,
        notice,
    } = frame;
    let has_tabs = !tabs.is_empty();
    let strip_shell = shell.clone();
    let strip = unavailable.into_iter().fold(
        WorkPanelTabs::new("work-panel-tabs", tabs)
            .selected(selected)
            .on_action(move |action, window, cx| {
                let _ = strip_shell.update(cx, |shell, cx| shell.work_action(action, window, cx));
            }),
        |strip, (kind, reason)| strip.unavailable(kind, reason),
    );
    let mut column = div().flex_1().min_h_0().min_w_0().flex().flex_col();
    if let Some((title, detail)) = notice {
        column = column.child(
            div()
                .debug_selector(|| "work-panel-notice".to_owned())
                .child(
                    Banner::new("work-panel-notice-banner", BannerVariant::Danger, title)
                        .detail(detail),
                ),
        );
    }
    // While a new terminal waits for its pane, the strip's row is blank so the pane's probe lands where the
    // first terminal's pane will be.
    let measuring = matches!(body, WorkBody::Measuring { .. });
    column = if measuring {
        column.child(div().flex_none().h(px(SIZE_CONTROL_DEFAULT)))
    } else if has_tabs {
        column.child(div().flex_none().child(strip))
    } else {
        column.child(div().flex_1().min_h_0().child(strip))
    };
    if has_tabs || measuring {
        column = column.child(body_element(body, &shell, &theme));
    }
    column.into_any_element()
}

/// The selected tab's body. A terminal with a view shows its pane; one without shows the note
/// that it runs on, with "Show terminal" to attach it.
fn body_element(body: WorkBody, shell: &WeakEntity<Shell>, theme: &Theme) -> AnyElement {
    let pane = div().flex_1().min_h_0().min_w_0().flex().flex_col();
    match body {
        WorkBody::Empty => pane.into_any_element(),
        WorkBody::Measuring { size } => pane.child(measured_pane(size, div())).into_any_element(),
        WorkBody::Detached {
            workspace,
            terminal,
            text,
            banner,
        } => {
            let show = shell.clone();
            pane.when_some(banner, |pane, message| {
                pane.child(unreachable_banner(message))
            })
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(SPACE_2))
                    .child(
                        div()
                            .text_style(TYPE_BODY_SM)
                            .text_color(theme.palette.text.tertiary)
                            .child(text),
                    )
                    .child(
                        Button::new("work-show-terminal", "Show terminal")
                            .variant(ButtonVariant::Secondary)
                            .on_click(move |window, cx| {
                                let _ = show.update(cx, |shell, cx| {
                                    shell.select_terminal(&workspace, &terminal, window, cx);
                                });
                            }),
                    ),
            )
            .into_any_element()
        }
        WorkBody::Pane {
            workspace,
            terminal,
            surface,
            banner,
            failure,
            size,
        } => {
            let retry = shell.clone();
            let pane = pane.when_some(banner, |pane, message| {
                pane.child(unreachable_banner(message))
            });
            match failure {
                Some(detail) => pane
                    .child(attach_failed_banner(
                        WORK_ATTACH_FAILED,
                        detail,
                        move |window, cx| {
                            let _ = retry.update(cx, |shell, cx| {
                                shell.retry_terminal(&workspace, &terminal, window, cx);
                            });
                        },
                    ))
                    .into_any_element(),
                None => pane.child(measured_pane(size, surface)).into_any_element(),
            }
        }
    }
}

/// A terminal's pane: `content`, over a probe that records the pane's size as it paints (the probe takes no room).
fn measured_pane(
    size: Rc<Cell<Option<(f32, f32)>>>,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .child(
            gpui::canvas(
                |_, _, _| (),
                move |bounds, (), _, _| {
                    size.set(Some((
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    )));
                },
            )
            .absolute()
            .size_full(),
        )
        .child(content)
}

/// The banner of a terminal whose host cannot be reached: it may still run, and it never says it
/// ended.
fn unreachable_banner(message: &'static str) -> impl IntoElement {
    div()
        .debug_selector(|| "work-unreachable".to_owned())
        .child(Banner::new(
            "work-unreachable-banner",
            BannerVariant::Unreachable,
            message,
        ))
}

/// The question before closing a terminal that may still run (X-DLG-01, D-TM-04).
pub(super) fn stop_dialog(shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let theme = *cx.theme();
    let (stop, keep) = (shell.clone(), shell.clone());
    Dialog::new("stop-terminal", "Stop the process or keep it running?")
        .variant(DialogVariant::Destructive)
        .open(true)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(
                    "Process still running. Keep running hides this view; the process goes on \
                     and its tab stays in the strip.",
                ),
        )
        .action("Stop process")
        .cancel_label("Keep running")
        .on_action(move |_, cx| {
            let _ = stop.update(cx, |shell, cx| shell.stop_prompt_answered(true, cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = keep.update(cx, |shell, cx| shell.stop_prompt_answered(false, cx));
        })
}
