//! The Work panel's pane on the Canvas (D-SH-17, TerminalSurface.md): the selected terminal's
//! TerminalSurface, attached through the bridge. Each view follows its terminal until the stream
//! ends or the view is dropped; a lost link waits for the service to connect again.

use gpui::prelude::*;
use gpui::{AnyElement, App, AsyncApp, Context, Entity, Task, WeakEntity, Window, div};
use massorca_client::AttachUpdate;
use massorca_protocol::methods::terminals::{Terminal, TerminalAttachParams};
use massorca_ui::components::terminal_surface::TerminalSurface;
use massorca_ui::components::terminal_surface::feed::FeedOutcome;
use massorca_ui::{ActiveTheme, TextStyled};
use tokio::sync::watch;

use super::Shell;
use crate::model::regions::Region;
use crate::model::terminals::new_terminal_params;
use crate::model::work_panel::{NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS, terminal_strip};
use crate::service::{
    ConnectionState, Refusal, ServiceHandle, StoreState, StreamEnd, TerminalStream,
    TerminalStreamItem, feed_events,
};

/// One terminal's view: its surface, and the task that follows it through the bridge. Dropping
/// the view detaches; the terminal keeps running.
#[derive(Debug)]
pub(super) struct TerminalView {
    pub(super) surface: Entity<TerminalSurface>,
    _follow: Task<()>,
    /// Why the attach was refused, until an attach works.
    pub(super) failure: Option<String>,
}

/// What the Work panel draws this frame: the pane of the selected terminal, if it has one.
pub(super) struct WorkFrame {
    body: WorkBody,
}

enum WorkBody {
    Empty,
    Pane {
        surface: Entity<TerminalSurface>,
        failure: Option<String>,
    },
}

/// What one attach update left to do.
enum Step {
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
    /// The Work panel as this frame draws it: the pane of the selected workspace's selected
    /// terminal, when that terminal has a view.
    pub(super) fn work_frame(&self, cx: &App) -> WorkFrame {
        let state = self.store.read(cx).state();
        let strip = self
            .model
            .selected_workspace()
            .map(|workspace| terminal_strip(state, workspace, self.model.terminal_selection()))
            .unwrap_or_default();
        let view = strip
            .selected
            .and_then(|index| strip.tabs.get(index))
            .and_then(|tab| self.terminal_views.get(&tab.id));
        let body = match view {
            Some(view) => WorkBody::Pane {
                surface: view.surface.clone(),
                failure: view.failure.clone(),
            },
            None => WorkBody::Empty,
        };
        WorkFrame { body }
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

    /// `terminal.create` in `workspace`. The new terminal is selected and focused once the service
    /// answers; a refused call changes nothing.
    fn create_terminal(&mut self, workspace: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let params = new_terminal_params(&workspace, NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS);
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

    fn show_created_terminal(
        &mut self,
        workspace: &str,
        answer: Result<Terminal, Refusal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Ok(terminal) = answer {
            self.select_terminal(workspace, &terminal.id, window, cx);
        }
        cx.notify();
    }

    /// Selects `terminal` in `workspace`'s strip, attaches its view when it has none, and focuses
    /// its pane.
    fn select_terminal(
        &mut self,
        workspace: &str,
        terminal: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model.select_terminal(workspace, terminal);
        if !self.terminal_views.contains_key(terminal) {
            self.attach_view(workspace, terminal, cx);
        }
        if let Some(view) = self.terminal_views.get(terminal) {
            let focus = view.surface.read(cx).focus_handle().clone();
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Creates the view of `terminal` and starts following it through the bridge.
    fn attach_view(&mut self, workspace: &str, terminal: &str, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let shell = cx.entity().downgrade();
        let surface = new_surface(
            shell,
            service.clone(),
            workspace.to_owned(),
            terminal.to_owned(),
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

    /// Drops the views of terminals the store no longer lists.
    pub(super) fn prune_terminal_views(&mut self, state: &StoreState) {
        self.terminal_views
            .retain(|id, _| state.terminals.iter().any(|terminal| terminal.id == *id));
    }
}

/// A pane for `terminal`: keys and resizes go to the service, and the Restart of its exit row
/// creates a new terminal in the same workspace.
fn new_surface(
    shell: WeakEntity<Shell>,
    service: ServiceHandle,
    workspace: String,
    terminal: String,
    cx: &mut App,
) -> Entity<TerminalSurface> {
    cx.new(|cx| {
        let (typing, typed_into) = (service.clone(), terminal.clone());
        let (sizing, sized) = (service, terminal);
        TerminalSurface::new(NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS, cx)
            .on_input(move |bytes, _, _| typing.send_terminal_input(&typed_into, bytes.to_vec()))
            .on_resize(move |cols, rows, _, cx| {
                let (service, terminal) = (sizing.clone(), sized.clone());
                cx.spawn(async move |_| {
                    let _ = service.resize_terminal(&terminal, cols, rows).await;
                })
                .detach();
            })
            .on_restart(move |window, cx| {
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
        let params = TerminalAttachParams {
            terminal: terminal.clone(),
            since: None,
            cols: None,
            rows: None,
        };
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

/// Feeds one attach update to the pane, then forwards the replies it owes the program.
fn apply_update(
    surface: &Entity<TerminalSurface>,
    service: &ServiceHandle,
    terminal: &str,
    update: &AttachUpdate,
    cx: &mut AsyncApp,
) -> Step {
    surface.update(cx, |view, cx| {
        let step = feed_update(view, update, cx);
        forward_replies(view, service, terminal);
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
fn forward_replies(view: &mut TerminalSurface, service: &ServiceHandle, terminal: &str) {
    let replies = view.take_replies();
    if !replies.is_empty() {
        service.send_terminal_input(terminal, replies);
    }
}

/// Shows why a terminal's attach was refused, until an attach works.
fn refuse_view(this: &WeakEntity<Shell>, terminal: &str, detail: String, cx: &mut AsyncApp) {
    let _ = this.update(cx, |shell, cx| {
        if let Some(view) = shell.terminal_views.get_mut(terminal) {
            view.failure = Some(detail);
        }
        cx.notify();
    });
}

/// Waits until the link to the service is connected. False once the bridge has stopped for good.
async fn wait_until_connected(link: &mut watch::Receiver<ConnectionState>) -> bool {
    loop {
        if link.borrow_and_update().is_connected() {
            return true;
        }
        if link.changed().await.is_err() {
            return false;
        }
    }
}

/// The Work panel's body: the selected pane, or the reason its attach was refused in its place.
pub(super) fn work_panel(frame: WorkFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let body = div().flex_1().min_h_0().min_w_0().flex().flex_col();
    match frame.body {
        WorkBody::Empty => body.into_any_element(),
        WorkBody::Pane {
            surface,
            failure: None,
        } => body
            .child(div().flex_1().min_h_0().child(surface))
            .into_any_element(),
        WorkBody::Pane {
            failure: Some(detail),
            ..
        } => body
            .child(
                div()
                    .text_body()
                    .text_color(theme.palette.status.danger_fg)
                    .child(detail),
            )
            .into_any_element(),
    }
}
