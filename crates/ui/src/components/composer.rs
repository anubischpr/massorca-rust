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
use crate::components::model_picker::{
    AgentOption, Choice, ModelPicker, ModelPickerAction, ModelPickerEvent, PickerState, RowRef,
};
use crate::components::popover::TriggerBounds;
use crate::components::text_editing::text_area::{TextArea, TextAreaEvent, TextAreaKey};
use crate::icon::IconName;
use crate::tokens::{SIZE_COMPOSER_MAX_HEIGHT, SIZE_COMPOSER_MIN_HEIGHT};

use self::policy::{
    EnterAction, EscapeAction, PrimaryAction, SentHistory, Situation, enter_action, escape_action,
    primary_action,
};

actions!(composer, [OpenModelPicker]);

const KEY_CONTEXT: &str = "Composer";
const PLACEHOLDER: &str = "Ask anything, @ to mention files, / for commands";
const HINT_DURATION: Duration = Duration::from_secs(3);

/// Builds the control shown after the access chip in the bottom row, on each render.
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
    /// The mode lifts the agent's permission prompts: the chip carries a warning glyph while it is selected.
    pub bypass: bool,
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
    /// The user sent the text. The draft stays until the owner clears it, once it is accepted.
    Send(String),
    /// The user queued the text behind the running turn. The draft stays until the owner clears it.
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
    /// The user pressed "Resume" on the composer of an ended session.
    Resume,
    /// The user pressed "Open Marketplace" in a picker with no agent installed.
    OpenMarketplace,
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
    /// A caption after the access chip, such as a mode waiting for the next resume.
    access_note: Option<SharedString>,
    /// Each access mode's secondary text by index, `None` for none; see `set_access_details`.
    access_details: Vec<Option<SharedString>>,
    /// Each access mode's tag by index, `None` for none; see `set_access_tags`.
    access_tags: Vec<Option<SharedString>>,
    agent_name: SharedString,
    voice: bool,
    /// Whether the attach button, its menu and dropped files are offered.
    attach_offered: bool,
    /// Whether the model picker chip is offered.
    model_picker_offered: bool,
    /// Whether a message sent while the agent runs queues behind its turn.
    queueing_offered: bool,
    primary_focus: FocusHandle,
    access_focus: FocusHandle,
    agent_busy: bool,
    host_confirmed: bool,
    disabled_reason: Option<SharedString>,
    /// The disabled composer offers "Sign in" beside its reason.
    sign_in: bool,
    /// The disabled composer offers "Resume" beside its reason.
    resume: bool,
    error: Option<SharedString>,
    enter_inserts_newline: bool,
    /// Send works with a blank draft (the start page creates a workspace with no prompt).
    sends_empty: bool,
    /// A placement card sits on the top edge: the composer spans the card's column, keeping its rounded corners.
    joined_top: bool,
    /// A control shown after the access chip (the start page's Chat / Terminal switch).
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
    /// The picker's events, kept apart because the owner can rebuild the picker.
    _picker_events: Vec<Subscription>,
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
        let picker_events = Self::subscribe_picker(&picker, cx);
        Self {
            text,
            picker,
            attachments: Vec::new(),
            access,
            access_index: 0,
            access_note: None,
            access_details: Vec::new(),
            access_tags: Vec::new(),
            agent_name: "Claude".into(),
            voice: true,
            attach_offered: true,
            model_picker_offered: true,
            queueing_offered: true,
            primary_focus: cx.focus_handle().tab_stop(true),
            access_focus: cx.focus_handle().tab_stop(true),
            agent_busy: false,
            host_confirmed: true,
            disabled_reason: None,
            sign_in: false,
            resume: false,
            error: None,
            enter_inserts_newline: false,
            sends_empty: false,
            joined_top: false,
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
            _subscriptions: vec![text_events],
            _picker_events: picker_events,
        }
    }

    /// The picker's events, as the composer's own: the model choice and the Marketplace link.
    fn subscribe_picker(picker: &Entity<ModelPicker>, cx: &mut Context<Self>) -> Vec<Subscription> {
        vec![
            cx.subscribe(picker, |_, _, event: &ModelPickerEvent, cx| match event {
                ModelPickerEvent::Changed(choice) => {
                    cx.emit(ComposerEvent::ModelChanged(choice.clone()));
                }
            }),
            cx.subscribe(
                picker,
                |_, _, action: &ModelPickerAction, cx| match action {
                    ModelPickerAction::OpenMarketplace => cx.emit(ComposerEvent::OpenMarketplace),
                },
            ),
        ]
    }

    pub fn picker(&self) -> &Entity<ModelPicker> {
        &self.picker
    }

    /// Whether no agent is installed: the picker shows "No agents installed" with its Marketplace
    /// link, and Send stays off.
    pub fn set_no_agents_installed(&mut self, none_installed: bool, cx: &mut Context<Self>) {
        self.picker.update(cx, |picker, cx| {
            picker.set_no_agents_installed(none_installed, cx)
        });
        cx.notify();
    }

    /// Whether the picker reports that no agent is installed.
    pub fn no_agents_installed(&self, cx: &App) -> bool {
        self.picker.read(cx).state().no_agents_installed()
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
        if self.agent_busy == busy {
            return;
        }
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
        if self.disabled_reason == reason && !self.sign_in {
            return;
        }
        let disabled = reason.is_some();
        self.disabled_reason = reason;
        self.sign_in = false;
        if !disabled {
            self.resume = false;
        }
        self.text
            .update(cx, |area, cx| area.set_disabled(disabled, cx));
        cx.notify();
    }

    /// Offers Resume beside the reason of a disabled composer; pressing it emits `Resume`.
    pub fn set_resume_offered(&mut self, offered: bool, cx: &mut Context<Self>) {
        let offered = offered && self.disabled_reason.is_some();
        if self.resume != offered {
            self.resume = offered;
            cx.notify();
        }
    }

    /// Whether Resume is offered beside the reason.
    pub fn resume_offered(&self) -> bool {
        self.resume
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

    /// A placement card sits on the top edge: the composer spans the card's column so the card's inset lines up with
    /// its sides, and keeps its rounded corners, as the start artboard draws it.
    pub fn joined_top(mut self, joined: bool) -> Self {
        self.joined_top = joined;
        self
    }

    /// Lets Send and Enter go through with a blank draft.
    pub fn sends_empty(mut self) -> Self {
        self.sends_empty = true;
        self
    }

    pub fn set_enter_inserts_newline(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.enter_inserts_newline = enabled;
        cx.notify();
    }

    /// Why sending is off right now, or `None` when it is on; Enter and the primary button follow it.
    pub fn set_send_unavailable(&mut self, reason: Option<SharedString>, cx: &mut Context<Self>) {
        if self.send_unavailable != reason {
            self.send_unavailable = reason;
            cx.notify();
        }
    }

    /// The reason the last send failed, shown above the text.
    pub fn error_reason(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    /// Why the composer cannot take input right now, or `None` when it can.
    pub fn disabled_reason(&self) -> Option<&SharedString> {
        self.disabled_reason.as_ref()
    }

    /// Replaces the agents the picker offers. The chosen model stays when the new list still offers it; otherwise
    /// `preselect` is chosen when the list offers it, and the first model offered when it does not.
    pub fn set_picker_agents(
        &mut self,
        agents: Vec<AgentOption>,
        preselect: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let (choice, favourites, none_installed, fast_reason) = {
            let state = self.picker.read(cx).state();
            if state.agents() == agents.as_slice() {
                return;
            }
            (
                state.choice().clone(),
                state.favourites().to_vec(),
                state.no_agents_installed(),
                state.fast_reason().cloned(),
            )
        };
        let mut state = PickerState::new(agents, choice.clone()).with_favourites(favourites);
        state.set_no_agents_installed(none_installed);
        state.set_fast_reason(fast_reason);
        if state.locate(choice.model.as_ref()).is_none() {
            let first = state
                .agents()
                .iter()
                .position(|agent| !agent.models.is_empty())
                .map(|agent| RowRef { agent, model: 0 });
            let row = preselect
                .as_deref()
                .and_then(|id| state.locate(id))
                .or(first);
            if let Some(row) = row {
                state.select_model(row);
            }
        }
        self.picker = cx.new(|cx| ModelPicker::new(state, cx));
        self._picker_events = Self::subscribe_picker(&self.picker, cx);
        cx.notify();
    }

    /// Why Fast is off on every model while the service does not offer it; see `ModelPicker::set_fast_reason`.
    pub fn set_picker_fast_reason(&mut self, reason: Option<SharedString>, cx: &mut Context<Self>) {
        self.picker
            .update(cx, |picker, cx| picker.set_fast_reason(reason, cx));
    }

    fn situation(&self, cx: &App) -> Situation {
        let area = self.text.read(cx);
        Situation {
            agent_busy: self.agent_busy,
            host_confirmed: self.host_confirmed,
            disabled: self.disabled_reason.is_some()
                || self.send_unavailable.is_some()
                || self.no_agents_installed(cx),
            text_empty: area.text().is_empty() && !self.sends_empty,
            composing: area.is_composing(),
            queueing: self.queueing_offered,
        }
    }

    /// What the primary button does now.
    pub fn primary_action(&self, cx: &App) -> PrimaryAction {
        primary_action(self.situation(cx))
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
        if text.trim().is_empty() && !self.sends_empty {
            return;
        }
        self.history.push(&text);
        self.error = None;
        self.clear_hint();
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
    use gpui::{
        InteractiveElement, IntoElement, ParentElement, TestAppContext, VisualTestContext, div,
    };
    use std::{cell::RefCell, rc::Rc};

    fn picker_state() -> PickerState {
        let levels: Vec<SharedString> = vec!["Low".into(), "Medium".into()];
        PickerState::new(
            vec![AgentOption {
                kind: AgentKind::Claude,
                availability: AgentAvailability::Ready,
                list: ModelList::Loaded,
                brand: None,
                models: vec![ModelOption {
                    id: "opus".into(),
                    name: "Opus 5.5".into(),
                    efforts: levels,
                    default_effort: 1,
                    fast: false,
                    fast_reason: None,
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
                bypass: false,
            },
            AccessMode {
                name: "Workspace write".into(),
                icon: IconName::Shield,
                bypass: false,
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
    fn an_ended_session_offers_resume_beside_its_reason(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        let events = record(&composer, cx);
        composer.update(cx, |composer, cx| {
            composer.set_disabled_reason(Some("This session has ended.".into()), cx);
            composer.set_resume_offered(true, cx);
        });
        cx.run_until_parked();
        let button = cx
            .debug_bounds("composer-resume")
            .expect("Resume is drawn")
            .center();
        cx.simulate_click(button, gpui::Modifiers::none());
        assert_eq!(*events.borrow(), vec![ComposerEvent::Resume]);
    }

    #[gpui::test]
    fn resume_is_offered_only_while_the_composer_is_disabled_for_a_reason(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        let offered = |cx: &mut VisualTestContext| {
            composer.read_with(cx, |composer, _| composer.resume_offered())
        };
        composer.update(cx, |composer, cx| composer.set_resume_offered(true, cx));
        assert!(!offered(cx), "no reason, nothing to resume beside");
        composer.update(cx, |composer, cx| {
            composer.set_disabled_reason(Some("This session has ended.".into()), cx);
            composer.set_resume_offered(true, cx);
        });
        assert!(offered(cx));
        composer.update(cx, |composer, cx| composer.set_disabled_reason(None, cx));
        assert!(!offered(cx), "the offer ends with its reason");
    }

    #[gpui::test]
    fn a_composer_that_cannot_queue_stops_the_run_even_with_text_typed(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (composer, cx) =
            cx.add_window_view(|_, cx| Composer::new(picker_state(), access(), cx).queueing(false));
        composer.update(cx, |composer, cx| composer.set_busy(true, cx));
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("later");
        cx.simulate_keystrokes("enter");
        assert!(events.borrow().is_empty(), "Enter neither sends nor queues");
        assert_eq!(
            composer.read_with(cx, |composer, cx| composer.primary_action(cx)),
            PrimaryAction::Stop
        );
    }

    #[gpui::test]
    fn a_composer_that_sends_empty_sends_a_blank_draft_on_enter(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (composer, cx) =
            cx.add_window_view(|_, cx| Composer::new(picker_state(), access(), cx).sends_empty());
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_keystrokes("enter");
        assert_eq!(*events.borrow(), vec![ComposerEvent::Send(String::new())]);
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
    fn typing_then_enter_sends_the_text_and_keeps_the_draft_until_the_owner_clears_it(
        cx: &mut TestAppContext,
    ) {
        let (composer, cx) = composer_in(cx);
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("hello");
        cx.simulate_keystrokes("enter");
        assert_eq!(*events.borrow(), vec![ComposerEvent::Send("hello".into())]);
        assert_eq!(draft_of(&composer, cx), "hello", "not accepted yet");
        composer.update(cx, |composer, cx| composer.set_draft("", cx));
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
        composer.update(cx, |composer, cx| composer.set_draft("", cx));
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

    fn offered(kind: AgentKind, model: &str) -> AgentOption {
        AgentOption {
            kind,
            availability: AgentAvailability::Ready,
            list: ModelList::Loaded,
            brand: None,
            models: vec![ModelOption {
                id: model.into(),
                name: model.into(),
                efforts: Vec::new(),
                default_effort: 0,
                fast: false,
                fast_reason: None,
            }],
        }
    }

    fn chosen_model(composer: &Entity<Composer>, cx: &VisualTestContext) -> String {
        composer.read_with(cx, |composer, cx| {
            composer
                .picker()
                .read(cx)
                .state()
                .choice()
                .model
                .to_string()
        })
    }

    #[gpui::test]
    fn a_new_agent_list_keeps_a_choice_it_still_offers_and_takes_the_first_otherwise(
        cx: &mut TestAppContext,
    ) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| {
            composer.set_picker_agents(
                vec![
                    offered(AgentKind::Codex, "codex"),
                    offered(AgentKind::Claude, "opus"),
                ],
                None,
                cx,
            )
        });
        assert_eq!(chosen_model(&composer, cx), "opus");
        composer.update(cx, |composer, cx| {
            composer.set_picker_agents(vec![offered(AgentKind::Codex, "codex")], None, cx)
        });
        assert_eq!(chosen_model(&composer, cx), "codex");
    }

    #[gpui::test]
    fn a_new_agent_list_without_the_choice_takes_the_preselected_model_over_the_first(
        cx: &mut TestAppContext,
    ) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| {
            composer.set_picker_agents(
                vec![
                    offered(AgentKind::Codex, "codex"),
                    offered(AgentKind::Claude, "haiku"),
                ],
                Some("haiku".into()),
                cx,
            )
        });
        assert_eq!(chosen_model(&composer, cx), "haiku");
    }

    #[gpui::test]
    fn sending_stays_off_while_a_reason_is_set_and_follows_the_reason(cx: &mut TestAppContext) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| {
            composer.set_send_unavailable(Some("Choose an agent.".into()), cx)
        });
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("hi");
        cx.simulate_keystrokes("enter");
        assert!(events.borrow().is_empty());
        assert_eq!(draft_of(&composer, cx), "hi");
        composer.update(cx, |composer, cx| composer.set_send_unavailable(None, cx));
        cx.simulate_keystrokes("enter");
        assert_eq!(*events.borrow(), vec![ComposerEvent::Send("hi".into())]);
    }

    #[gpui::test]
    fn the_bottom_row_reads_attach_access_leading_then_the_model_chip_and_send(
        cx: &mut TestAppContext,
    ) {
        cx.update(bind_keys);
        let (_composer, cx) = cx.add_window_view(|_, cx| {
            Composer::new(picker_state(), access(), cx).leading_control(|_, _| {
                div()
                    .debug_selector(|| "composer-leading".to_owned())
                    .child("Chat")
                    .into_any_element()
            })
        });
        cx.run_until_parked();
        let order = [
            "composer-attach",
            "composer-access",
            "composer-leading",
            "model-picker-trigger",
            "composer-primary",
        ];
        let lefts: Vec<_> = order
            .iter()
            .map(|name| {
                cx.debug_bounds(name)
                    .unwrap_or_else(|| panic!("{name} is drawn"))
                    .left()
            })
            .collect();
        assert!(
            lefts.windows(2).all(|pair| pair[0] < pair[1]),
            "left to right: {order:?}, drawn at {lefts:?}"
        );
    }

    #[gpui::test]
    fn without_an_installed_agent_send_stays_off_and_the_marketplace_link_reaches_the_owner(
        cx: &mut TestAppContext,
    ) {
        let (composer, cx) = composer_in(cx);
        composer.update(cx, |composer, cx| {
            composer.set_no_agents_installed(true, cx)
        });
        assert!(composer.read_with(cx, |composer, cx| composer.no_agents_installed(cx)));
        let events = record(&composer, cx);
        focus_and_draw(&composer, cx);
        cx.simulate_input("hi");
        cx.simulate_keystrokes("enter");
        assert!(
            events
                .borrow()
                .iter()
                .all(|event| !matches!(event, ComposerEvent::Send(_))),
            "Send stays off"
        );
        assert_eq!(
            draft_of(&composer, cx),
            "hi",
            "the draft stays in the composer"
        );
        assert_eq!(
            composer.read_with(cx, |composer, cx| composer.primary_action(cx)),
            PrimaryAction::Disabled
        );
        let picker = composer.read_with(cx, |composer, _| composer.picker().clone());
        picker.update_in(cx, |picker, window, cx| picker.open(window, cx));
        cx.run_until_parked();
        let link = cx
            .debug_bounds("model-picker-marketplace")
            .expect("the link is drawn")
            .center();
        cx.simulate_click(link, gpui::Modifiers::none());
        assert_eq!(*events.borrow(), vec![ComposerEvent::OpenMarketplace]);
        composer.update(cx, |composer, cx| {
            composer.set_no_agents_installed(false, cx)
        });
        assert_eq!(
            composer.read_with(cx, |composer, cx| composer.primary_action(cx)),
            PrimaryAction::Send,
            "Send returns once an agent is installed"
        );
    }

    fn same_named_modes() -> Vec<AccessMode> {
        let auto = || AccessMode {
            name: "Auto".into(),
            icon: IconName::Shield,
            bypass: false,
        };
        vec![auto(), auto()]
    }

    #[gpui::test]
    fn the_access_detail_tells_two_same_named_modes_apart_in_the_menu_and_the_tooltip(
        cx: &mut TestAppContext,
    ) {
        use crate::components::button::test_support::{focus_and_draw as focus_element, press_key};
        cx.update(bind_keys);
        let (composer, cx) =
            cx.add_window_view(|_, cx| Composer::new(picker_state(), same_named_modes(), cx));
        composer.update(cx, |composer, cx| {
            composer.set_access_details(vec![Some("The CLI's default".into()), None], cx)
        });
        cx.run_until_parked();
        assert_eq!(
            composer.read_with(cx, |composer, _| composer
                .access_detail(0)
                .map(|text| text.to_string())),
            Some("The CLI's default".to_owned())
        );
        assert_eq!(
            composer.read_with(cx, |composer, _| composer.access_detail(1).cloned()),
            None
        );
        // Keyboard focus on the selected chip shows its detail as the tooltip.
        let access_focus = composer.read_with(cx, |composer, _| composer.access_focus.clone());
        cx.update(|window, _| window.activate_window());
        press_key(cx, "f1");
        focus_element(cx, &access_focus);
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("tooltip-panel").is_some(),
            "the chip's tooltip shows on keyboard focus"
        );
        // Opening the chip lists both modes; only the first row carries the detail line.
        let chip = cx
            .debug_bounds("composer-access")
            .expect("the chip is drawn")
            .center();
        cx.simulate_click(chip, gpui::Modifiers::none());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let with_detail = cx
            .debug_bounds("menu-item-0")
            .expect("the default row is drawn")
            .size
            .height;
        let plain = cx
            .debug_bounds("menu-item-1")
            .expect("the declared row is drawn")
            .size
            .height;
        assert!(
            f32::from(with_detail) > f32::from(plain),
            "the detail adds a second line to its row"
        );
    }

    #[gpui::test]
    fn access_details_stay_with_their_modes_until_the_modes_change(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (composer, cx) =
            cx.add_window_view(|_, cx| Composer::new(picker_state(), access(), cx));
        composer.update(cx, |composer, cx| {
            composer.set_access_details(vec![Some("The CLI's default".into())], cx)
        });
        composer.update(cx, |composer, cx| {
            composer.set_access_modes(access(), 0, cx)
        });
        assert_eq!(
            composer.read_with(cx, |composer, _| composer.access_detail(0).cloned()),
            Some("The CLI's default".into()),
            "unchanged modes keep their details"
        );
        composer.update(cx, |composer, cx| {
            composer.set_access_modes(same_named_modes(), 0, cx)
        });
        assert_eq!(
            composer.read_with(cx, |composer, _| composer.access_detail(0).cloned()),
            None,
            "changed modes drop the details that named the old list"
        );
    }
}
