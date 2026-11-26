//! A session row's title, edited in place (A97, Rename). The row becomes a field that opens with the current title:
//! Enter sends `agent.rename` with the new title, and Escape closes the editor and keeps the title.

use std::fmt;

use gpui::{AppContext, Context, Entity, Focusable, Subscription, Window};
use massorca_ui::components::text_field::{TextField, TextFieldEvent};

use super::Shell;

/// The session row whose title is being edited.
pub(super) struct SessionRename {
    /// The session whose row is the editor.
    pub(super) session: String,
    pub(super) field: Entity<TextField>,
    /// Whether the field still needs the focus; the first draw after the editor opens gives it.
    pub(super) autofocus: bool,
    _submitted: Subscription,
}

impl fmt::Debug for SessionRename {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionRename")
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl Shell {
    /// Turns a session's row into a title editor that holds the session's current title.
    pub(super) fn start_session_rename(&mut self, session: &str, cx: &mut Context<Self>) {
        let title = self
            .store
            .read(cx)
            .state()
            .agents
            .iter()
            .find(|agent| agent.id == session)
            .and_then(|agent| agent.title.clone())
            .unwrap_or_default();
        let field = cx.new(|cx| {
            TextField::new(cx)
                .placeholder("Session title")
                .with_text(&title)
        });
        let submitted = cx.subscribe(&field, |shell, _, event: &TextFieldEvent, cx| {
            if matches!(event, TextFieldEvent::Submitted) {
                shell.finish_session_rename(true, cx);
            }
        });
        self.session_rename = Some(SessionRename {
            session: session.to_owned(),
            field,
            autofocus: true,
            _submitted: submitted,
        });
        cx.notify();
    }

    /// Gives a new title editor the focus, once, on the draw after it opened.
    pub(super) fn focus_session_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.session_rename.as_mut() else {
            return;
        };
        if !std::mem::take(&mut rename.autofocus) {
            return;
        }
        let focus = rename.field.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }

    /// Ends the title edit. `commit` sends the typed title when it is not empty and differs from the session's
    /// current one; otherwise the title stays as it was.
    pub(super) fn finish_session_rename(&mut self, commit: bool, cx: &mut Context<Self>) {
        let Some(rename) = self.session_rename.take() else {
            return;
        };
        if commit {
            let title = rename.field.read(cx).text().trim().to_owned();
            let current = self
                .store
                .read(cx)
                .state()
                .agents
                .iter()
                .find(|agent| agent.id == rename.session)
                .and_then(|agent| agent.title.as_deref().map(str::trim).map(str::to_owned));
            if let Some(service) = self.service.clone()
                && !title.is_empty()
                && current.as_deref() != Some(title.as_str())
            {
                let session = rename.session;
                cx.spawn(async move |_, _| {
                    // The store follows the service's own answer; a refusal leaves the title as it was.
                    let _ = service.agent_rename(&session, &title).await;
                })
                .detach();
            }
        }
        cx.notify();
    }
}
