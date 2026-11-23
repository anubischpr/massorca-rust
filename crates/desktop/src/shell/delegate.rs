//! The delegate form under a running session (X-CH-06): a prompt, the agent the child starts on, an optional
//! workspace name and title, and read only. Sending calls `agent.delegate` with this submit's id, so a retry sends
//! the same id and the service answers with the child it already made. A refusal keeps the form and its input; a
//! started child closes it, and the session tree lists the child once the refetch after the answer has run.

use gpui::{
    AnyElement, App, Context, Entity, Focusable, Subscription, WeakEntity, Window, div, prelude::*,
    px,
};
use massorca_protocol::methods::agents::AgentDelegated;
use massorca_ui::components::dialog::{Dialog, DialogWidth};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::switch::Switch;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::tokens::SPACE_3;

use super::Shell;
use crate::model::agent_delegate::{
    DelegateDraft, NO_KIND, delegate_params, new_request_id, starting_kind,
};
use crate::model::agent_kinds::{KindChoice, session_kind_choices};
use crate::service::Refusal;

/// The delegate form on screen: its fields, the agents it offers, and what its last submit got.
#[derive(Debug)]
pub(super) struct DelegateForm {
    pub(super) parent: String,
    pub(super) prompt: Entity<TextField>,
    pub(super) name: Entity<TextField>,
    pub(super) title: Entity<TextField>,
    /// The agents the form offers: the ones turned on and detected here, in the order the service lists them.
    pub(super) kinds: Vec<KindChoice>,
    kind_select: Entity<SelectView>,
    /// The index into `kinds` of the agent the child starts on: the parent's, or the one the person picked.
    pub(super) kind: Option<usize>,
    pub(super) read_only: bool,
    /// The id of the submit the form last sent. It stays while the answer or its refusal shows, so a retry sends it.
    pub(super) request: Option<String>,
    pub(super) sending: bool,
    pub(super) error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

/// The dialog on screen, if any: the delegate form.
pub(in crate::shell) fn dialog(
    shell: &Shell,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let form = shell.delegate.as_ref()?;
    Some(form_view(form, weak, cx))
}

/// A text field of the form. Its edits clear the last refusal, and Enter sends the form.
fn form_field(
    label: &'static str,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (Entity<TextField>, Subscription) {
    let field = cx.new(|cx| TextField::new(cx).label(label).placeholder(placeholder));
    let subscription = cx.subscribe_in(
        &field,
        window,
        |shell, _, event: &TextFieldEvent, _, cx| match event {
            TextFieldEvent::Changed => shell.delegate_edited(cx),
            TextFieldEvent::Submitted => shell.submit_delegate(cx),
        },
    );
    (field, subscription)
}

impl Shell {
    /// Opens the delegate form under a session the store holds, offering the agents this host runs. Nothing opens
    /// without a service to send to.
    pub(in crate::shell) fn open_delegate(
        &mut self,
        session: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.service.is_none() {
            return;
        }
        let (parent_kind, kinds) = {
            let state = self.store.read(cx).state();
            let Some(parent) = state.agents.iter().find(|parent| parent.id == session) else {
                return;
            };
            (
                parent.kind.clone(),
                session_kind_choices(&state.agent_kinds),
            )
        };
        let (prompt, prompt_events) = form_field("Prompt", "What should the child do?", window, cx);
        let (name, name_events) =
            form_field("Name", "Optional: the child's workspace name", window, cx);
        let (title, title_events) = form_field(
            "Title",
            "Optional: the first line of the prompt",
            window,
            cx,
        );
        let starting = starting_kind(&kinds, &parent_kind);
        let kind_select = cx.new(|cx| {
            SelectView::new(kinds.iter().map(|kind| kind.name.clone()), cx)
                .label("Agent")
                .selected(starting)
                .empty_text(NO_KIND)
                .layer(OverlayLayer::Dialog)
        });
        let kind_events = cx.subscribe_in(
            &kind_select,
            window,
            |shell, _, event: &SelectEvent, _, cx| match event {
                SelectEvent::Changed(index) => shell.set_delegate_kind(*index, cx),
            },
        );
        self.delegate = Some(DelegateForm {
            parent: session.to_owned(),
            prompt,
            name,
            title,
            kinds,
            kind_select,
            kind: starting,
            read_only: false,
            request: None,
            sending: false,
            error: None,
            _subscriptions: vec![prompt_events, name_events, title_events, kind_events],
        });
        cx.notify();
    }

    /// Closes the form, whatever it shows.
    pub(super) fn close_delegate(&mut self, cx: &mut Context<Self>) {
        self.delegate = None;
        cx.notify();
    }

    /// Sends the form. A submit that follows no refusal gets a new id; a retry after a refusal sends the id it had.
    pub(super) fn submit_delegate(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(form) = self.delegate.as_mut() else {
            return;
        };
        if form.sending {
            return;
        }
        let draft = DelegateDraft {
            parent: form.parent.clone(),
            prompt: form.prompt.read(cx).text().to_owned(),
            kind: form
                .kind
                .and_then(|index| form.kinds.get(index))
                .map(|kind| kind.id.clone()),
            name: form.name.read(cx).text().to_owned(),
            title: form.title.read(cx).text().to_owned(),
            read_only: form.read_only,
        };
        let request = form.request.clone().unwrap_or_else(new_request_id);
        let params = match delegate_params(&draft, &request) {
            Ok(params) => params,
            Err(message) => {
                form.error = Some(message.to_owned());
                cx.notify();
                return;
            }
        };
        form.request = Some(request.clone());
        form.sending = true;
        form.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let answer = service.agent_delegate(params).await;
            let _ = this.update(cx, |shell, cx| shell.finish_delegate(&request, answer, cx));
        })
        .detach();
    }

    /// Shows what the service answered to the submit that holds `request`: a refusal keeps the form and its reason,
    /// and a started child closes it. An answer for a form that has moved on is dropped.
    fn finish_delegate(
        &mut self,
        request: &str,
        answer: Result<AgentDelegated, Refusal>,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.delegate.as_mut() else {
            return;
        };
        if form.request.as_deref() != Some(request) {
            return;
        }
        match answer {
            Ok(_) => self.delegate = None,
            Err(refusal) => {
                form.sending = false;
                form.error = Some(refusal.message());
            }
        }
        cx.notify();
    }

    /// An edit clears the last refusal, so the next submit is a new call with a new id.
    fn delegate_edited(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.delegate.as_mut()
            && !form.sending
        {
            form.error = None;
            form.request = None;
            cx.notify();
        }
    }

    /// Picks the agent the child starts on.
    pub(super) fn set_delegate_kind(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(form) = self.delegate.as_mut() {
            form.kind = Some(index);
        }
        self.delegate_edited(cx);
    }

    /// Turns read only on or off. A read-only child runs in this worktree, so it takes no name of its own.
    pub(super) fn set_delegate_read_only(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some(form) = self.delegate.as_mut() else {
            return;
        };
        form.read_only = on;
        form.name.update(cx, |field, cx| field.set_disabled(on, cx));
        self.delegate_edited(cx);
    }
}

/// The form's dialog: its fields, the read-only choice, and its submit, which reads Retry after a refusal.
fn form_view(form: &DelegateForm, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let can_send =
        !form.prompt.read(cx).text().trim().is_empty() && form.kind.is_some() && !form.sending;
    let read_only_weak = weak.clone();
    let read_only = Switch::new("delegate-read-only", form.read_only)
        .label("Read only")
        .helper("The child only reads and runs in this session's worktree.")
        .on_change(move |on, _, cx| {
            let _ = read_only_weak.update(cx, |shell, cx| shell.set_delegate_read_only(on, cx));
        });
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(form.prompt.clone())
        .child(form.kind_select.clone())
        .child(form.name.clone())
        .child(form.title.clone())
        .child(read_only);
    let (on_action, on_dismiss) = (weak.clone(), weak.clone());
    let mut dialog = Dialog::new("delegate", "Delegate a subtask")
        .open(true)
        .width(DialogWidth::Large)
        .busy(form.sending)
        .child(body)
        .field(form.prompt.read(cx).focus_handle(cx))
        .action(if form.error.is_some() {
            "Retry"
        } else {
            "Delegate"
        })
        .action_disabled(!can_send)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_delegate(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_delegate(cx));
        });
    if let Some(error) = &form.error {
        dialog = dialog.error(error.clone());
    }
    dialog.into_any_element()
}
