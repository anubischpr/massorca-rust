//! The composer: the box where the user writes to an agent (Composer.md, agreements A29,
//! A32 and A33). It is the approved session body: attachment chips, the text area, and a
//! bottom row with attach, access, the model picker, the microphone and the primary button,
//! which sends, queues or stops. The rules are in [`policy`]; the owner receives what the user
//! asked for as [`ComposerEvent`]s.

pub mod policy;

mod chrome;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, KeyBinding,
    SharedString, Subscription, Task, Window, actions,
};

use crate::components::menu::MenuView;
use crate::components::model_picker::{Choice, ModelPicker, ModelPickerEvent, PickerState};
use crate::components::popover::TriggerBounds;
use crate::components::text_editing::text_area::{TextArea, TextAreaEvent, TextAreaKey};
use crate::icon::IconName;
use crate::tokens::{SIZE_COMPOSER_MAX_HEIGHT, SIZE_COMPOSER_MIN_HEIGHT};

use self::policy::{
    EnterAction, EscapeAction, SentHistory, Situation, enter_action, escape_action,
};

actions!(composer, [OpenModelPicker]);

const KEY_CONTEXT: &str = "Composer";
const PLACEHOLDER: &str = "Ask anything, @ to mention files, / for commands";
const HINT_DURATION: Duration = Duration::from_secs(3);

/// Builds the control shown first in the bottom row, on each render.
pub type LeadingControl = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// Binds the composer's keys and those of the parts it holds; apps call this once at startup.
pub fn bind_keys(cx: &mut App) {
    crate::components::text_editing::text_area::bind_keys(cx);
    crate::components::model_picker::bind_keys(cx);
    crate::components::text_field::bind_keys(cx);
    let open = if cfg!(target_os = "macos") {
        "cmd-shift-m"
    } else {
        "ctrl-shift-m"
    };
    cx.bind_keys([KeyBinding::new(open, OpenModelPicker, Some(KEY_CONTEXT))]);
}

/// An access mode the agent offers, with its icon; the chip shows the selected one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessMode {
    pub name: SharedString,
    pub icon: IconName,
}

/// A file or image attached to the next message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub id: u64,
    pub name: SharedString,
}

/// What the composer reports to its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerEvent {
    /// The text is sent now; the composer has cleared itself.
    Send(String),
    /// The text waits behind the running turn; the composer has cleared itself.
    Queue(String),
    /// The user stopped the running turn.
    Stop,
    /// The draft text changed (save it per session).
    DraftChanged,
    AccessChanged(usize),
    ModelChanged(Choice),
    AttachFiles,
    PasteFromClipboard,
    Dictate,
    RemoveAttachment(u64),
    /// The user asked to send the failed message again.
    RetrySend,
    /// The user pressed "Sign in" on a signed-out composer.
    SignIn,
    /// Files dropped on the composer.
    DroppedFiles(Vec<PathBuf>),
}

/// The session composer. The owner keeps the session state and tells the composer about it
/// through the setters; the composer keeps what it shows of its own: the draft, the sent
/// history and the escape hint.
pub struct Composer {
    text: Entity<TextArea>,
    picker: Entity<ModelPicker>,
    attachments: Vec<Attachment>,
    access: Vec<AccessMode>,
    access_index: usize,
    agent_name: SharedString,
    voice: bool,
    primary_focus: FocusHandle,
    access_focus: FocusHandle,
    agent_busy: bool,
    host_confirmed: bool,
    disabled_reason: Option<SharedString>,
    /// The disabled composer offers "Sign in" beside its reason.
    sign_in: bool,
    error: Option<SharedString>,
    enter_inserts_newline: bool,
    /// A control shown first in the bottom row (the start page's Chat / Terminal switch).
    leading: Option<LeadingControl>,
    /// Why sending is not possible yet: Send and Enter stay off and the draft stays put.
    send_unavailable: Option<SharedString>,
    /// Why voice input is not possible yet: the microphone stays disabled with this reason.
    voice_unavailable: Option<SharedString>,
    hint_shown: bool,
    /// Keeps the escape hint's timer alive; clearing the hint drops it and cancels the timer.
    _hint_timer: Option<Task<()>>,
    history: SentHistory,
    access_trigger: TriggerBounds,
    access_menu: Option<(Entity<MenuView>, Subscription)>,
    attach_trigger: TriggerBounds,
    attach_menu: Option<(Entity<MenuView>, Subscription)>,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for Composer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composer")
            .field("agent_busy", &self.agent_busy)
            .field("host_confirmed", &self.host_confirmed)
            .field("disabled_reason", &self.disabled_reason)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Composer {
    pub fn new(picker: PickerState, access: Vec<AccessMode>, cx: &mut Context<Self>) -> Self {
        let text = cx.new(|cx| {
            TextArea::new(cx)
                .id("composer-text")
                .placeholder(PLACEHOLDER)
                .label("Message to Claude")
                .heights(SIZE_COMPOSER_MIN_HEIGHT, SIZE_COMPOSER_MAX_HEIGHT)
        });
        let picker = cx.new(|cx| ModelPicker::new(picker, cx));
        let text_events = cx.subscribe(&text, |this, _, event: &TextAreaEvent, cx| {
            this.on_text_event(*event, cx);
        });
        let picker_events = cx.subscribe(&picker, |_, _, event: &ModelPickerEvent, cx| {
            let ModelPickerEvent::Changed(choice) = event;
            cx.emit(ComposerEvent::ModelChanged(choice.clone()));
        });
        Self {
            text,
            picker,
            attachments: Vec::new(),
            access,
            access_index: 0,
            agent_name: "Claude".into(),
            voice: true,
            primary_focus: cx.focus_handle().tab_stop(true),
            access_focus: cx.focus_handle().tab_stop(true),
            agent_busy: false,
            host_confirmed: true,
            disabled_reason: None,
            sign_in: false,
            error: None,
            enter_inserts_newline: false,
            leading: None,
            send_unavailable: None,
            voice_unavailable: None,
            hint_shown: false,
            _hint_timer: None,
            history: SentHistory::default(),
            access_trigger: TriggerBounds::default(),
            access_menu: None,
            attach_trigger: TriggerBounds::default(),
            attach_menu: None,
            _subscriptions: vec![text_events, picker_events],
        }
    }

    pub fn picker(&self) -> &Entity<ModelPicker> {
        &self.picker
    }

    /// The draft text as typed.
    pub fn draft(&self, cx: &App) -> String {
        self.text.read(cx).text().to_owned()
    }

    /// Replaces the draft, for example when the user returns to a session.
    pub fn set_draft(&mut self, text: &str, cx: &mut Context<Self>) {
        self.text
            .update(cx, |area, cx| area.set_text_in_place(text, cx));
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.text
            .clone()
            .update(cx, |area, cx| area.focus(window, cx));
    }

    pub fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.agent_busy = busy;
        if !busy {
            self.clear_hint();
        }
        cx.notify();
    }

    /// `false` while the host is unreachable: the message stays in the composer.
    pub fn set_host_confirmed(&mut self, confirmed: bool, cx: &mut Context<Self>) {
        self.host_confirmed = confirmed;
        cx.notify();
    }

    /// Why the composer cannot take input (archived, or sign-in needed); `None` when it can.
    pub fn set_disabled_reason(&mut self, reason: Option<SharedString>, cx: &mut Context<Self>) {
        let disabled = reason.is_some();
        self.disabled_reason = reason;
        self.sign_in = false;
        self.text
            .update(cx, |area, cx| area.set_disabled(disabled, cx));
        cx.notify();
    }
    /// The agent account needs sign-in: the composer is disabled with "Sign in to <agent> to
    /// continue" and a "Sign in" action (Composer.md States, disabled).
    pub fn set_signed_out(&mut self, signed_out: bool, cx: &mut Context<Self>) {
        let reason =
            signed_out.then(|| format!("Sign in to {} to continue", self.agent_name).into());
        self.set_disabled_reason(reason, cx);
        self.sign_in = signed_out;
        cx.notify();
    }

    pub fn set_enter_inserts_newline(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.enter_inserts_newline = enabled;
        cx.notify();
    }

    fn situation(&self, cx: &App) -> Situation {
        let area = self.text.read(cx);
        Situation {
            agent_busy: self.agent_busy,
            host_confirmed: self.host_confirmed,
            disabled: self.disabled_reason.is_some() || self.send_unavailable.is_some(),
            text_empty: area.text().is_empty(),
            composing: area.is_composing(),
        }
    }

    fn on_text_event(&mut self, event: TextAreaEvent, cx: &mut Context<Self>) {
        match event {
            TextAreaEvent::Changed => {
                self.clear_hint();
                cx.emit(ComposerEvent::DraftChanged);
                cx.notify();
            }
            TextAreaEvent::Key(TextAreaKey::Enter) => self.enter(false, cx),
            TextAreaEvent::Key(TextAreaKey::ModifiedEnter) => self.enter(true, cx),
            TextAreaEvent::Key(TextAreaKey::Escape) => self.escape(cx),
            TextAreaEvent::HistoryPrevious => {
                if let Some(text) = self.history.older() {
                    self.set_draft(&text, cx);
                }
            }
            TextAreaEvent::HistoryNext => {
                if let Some(text) = self.history.newer() {
                    self.set_draft(&text, cx);
                }
            }
        }
    }

    fn enter(&mut self, modified: bool, cx: &mut Context<Self>) {
        let situation = self.situation(cx);
        match enter_action(situation, modified, self.enter_inserts_newline) {
            EnterAction::Send => self.submit(false, cx),
            EnterAction::Queue => self.submit(true, cx),
            EnterAction::Newline => self.text.update(cx, |area, cx| area.insert_newline(cx)),
            EnterAction::Nothing => {}
        }
    }

    /// Sends or queues the draft, clears it and remembers it for Up.
    fn submit(&mut self, queue: bool, cx: &mut Context<Self>) {
        let text = self.draft(cx);
        if text.trim().is_empty() {
            return;
        }
        self.history.push(&text);
        self.error = None;
        self.clear_hint();
        self.text.update(cx, |area, cx| area.clear(cx));
        let event = if queue {
            ComposerEvent::Queue(text)
        } else {
            ComposerEvent::Send(text)
        };
        cx.emit(event);
        cx.notify();
    }

    fn escape(&mut self, cx: &mut Context<Self>) {
        let situation = self.situation(cx);
        match escape_action(self.agent_busy, situation.text_empty, self.hint_shown) {
            EscapeAction::Nothing => {}
            EscapeAction::ShowHint => {
                self.hint_shown = true;
                self._hint_timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(HINT_DURATION).await;
                    this.update(cx, |this, cx| {
                        this.clear_hint();
                        cx.notify();
                    })
                    .ok();
                }));
                cx.notify();
            }
            EscapeAction::Stop => self.stop(cx),
        }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.clear_hint();
        cx.emit(ComposerEvent::Stop);
    }

    fn clear_hint(&mut self) {
        self.hint_shown = false;
        self._hint_timer = None;
    }
}

#[cfg(test)]
mod tests {
    use super::policy::{PrimaryAction, primary_action};
    use super::*;
    use crate::components::agent_icon::AgentKind;
    use crate::components::model_picker::{AgentAvailability, AgentOption, ModelList, ModelOption};
    use gpui::{TestAppContext, VisualTestContext};
    use std::{cell::RefCell, rc::Rc};

    fn picker_state() -> PickerState {
        let levels: Vec<SharedString> = vec!["Low".into(), "Medium".into()];
        PickerState::new(
            vec![AgentOption {
                kind: AgentKind::Claude,
                availability: AgentAvailability::Ready,
                list: ModelList::Loaded,
                models: vec![ModelOption {
                    id: "opus".into(),
                    name: "Opus 5.5".into(),
                    efforts: levels,
                    default_effort: 1,
                    fast: false,
                }],
            }],
            Choice {
                model: "opus".into(),
                effort: Some(1),
                fast: false,
            },
        )
    }

    fn access() -> Vec<AccessMode> {
        vec![
            AccessMode {
                name: "Ask for approval".into(),
                icon: IconName::Lock,
            },
            AccessMode {
                name: "Workspace write".into(),
                icon: IconName::Shield,
            },
        ]
    }

    fn composer_in(cx: &mut TestAppContext) -> (Entity<Composer>, &mut VisualTestContext) {
        cx.update(bind_keys);
        cx.add_window_view(|_, cx| Composer::new(picker_state(), access(), cx))
    }

    /// Subscribes to a composer and collects its events, except the per-keystroke draft changes.
    fn record(
        composer: &Entity<Composer>,
        cx: &mut VisualTestContext,
    ) -> Rc<RefCell<Vec<ComposerEvent>>> {
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(composer, move |_, event: &ComposerEvent, _| {
                if !matches!(event, ComposerEvent::DraftChanged) {
                    sink.borrow_mut().push(event.clone());
                }
            })
            .detach();
        });
        events
    }

    fn focus_and_draw(composer: &Entity<Composer>, cx: &mut VisualTestContext) {
        composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
        cx.run_until_parked();
    }

    fn draft_of(composer: &Entity<Composer>, cx: &VisualTestContext) -> String {
        composer.read_with(cx, |composer, cx| composer.draft(cx))
    }

    #[gpui::test]
    fn a_signed_out_composer_offers_sign_in(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        let events = record(&composer, cx);
        composer.update(cx, |composer, cx| composer.set_signed_out(true, cx));
        cx.run_until_parked();
        let reason = composer.read_with(cx, |composer, _| composer.disabled_reason.clone());
        assert_eq!(reason.as_deref(), Some("Sign in to Claude to continue"));
        let button = cx
            .debug_bounds("composer-sign-in")
            .expect("Sign in is drawn")
            .center();
        cx.simulate_click(button, gpui::Modifiers::none());
        assert_eq!(*events.borrow(), vec![ComposerEvent::SignIn]);
    }

    #[gpui::test]
    fn the_send_failed_line_sits_above_the_text(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| {
            composer.set_error(Some("offline".into()), cx)
        });
        cx.run_until_parked();
        let error = cx
            .debug_bounds("composer-error")
            .expect("the line is drawn");
        // Above the text, the line starts within the text area's own minimum height.
        assert!(error.top() < gpui::px(SIZE_COMPOSER_MIN_HEIGHT));
    }

    #[gpui::test]
    fn typing_then_enter_sends_the_text_and_clears_the_draft(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("hello");
        cx.simulate_keystrokes("enter");
        assert_eq!(*events.borrow(), vec![ComposerEvent::Send("hello".into())]);
        assert_eq!(draft_of(&composer, cx), "");
    }

    #[gpui::test]
    fn enter_while_the_agent_runs_queues_instead(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| composer.set_busy(true, cx));
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("later");
        cx.simulate_keystrokes("enter");
        assert_eq!(*events.borrow(), vec![ComposerEvent::Queue("later".into())]);
    }

    #[gpui::test]
    fn enter_keeps_the_draft_when_sending_is_unavailable(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (composer, cx) = cx.add_window_view(|_, cx| {
            Composer::new(picker_state(), access(), cx)
                .send_unavailable("Agent sessions come later.")
        });
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("hi");
        cx.simulate_keystrokes("enter");
        assert_eq!(draft_of(&composer, cx), "hi");
        assert!(events.borrow().is_empty());
    }

    #[gpui::test]
    fn shift_enter_adds_a_line_and_mod_enter_sends(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("a");
        cx.simulate_keystrokes("shift-enter");
        cx.simulate_input("b");
        assert_eq!(draft_of(&composer, cx), "a\nb");
        assert!(
            events
                .borrow()
                .iter()
                .all(|e| !matches!(e, ComposerEvent::Send(_)))
        );
        cx.simulate_keystrokes("secondary-enter");
        assert_eq!(*events.borrow(), vec![ComposerEvent::Send("a\nb".into())]);
    }

    #[gpui::test]
    fn an_unreachable_host_keeps_the_message_in_the_composer(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| composer.set_host_confirmed(false, cx));
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("keep me");
        cx.simulate_keystrokes("enter");
        assert!(events.borrow().is_empty());
        assert_eq!(draft_of(&composer, cx), "keep me");
    }

    #[gpui::test]
    fn escape_hints_first_and_stops_the_run_on_the_second_press(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| composer.set_busy(true, cx));
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_keystrokes("escape");
        assert!(composer.read_with(cx, |composer, _| composer.hint_shown));
        assert!(events.borrow().is_empty());
        cx.simulate_keystrokes("escape");
        assert_eq!(*events.borrow(), vec![ComposerEvent::Stop]);
    }

    #[gpui::test]
    fn up_in_an_empty_composer_recalls_the_last_sent_message(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("first");
        cx.simulate_keystrokes("enter");
        cx.simulate_keystrokes("up");
        assert_eq!(draft_of(&composer, cx), "first");
    }

    #[gpui::test]
    fn the_newline_setting_makes_enter_a_line_break(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| {
            composer.set_enter_inserts_newline(true, cx)
        });
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("x");
        cx.simulate_keystrokes("enter");
        assert!(events.borrow().is_empty());
        assert_eq!(draft_of(&composer, cx), "x\n");
    }

    #[gpui::test]
    fn the_primary_action_reads_send_queue_stop_and_disabled(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        let action = |cx: &mut VisualTestContext| {
            composer.read_with(cx, |composer, cx| primary_action(composer.situation(cx)))
        };
        assert_eq!(action(cx), PrimaryAction::Disabled);
        composer.update(cx, |composer, cx| composer.set_busy(true, cx));
        assert_eq!(action(cx), PrimaryAction::Stop);
        focus_and_draw(&composer, cx);
        cx.simulate_input("q");
        assert_eq!(action(cx), PrimaryAction::Queue);
    }
}
