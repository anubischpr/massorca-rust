//! The delegate form under a running session (X-CH-06): a prompt, the agent the child starts on, an optional
//! workspace name and title, read only, and the line that says where a writing child runs, stated before the person
//! types. Sending calls `agent.delegate` with this submit's id, so a retry sends the same id and the service answers
//! with the child it already made. A refusal keeps the form and its input.

use gpui::{
    AnyElement, App, Context, Entity, Focusable, Subscription, WeakEntity, Window, div, prelude::*,
    px,
};
use massorca_protocol::methods::agents::AgentDelegated;
use massorca_ui::components::dialog::{Dialog, DialogWidth};
use massorca_ui::components::kbd::Kbd;
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::switch::Switch;
use massorca_ui::components::text_editing::text_area::{TextArea, TextAreaEvent, TextAreaKey};
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::tokens::{
    RADIUS_MD, SIZE_COMPOSER_MAX_HEIGHT, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_CAPTION,
    TYPE_LABEL,
};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use crate::model::agent_delegate::{
    DelegateDraft, NO_KIND, PlacementNotice, WritingChild, delegate_params, new_request_id,
    placement_notice, starting_kind, writing_child_of,
};
use crate::model::agent_kinds::{KindChoice, session_kind_choices};
use crate::service::Refusal;

/// The delegate form on screen: its fields, the agents it offers, and what its last submit got.
#[derive(Debug)]
pub(super) struct DelegateForm {
    pub(super) parent: String,
    /// A brief usually runs several lines: Enter adds one and the primary modifier with Enter sends.
    pub(super) prompt: Entity<TextArea>,
    pub(super) name: Entity<TextField>,
    pub(super) title: Entity<TextField>,
    /// The agents the form offers: the ones turned on and detected here, in the order the service lists them.
    pub(super) kinds: Vec<KindChoice>,
    kind_select: Entity<SelectView>,
    /// The index into `kinds` of the agent the child starts on: the parent's, or the one the person picked.
    pub(super) kind: Option<usize>,
    /// Where a writing child of the parent runs, as the parent declares it; `None` while none is declared.
    pub(super) placement: Option<WritingChild>,
    pub(super) read_only: bool,
    /// The id of the submit the form last sent. It stays while the answer or its refusal shows, so a retry sends it.
    pub(super) request: Option<String>,
    pub(super) sending: bool,
    pub(super) error: Option<String>,
    /// The child the service started, once it did; the form then offers to open it.
    pub(super) started: Option<StartedChild>,
    _subscriptions: Vec<Subscription>,
}

/// The child a delegate call started: its id, its title and the note the service attached to the answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StartedChild {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) note: Option<String>,
}

impl DelegateForm {
    /// The line the form states before the person types; a read-only child needs none.
    fn notice(&self) -> Option<PlacementNotice> {
        self.placement
            .as_ref()
            .and_then(|writing| placement_notice(writing, self.read_only))
    }

    /// Whether the parent's placement keeps Delegate off, as an unavailable session does.
    fn blocked(&self) -> bool {
        self.notice().is_some_and(|notice| notice.blocks_send)
    }
}

/// The dialog on screen, if any: the form until its call is answered, then the child it started.
pub(in crate::shell) fn dialog(
    shell: &Shell,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let form = shell.delegate.as_ref()?;
    Some(match &form.started {
        Some(started) => started_view(started, weak, cx),
        None => form_view(form, weak, cx),
    })
}

/// Room for about three lines of brief before the prompt grows.
const PROMPT_MIN_HEIGHT: f32 = 72.0;

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
        let (parent_kind, placement, kinds) = {
            let state = self.store.read(cx).state();
            let Some(parent) = state.agents.iter().find(|parent| parent.id == session) else {
                return;
            };
            (
                parent.kind.clone(),
                writing_child_of(parent),
                session_kind_choices(&state.agent_kinds),
            )
        };
        let prompt = cx.new(|cx| {
            TextArea::new(cx)
                .id("delegate-prompt")
                .label("Prompt")
                .placeholder("What should the child do?")
                .heights(PROMPT_MIN_HEIGHT, SIZE_COMPOSER_MAX_HEIGHT)
        });
        let prompt_events = cx.subscribe_in(
            &prompt,
            window,
            |shell, area, event: &TextAreaEvent, _, cx| match event {
                TextAreaEvent::Changed => shell.delegate_edited(cx),
                TextAreaEvent::Key(TextAreaKey::Enter) => {
                    area.update(cx, |area, cx| area.insert_newline(cx));
                }
                TextAreaEvent::Key(TextAreaKey::ModifiedEnter) => shell.submit_delegate(cx),
                _ => {}
            },
        );
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
            placement,
            read_only: false,
            request: None,
            sending: false,
            error: None,
            started: None,
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
        if form.sending || form.started.is_some() || form.blocked() {
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

    /// Shows what the service answered to the submit that holds `request`; an answer for a form that has moved on
    /// is dropped.
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
        form.sending = false;
        match answer {
            Ok(delegated) => {
                form.started = Some(StartedChild {
                    id: delegated.session.id,
                    title: delegated.session.title.unwrap_or_default(),
                    note: delegated.note,
                });
            }
            Err(refusal) => form.error = Some(refusal.message()),
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

    /// Opens the child the form started in the session view, and closes the form.
    fn open_delegated_child(&mut self, child: &str, cx: &mut Context<Self>) {
        self.delegate = None;
        self.model.open_session(child);
        cx.notify();
    }
}

/// The form's dialog: its fields, the read-only choice, and its submit, which reads Retry after a refusal.
fn form_view(form: &DelegateForm, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let can_send = !form.prompt.read(cx).text().trim().is_empty()
        && form.kind.is_some()
        && !form.sending
        && !form.blocked();
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
        .children(form.notice().map(|notice| notice_view(&notice, cx)))
        .child(prompt_view(form, cx))
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

/// The dialog once the service started the child: its title, the service's note, and the way to open the child.
fn started_view(started: &StartedChild, weak: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let title = if started.title.is_empty() {
        "The child session".to_owned()
    } else {
        started.title.clone()
    };
    let mut body = div().flex().flex_col().gap(px(SPACE_1)).child(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.primary)
            .child(title),
    );
    if let Some(note) = &started.note {
        body = body.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(note.clone()),
        );
    }
    let (open, done) = (weak.clone(), weak.clone());
    let child = started.id.clone();
    Dialog::new("delegate", "Delegated")
        .open(true)
        .child(body)
        .action("Open child")
        .cancel_label("Done")
        .on_action(move |_, cx| {
            let _ = open.update(cx, |shell, cx| shell.open_delegated_child(&child, cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = done.update(cx, |shell, cx| shell.close_delegate(cx));
        })
        .into_any_element()
}

/// The line about where the child runs. It sits above the prompt, so the person reads it before typing.
fn notice_view(notice: &PlacementNotice, cx: &App) -> AnyElement {
    let p = cx.theme().palette;
    div()
        .debug_selector(|| "delegate-placement-notice".to_owned())
        .text_style(TYPE_BODY_SM)
        .text_color(p.text.secondary)
        .child(notice.text.clone())
        .into_any_element()
}

/// The prompt: its label, the multi-line editor in a field outline, and how to add a line and send.
fn prompt_view(form: &DelegateForm, cx: &App) -> AnyElement {
    let p = cx.theme().palette;
    let caption = |text: &'static str| {
        div()
            .text_style(TYPE_CAPTION)
            .text_color(p.text.tertiary)
            .child(text)
    };
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(p.text.secondary)
                .child("Prompt"),
        )
        .child(
            div()
                .px(px(SPACE_2))
                .py(px(SPACE_1))
                .rounded(px(RADIUS_MD))
                .border_1()
                .border_color(p.border.strong)
                .child(form.prompt.clone()),
        )
        .child(
            div()
                .debug_selector(|| "delegate-prompt-hint".to_owned())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(SPACE_1))
                .child(caption("Enter adds a line."))
                .child(Kbd::new("Mod+Enter"))
                .child(caption("delegates.")),
        )
        .into_any_element()
}
