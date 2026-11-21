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
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, KeyBinding, SharedString,
    Subscription, Task, Window, actions,
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
    error: Option<SharedString>,
    enter_inserts_newline: bool,
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
            error: None,
            enter_inserts_newline: false,
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
        self.text
            .update(cx, |area, cx| area.set_disabled(disabled, cx));
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
            disabled: self.disabled_reason.is_some(),
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
