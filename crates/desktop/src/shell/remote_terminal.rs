//! The live terminal of a session a paired host runs (STAT-16), shown by its session view. Its output comes over the
//! host's own attach, and its keys and resizes go to the host. Dropping the view detaches; the host's terminal keeps
//! running. While the host is out of contact the last screen stays, and a line says so.

use gpui::prelude::*;
use gpui::{AsyncApp, Context, Entity, Task, WeakEntity};
use massorca_ui::components::terminal_surface::TerminalSurface;

use super::Shell;
use super::work_panel::{Step, apply_update_with};
use crate::service::{HostTerminal, HostTerminalItem, HostWriter};

/// The terminal of the open remote session: its surface, and the task that feeds it from the host's attach.
#[derive(Debug)]
pub(super) struct RemoteTerminal {
    host: String,
    terminal: String,
    pub(super) surface: Entity<TerminalSurface>,
    /// Whether the host is out of contact: its line shows, and the last screen stays.
    pub(super) out_of_contact: bool,
    /// Why the host refused the attach; Retry attaches again.
    pub(super) failure: Option<String>,
    _follow: Task<()>,
}

impl RemoteTerminal {
    /// Whether this is terminal `terminal` of `host`.
    pub(super) fn is(&self, host: &str, terminal: &str) -> bool {
        self.host == host && self.terminal == terminal
    }
}

impl Shell {
    /// Attaches `terminal` of the paired host `host` at the grid a session starts at, and follows it until its view goes.
    pub(super) fn attach_remote_terminal(
        &mut self,
        host: &str,
        terminal: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let (cols, rows) = self.start_grid(cx);
        let attach = service.attach_host_terminal(host, terminal, cols, rows);
        let writer = attach.writer();
        let shell = cx.entity().downgrade();
        let surface = cx.new(|cx| {
            let (typing, sizing) = (writer.clone(), writer.clone());
            let (noting, noted) = (shell.clone(), (host.to_owned(), terminal.to_owned()));
            TerminalSurface::new(cols, rows, cx)
                .on_input(move |bytes, _, _| typing.input(bytes.to_vec()))
                .on_resize(move |cols, rows, _, cx| {
                    sizing.resize(cols, rows);
                    let _ = noting.update(cx, |shell, _| {
                        shell.note_remote_grid(&noted.0, &noted.1, cols, rows);
                    });
                })
        });
        let (pane, feed) = (surface.clone(), (host.to_owned(), terminal.to_owned()));
        let follow = cx.spawn(async move |this, cx| {
            follow_remote_terminal(this, pane, attach, writer, feed, cx).await;
        });
        self.remote_terminal = Some(RemoteTerminal {
            host: host.to_owned(),
            terminal: terminal.to_owned(),
            surface,
            out_of_contact: false,
            failure: None,
            _follow: follow,
        });
    }

    /// Attaches a remote terminal again, from the Retry of its refusal: the next frame attaches it.
    pub(super) fn retry_remote_terminal(&mut self, cx: &mut Context<Self>) {
        self.remote_terminal = None;
        cx.notify();
    }
}

/// Feeds the surface from the host's attach until the attach ends. The replies the emulator owes the program go back to
/// the host as input; a gap asks the host for a snapshot again.
async fn follow_remote_terminal(
    this: WeakEntity<Shell>,
    surface: Entity<TerminalSurface>,
    mut attach: HostTerminal,
    writer: HostWriter,
    (host, terminal): (String, String),
    cx: &mut AsyncApp,
) {
    let mut out_of_contact = false;
    while let Some(item) = attach.next().await {
        match item {
            HostTerminalItem::Update(update) => {
                if let Step::Reattach =
                    apply_update_with(&surface, &update, cx, |replies| writer.input(replies))
                {
                    writer.reattach();
                }
                if out_of_contact {
                    out_of_contact = false;
                    set_out_of_contact(&this, &host, &terminal, false, cx);
                }
            }
            HostTerminalItem::Unreachable => {
                out_of_contact = true;
                set_out_of_contact(&this, &host, &terminal, true, cx);
            }
            HostTerminalItem::Finished => return,
            HostTerminalItem::Refused(refusal) => {
                let _ = this.update(cx, |shell, cx| {
                    if let Some(view) = shell
                        .remote_terminal
                        .as_mut()
                        .filter(|view| view.is(&host, &terminal))
                    {
                        view.failure = Some(refusal.message());
                    }
                    cx.notify();
                });
                return;
            }
        }
    }
}

/// Shows or clears the out-of-contact line of terminal `terminal` of `host`. A view that has gone changes nothing.
fn set_out_of_contact(
    this: &WeakEntity<Shell>,
    host: &str,
    terminal: &str,
    out: bool,
    cx: &mut AsyncApp,
) {
    let _ = this.update(cx, |shell, cx| {
        if let Some(view) = shell
            .remote_terminal
            .as_mut()
            .filter(|view| view.is(host, terminal))
        {
            view.out_of_contact = out;
            cx.notify();
        }
    });
}
