//! Notification cards and their stack (docs/design/components/NotificationCard.md): a
//! persistent card that asks for attention and offers at most two actions. The stack shows the
//! three highest-priority cards and folds the rest into a "+N more" row.

use crate::components::agent_status::{AgentState, AgentStatus};
use crate::components::button::{Button, ButtonSize, ButtonVariant, ClickHandler};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::progress::Spinner;
use crate::components::switch::ease;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    Color, DURATION_MODERATE, EASE_ENTER, LAYOUT_TOAST_WIDTH_DEFAULT, LAYOUT_TOAST_WIDTH_MAX,
    LAYOUT_TOAST_WIDTH_MIN, Palette, RADIUS_LG, RADIUS_MD, SIZE_AVATAR_MD, SIZE_PROGRESS_HEIGHT,
    SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION,
};
use gpui::{
    Animation, AnimationExt, App, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, relative,
};
use std::rc::Rc;
use std::sync::Arc;

/// Cards a stack shows before it folds the rest into a "+N more" row.
pub const MAX_VISIBLE: usize = 3;
/// Actions a card holds; the spec allows two.
const MAX_ACTIONS: usize = 2;

/// What a card is about. It sets the plate, the glyph and the meta word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationKind {
    Attention,
    Error,
    Update,
    SignIn,
    Info,
}

impl NotificationKind {
    /// The meta word; it is always shown, so the kind never depends on colour alone.
    pub fn word(self) -> &'static str {
        match self {
            Self::Attention => "Waiting for you",
            Self::Error => "Error",
            Self::Update => "Update",
            Self::SignIn => "Sign-in needed",
            Self::Info => "Note",
        }
    }

    /// Lower sorts higher in a stack: attention and errors, then sign-in, updates and notes.
    pub fn priority(self) -> u8 {
        match self {
            Self::Attention | Self::Error => 0,
            Self::SignIn => 1,
            Self::Update => 2,
            Self::Info => 3,
        }
    }

    fn glyph(self) -> IconName {
        match self {
            Self::Attention => IconName::CircleHelp,
            Self::Error => IconName::TriangleAlert,
            Self::Update => IconName::Download,
            Self::SignIn => IconName::User,
            Self::Info => IconName::Info,
        }
    }

    /// The plate fill and glyph colour of the kind.
    fn plate(self, p: &Palette) -> (Color, Color) {
        match self {
            Self::Attention => (p.agent.waiting_bg_overlay, p.agent.waiting),
            Self::Error => (p.agent.error_bg_overlay, p.agent.error),
            Self::Update => (p.status.info_bg, p.status.info_fg),
            Self::SignIn => (p.status.warning_bg, p.status.warning_fg),
            Self::Info => (p.surface.sunken, p.text.secondary),
        }
    }
}

/// The update card's state; its title, progress and actions follow it.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Available { version: SharedString },
    Checking,
    Downloading { percent: u8 },
    Ready,
    Failed { reason: SharedString },
}

impl UpdateState {
    /// A failed update is an error; every other state is an update.
    pub fn kind(&self) -> NotificationKind {
        match self {
            Self::Failed { .. } => NotificationKind::Error,
            _ => NotificationKind::Update,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Available { version } => format!("MassOrca {version} is available"),
            Self::Checking => "Checking for updates".to_owned(),
            Self::Downloading { percent } => format!("Downloading, {percent}%"),
            Self::Ready => "Restart to finish the update".to_owned(),
            Self::Failed { reason } => format!("Update failed: {reason}"),
        }
    }

    /// The bar fraction while downloading.
    pub fn progress(&self) -> Option<f32> {
        match self {
            Self::Downloading { percent } => Some(f32::from(*percent) / 100.0),
            _ => None,
        }
    }

    /// Action labels, primary first; the caller maps each one to a Button.
    pub fn action_labels(&self) -> &'static [&'static str] {
        match self {
            Self::Available { .. } => &["Download", "Later"],
            Self::Ready => &["Restart now", "Later"],
            Self::Failed { .. } => &["Retry"],
            Self::Checking | Self::Downloading { .. } => &[],
        }
    }
}

/// A floating card that asks for attention: a plate, a title, a body, a meta line and up to two
/// actions. The card is a group; its controls take focus, not the card itself.
#[derive(IntoElement)]
pub struct NotificationCard {
    id: ElementId,
    kind: NotificationKind,
    title: SharedString,
    body: Option<SharedString>,
    source: SharedString,
    age: SharedString,
    /// Each action, and whether it answers the session (disabled while the host is unreachable).
    actions: Vec<(Button, bool)>,
    progress: Option<f32>,
    spinner: bool,
    unreachable: bool,
    inserting: bool,
    width: f32,
    focus: Option<FocusHandle>,
    on_dismiss: Option<ClickHandler>,
}

impl std::fmt::Debug for NotificationCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationCard")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("title", &self.title)
            .field("actions", &self.actions.len())
            .finish_non_exhaustive()
    }
}

impl NotificationCard {
    pub fn new(
        id: impl Into<ElementId>,
        kind: NotificationKind,
        title: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            body: None,
            source: SharedString::default(),
            age: SharedString::default(),
            actions: Vec::new(),
            progress: None,
            spinner: false,
            unreachable: false,
            inserting: false,
            width: LAYOUT_TOAST_WIDTH_DEFAULT,
            focus: None,
            on_dismiss: None,
        }
    }

    pub fn kind(&self) -> NotificationKind {
        self.kind
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Where the card comes from: a session, a project or an extension.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = source.into();
        self
    }

    pub fn age(mut self, age: impl Into<SharedString>) -> Self {
        self.age = age.into();
        self
    }

    /// An action that always works, such as "Open".
    pub fn action(self, button: Button) -> Self {
        self.push_action(button, false)
    }

    /// An action that answers the session; it is disabled while the host is unreachable.
    pub fn answer(self, button: Button) -> Self {
        self.push_action(button, true)
    }

    fn push_action(mut self, button: Button, answer: bool) -> Self {
        if self.actions.len() < MAX_ACTIONS {
            self.actions.push((button, answer));
        }
        self
    }

    /// A 4 px bar for work in progress, from 0 to 1.
    pub fn progress(mut self, fraction: f32) -> Self {
        self.progress = Some(fraction.clamp(0.0, 1.0));
        self
    }

    /// Shows a spinner beside the title while the update checks.
    pub fn spinner(mut self, spinning: bool) -> Self {
        self.spinner = spinning;
        self
    }

    /// The host cannot be reached: the card reads the last known state, with the unreachable glyph.
    pub fn unreachable(mut self, unreachable: bool) -> Self {
        self.unreachable = unreachable;
        self
    }

    /// Fades the card in and lifts it 8 px over `duration.moderate` (instant with reduced motion).
    pub fn inserting(mut self, inserting: bool) -> Self {
        self.inserting = inserting;
        self
    }

    /// The card width, clamped to the toast range of 320 to 400 px.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.clamp(LAYOUT_TOAST_WIDTH_MIN, LAYOUT_TOAST_WIDTH_MAX);
        self
    }

    /// Keyboard focus lives with the caller or the stack; otherwise the card keeps its own handle.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    /// Runs when the dismiss control is pressed or Escape is pressed on the focused card.
    pub fn on_dismiss(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(listener));
        self
    }

    /// The body line, or the last known state while the host is unreachable.
    fn body_text(&self) -> Option<SharedString> {
        if self.unreachable {
            let line = format!(
                "Last known: {}, {}",
                self.kind.word().to_lowercase(),
                self.age
            );
            Some(line.into())
        } else {
            self.body.clone()
        }
    }

    /// Source, kind word and age, joined with a dot; an empty part is left out.
    fn meta(&self) -> String {
        [
            self.source.to_string(),
            self.kind.word().to_owned(),
            self.age.to_string(),
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
    }
}

impl RenderOnce for NotificationCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = self.id.clone();
        let unreachable = self.unreachable;
        let (plate_fill, plate_ink) = if unreachable {
            (p.agent.unreachable_bg_overlay, p.agent.unreachable)
        } else {
            self.kind.plate(&p)
        };
        let alert = self.kind == NotificationKind::Error;
        let mut root = div()
            .id(id.clone())
            .role(if alert { Role::Alert } else { Role::Status })
            .aria_label(format!("{}: {}", self.kind.word(), self.title))
            .relative()
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .w(px(self.width))
            .p(px(SPACE_3))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
            .bg(p.surface.overlay)
            .shadow(box_shadows(theme.shadows.e2))
            .text_color(p.text.primary);
        if let Some(focus) = &self.focus {
            root = root.track_focus(focus);
        }
        if let Some(dismiss) = self.on_dismiss.clone() {
            root = root.on_key_down(move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    dismiss(window, cx);
                }
            });
        }

        let mark = if unreachable {
            AgentStatus::new(AgentState::Unreachable).into_any_element()
        } else {
            Icon::new(self.kind.glyph())
                .size(IconSize::Small)
                .color(plate_ink)
                .into_any_element()
        };
        let plate = div()
            .flex_none()
            .size(px(SIZE_AVATAR_MD))
            .rounded(px(RADIUS_MD))
            .bg(plate_fill)
            .flex()
            .items_center()
            .justify_center()
            .child(mark);

        let mut title_row = div()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .min_w_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_BODY_STRONG)
                    .child(self.title.clone()),
            );
        if self.spinner {
            title_row = title_row.child(Spinner::new().size(IconSize::Small));
        }
        let mut content = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(title_row);
        if let Some(body) = self.body_text() {
            content = content.child(
                div()
                    .min_w_0()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .line_clamp(2)
                    .child(body),
            );
        }
        if let Some(fraction) = self.progress {
            content = content.child(progress_bar((id.clone(), "progress"), fraction, &p));
        }
        content = content.child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_CAPTION)
                .tabular_figures()
                .text_color(p.text.tertiary)
                .child(self.meta()),
        );

        let mut head = div()
            .flex()
            .items_start()
            .gap(px(SPACE_2))
            .child(plate)
            .child(content);
        if let Some(dismiss) = self.on_dismiss.clone() {
            let name = format!("Dismiss {}", self.title);
            head = head.child(
                div().debug_selector(|| "dismiss".into()).child(
                    IconButton::new((id.clone(), "dismiss"), IconName::X, name)
                        .variant(IconButtonVariant::Ghost)
                        .size(ButtonSize::Compact)
                        .on_click(move |window, cx| dismiss(window, cx)),
                ),
            );
        }
        root = root.child(head);

        if !self.actions.is_empty() {
            let buttons: Vec<Button> = self
                .actions
                .into_iter()
                .map(|(button, answer)| {
                    if answer && unreachable {
                        button.disabled(true)
                    } else {
                        button
                    }
                })
                .collect();
            root = root.child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(SPACE_2))
                    .children(buttons),
            );
        }

        if self.inserting && !cx.reduce_motion() {
            root.with_animation(
                (id, "notification-insert"),
                Animation::new(DURATION_MODERATE).with_easing(|t| ease(EASE_ENTER, t)),
                |card, t| card.opacity(t).top(px(SPACE_2 * (1.0 - t))),
            )
            .into_any_element()
        } else {
            root.into_any_element()
        }
    }
}

/// The 4 px bar of a card in progress: a track with an accent fill, announced as progress.
fn progress_bar(id: impl Into<ElementId>, fraction: f32, p: &Palette) -> impl IntoElement {
    let fraction = fraction.clamp(0.0, 1.0);
    div()
        .id(id)
        .role(Role::ProgressIndicator)
        .aria_label("Progress")
        .aria_numeric_value(f64::from(fraction * 100.0))
        .h(px(SIZE_PROGRESS_HEIGHT))
        .w_full()
        .rounded(px(SIZE_PROGRESS_HEIGHT / 2.0))
        .bg(p.progress.track)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(fraction))
                .rounded(px(SIZE_PROGRESS_HEIGHT / 2.0))
                .bg(p.accent.default),
        )
}

/// Cards in stack order, top first: priority, then newest first. The input is oldest first.
pub fn stack_order(kinds: &[NotificationKind]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..kinds.len()).collect();
    order.sort_by_key(|&index| (kinds[index].priority(), std::cmp::Reverse(index)));
    order
}

/// How many cards are drawn: the first three, or all of them once the stack is expanded.
pub fn stack_visible(total: usize, expanded: bool) -> usize {
    if expanded {
        total
    } else {
        total.min(MAX_VISIBLE)
    }
}

/// The card's own focus handle, or one the window keeps for the card's id.
fn card_focus(card: &NotificationCard, window: &mut Window, cx: &mut App) -> FocusHandle {
    card.focus.clone().unwrap_or_else(|| {
        let key = ElementId::NamedChild(Arc::new(card.id.clone()), "focus".into());
        window
            .use_keyed_state(key, cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
    })
}

/// The cards of one corner: the top card first, the rest behind a "+N more" row. Up and Down
/// move focus between the visible cards.
#[derive(IntoElement)]
pub struct NotificationStack {
    id: ElementId,
    cards: Vec<NotificationCard>,
    expanded: bool,
    width: Option<f32>,
    max_height: Option<f32>,
    on_expand: Option<ClickHandler>,
}

impl std::fmt::Debug for NotificationStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationStack")
            .field("id", &self.id)
            .field("cards", &self.cards.len())
            .field("expanded", &self.expanded)
            .finish_non_exhaustive()
    }
}

impl NotificationStack {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            cards: Vec::new(),
            expanded: false,
            width: None,
            max_height: None,
            on_expand: None,
        }
    }

    /// The cards in creation order, oldest first; the stack sorts them.
    pub fn cards(mut self, cards: Vec<NotificationCard>) -> Self {
        self.cards = cards;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Applies one width to every card, for a Conversation narrower than 400 px.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Caps the expanded stack; it scrolls past this height.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Runs when the "+N more" row is pressed; the caller keeps `expanded` and sets it again.
    pub fn on_expand(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_expand = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for NotificationStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.theme().palette;
        let Self {
            id,
            cards,
            expanded,
            width,
            max_height,
            on_expand,
        } = self;
        let kinds: Vec<NotificationKind> = cards.iter().map(NotificationCard::kind).collect();
        let order = stack_order(&kinds);
        let shown = stack_visible(order.len(), expanded);
        let folded = order.len() - shown;
        let mut slots: Vec<Option<NotificationCard>> = cards.into_iter().map(Some).collect();

        let mut column = div()
            .id(id.clone())
            .role(Role::Region)
            .aria_label("Notifications")
            .flex()
            .flex_col()
            .gap(px(SPACE_2));
        if let Some(height) = max_height {
            column = column.max_h(px(height)).overflow_y_scroll();
        }
        if let (true, Some(expand)) = (folded > 0, on_expand) {
            column = column.child(
                div()
                    .debug_selector(|| "folded".into())
                    .flex()
                    .justify_center()
                    .rounded(px(RADIUS_LG))
                    .border_1()
                    .border_color(p.border.default)
                    .bg(p.surface.overlay)
                    .child(
                        Button::new((id.clone(), "more"), format!("+{folded} more"))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Compact)
                            .on_click(move |window, cx| expand(window, cx)),
                    ),
            );
        }

        let mut handles = Vec::with_capacity(shown);
        for &index in &order[..shown] {
            if let Some(card) = slots[index].take() {
                let handle = card_focus(&card, window, cx);
                handles.push(handle.clone());
                let mut card = card.focus_handle(handle);
                if let Some(width) = width {
                    card = card.width(width);
                }
                column = column.child(card);
            }
        }

        column.on_key_down(move |event: &KeyDownEvent, window, cx| {
            let step: isize = match event.keystroke.key.as_str() {
                "up" => -1,
                "down" => 1,
                _ => return,
            };
            let Some(current) = handles.iter().position(|handle| handle.is_focused(window)) else {
                return;
            };
            let next = (current as isize + step).clamp(0, handles.len() as isize - 1) as usize;
            cx.stop_propagation();
            handles[next].focus(window, cx);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use std::cell::Cell;

    #[test]
    fn kinds_carry_the_spec_words_and_priorities() {
        assert_eq!(NotificationKind::Attention.word(), "Waiting for you");
        assert_eq!(NotificationKind::SignIn.word(), "Sign-in needed");
        assert_eq!(NotificationKind::Info.word(), "Note");
        assert_eq!(
            NotificationKind::Attention.priority(),
            NotificationKind::Error.priority()
        );
        assert!(NotificationKind::Error.priority() < NotificationKind::SignIn.priority());
        assert!(NotificationKind::SignIn.priority() < NotificationKind::Update.priority());
        assert!(NotificationKind::Update.priority() < NotificationKind::Info.priority());
    }

    #[test]
    fn stack_puts_priority_first_and_newest_first_within_it() {
        use NotificationKind::{Attention, Info, Update};
        let kinds = [Info, Attention, Update, Attention];
        assert_eq!(stack_order(&kinds), vec![3, 1, 2, 0]);
    }

    #[test]
    fn stack_shows_three_and_folds_the_rest_until_expanded() {
        assert_eq!(stack_visible(2, false), 2);
        assert_eq!(stack_visible(5, false), MAX_VISIBLE);
        assert_eq!(stack_visible(5, true), 5);
    }

    #[test]
    fn update_states_follow_the_spec_copy() {
        let available = UpdateState::Available {
            version: "1.4.2".into(),
        };
        assert_eq!(available.title(), "MassOrca 1.4.2 is available");
        assert_eq!(available.action_labels(), &["Download", "Later"]);
        let failed = UpdateState::Failed {
            reason: "offline".into(),
        };
        assert_eq!(failed.kind(), NotificationKind::Error);
        assert_eq!(failed.title(), "Update failed: offline");
        assert_eq!(failed.action_labels(), &["Retry"]);
        let downloading = UpdateState::Downloading { percent: 64 };
        assert_eq!(downloading.title(), "Downloading, 64%");
        assert!(
            downloading
                .progress()
                .is_some_and(|f| (f - 0.64).abs() < 1e-6)
        );
        assert!(UpdateState::Checking.action_labels().is_empty());
    }

    #[test]
    fn unreachable_card_reads_its_last_known_state() {
        let card = NotificationCard::new("c", NotificationKind::Attention, "Run?")
            .age("5 min ago")
            .unreachable(true);
        let body = card.body_text().expect("a body line");
        assert_eq!(body.as_ref(), "Last known: waiting for you, 5 min ago");
    }

    #[test]
    fn meta_line_skips_empty_parts_and_answers_are_flagged() {
        let card = NotificationCard::new("c", NotificationKind::Update, "x")
            .source("Acme")
            .answer(Button::new("a", "Approve"))
            .action(Button::new("o", "Open"))
            .action(Button::new("extra", "Ignored"));
        assert_eq!(card.meta(), "Acme · Update");
        let flags: Vec<bool> = card.actions.iter().map(|(_, answer)| *answer).collect();
        assert_eq!(flags, vec![true, false], "a third action is dropped");
    }

    #[gpui::test]
    fn dismiss_button_and_escape_dismiss_and_delete_does_not(cx: &mut gpui::TestAppContext) {
        let dismissed = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| DismissHarness::new(cx, dismissed.clone()));
        cx.run_until_parked();

        let center = cx
            .debug_bounds("dismiss")
            .expect("dismiss is drawn")
            .center();
        cx.simulate_click(center, gpui::Modifiers::none());
        assert_eq!(dismissed.get(), 1);

        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "delete");
        assert_eq!(dismissed.get(), 1, "delete never dismisses");
        press_key(cx, "escape");
        assert_eq!(dismissed.get(), 2);
    }

    #[gpui::test]
    fn arrow_keys_move_focus_between_cards(cx: &mut gpui::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| StackHarness::new(cx));
        cx.run_until_parked();
        let (first, second) = view.read_with(cx, |h, _| (h.first.clone(), h.second.clone()));

        // The newest card sits on top, so the first card is the one below it.
        focus_and_draw(cx, &second);
        press_key(cx, "down");
        assert!(cx.update(|window, _| first.is_focused(window)));
        press_key(cx, "up");
        assert!(cx.update(|window, _| second.is_focused(window)));
    }

    #[gpui::test]
    fn folded_row_expands_the_stack(cx: &mut gpui::TestAppContext) {
        let expanded = Rc::new(Cell::new(0_u32));
        let (_, cx) = cx.add_window_view(|_, _| FoldHarness {
            expanded: expanded.clone(),
        });
        cx.run_until_parked();

        let center = cx.debug_bounds("folded").expect("folded row").center();
        cx.simulate_click(center, gpui::Modifiers::none());
        assert_eq!(expanded.get(), 1);
    }

    /// Hosts one card that dismisses; its focus handle lets the test focus it.
    struct DismissHarness {
        focus: FocusHandle,
        dismissed: Rc<Cell<u32>>,
    }

    impl DismissHarness {
        fn new(cx: &mut gpui::Context<Self>, dismissed: Rc<Cell<u32>>) -> Self {
            Self {
                focus: cx.focus_handle(),
                dismissed,
            }
        }
    }

    impl gpui::Render for DismissHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let dismissed = self.dismissed.clone();
            div().child(
                NotificationCard::new("card", NotificationKind::Attention, "Run command?")
                    .focus_handle(self.focus.clone())
                    .on_dismiss(move |_, _| dismissed.set(dismissed.get() + 1)),
            )
        }
    }

    /// Hosts a two-card stack whose cards hold focus handles the test can read.
    struct StackHarness {
        first: FocusHandle,
        second: FocusHandle,
    }

    impl StackHarness {
        fn new(cx: &mut gpui::Context<Self>) -> Self {
            Self {
                first: cx.focus_handle(),
                second: cx.focus_handle(),
            }
        }
    }

    impl gpui::Render for StackHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            NotificationStack::new("stack").cards(vec![
                NotificationCard::new("one", NotificationKind::Error, "First")
                    .focus_handle(self.first.clone()),
                NotificationCard::new("two", NotificationKind::Error, "Second")
                    .focus_handle(self.second.clone()),
            ])
        }
    }

    /// Hosts a stack of four cards, one folded, with an expand handler that counts.
    struct FoldHarness {
        expanded: Rc<Cell<u32>>,
    }

    impl gpui::Render for FoldHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let expanded = self.expanded.clone();
            let cards = (0_usize..4)
                .map(|index| {
                    NotificationCard::new(
                        ("card", index),
                        NotificationKind::Update,
                        format!("Update {index}"),
                    )
                })
                .collect();
            NotificationStack::new("stack")
                .cards(cards)
                .on_expand(move |_, _| expanded.set(expanded.get() + 1))
        }
    }
}
