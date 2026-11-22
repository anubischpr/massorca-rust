//! Secret request card (docs/design/components/SecretRequestCard.md): a card in the conversation
//! that asks for a secret by its variable name. It holds no value: Add opens the Secret modal,
//! and the value never enters the card, the chat or the logs.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SIZE_AVATAR_MD, SIZE_CONTROL_COMFORTABLE,
    SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION, TYPE_LABEL,
    TYPE_MONO_SM,
};
use gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px,
};
use std::rc::Rc;

/// The longest variable name the spec allows.
pub const MAX_NAME_LEN: usize = 64;

/// Whether a name is a variable name: letters, digits and underscores, not starting with a
/// digit, and at most 64 characters.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Where a request stands. The card keeps only this, never a value.
#[derive(Clone, Debug, PartialEq)]
pub enum SecretState {
    /// Asked, waiting for the user.
    Pending,
    /// The Secret modal is open.
    Entering,
    Stored,
    /// Points at a secret of another project.
    Linked,
    Declined,
    /// The save failed; the reason is shown.
    Failed {
        reason: SharedString,
    },
}

impl SecretState {
    /// The word beside the glyph; a failed save names its reason.
    pub fn word(&self) -> String {
        match self {
            Self::Pending => "Waiting for you".to_owned(),
            Self::Entering => "Entering secret".to_owned(),
            Self::Stored => "Stored".to_owned(),
            Self::Linked => "Linked".to_owned(),
            Self::Declined => "Declined".to_owned(),
            Self::Failed { reason } => format!("Not saved: {reason}"),
        }
    }

    fn glyph(&self) -> IconName {
        match self {
            Self::Pending | Self::Entering => IconName::CircleHelp,
            Self::Stored => IconName::CircleCheck,
            Self::Linked => IconName::Link,
            Self::Declined => IconName::Ban,
            Self::Failed { .. } => IconName::OctagonAlert,
        }
    }

    /// A settled request collapses to one compact row.
    pub fn compact(&self) -> bool {
        matches!(self, Self::Stored | Self::Linked | Self::Declined)
    }
}

/// What a press on the card asks for. The caller opens the modal or updates the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretAction {
    Add,
    Decline,
    Retry,
    OpenVault,
}

type ActionHandler = Rc<dyn Fn(SecretAction, &mut Window, &mut App)>;

/// A press handler for a button.
fn emit(
    handler: &Option<ActionHandler>,
    action: SecretAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| {
        if let Some(handler) = &handler {
            handler(action, window, cx);
        }
    }
}

/// A press handler for a plain element, whose click also carries the event.
fn emit_click(
    handler: &Option<ActionHandler>,
    action: SecretAction,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |_, window, cx| {
        if let Some(handler) = &handler {
            handler(action, window, cx);
        }
    }
}

/// A card that asks for one secret by name, with the action for its state.
#[derive(IntoElement)]
pub struct SecretRequestCard {
    id: ElementId,
    name: SharedString,
    title: SharedString,
    reason: Option<SharedString>,
    scope: SharedString,
    host: SharedString,
    state: SecretState,
    unreachable: bool,
    add_focus: Option<FocusHandle>,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for SecretRequestCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretRequestCard")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl SecretRequestCard {
    /// `title` is "Claude needs a secret" or "Add a secret for <project>".
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        title: impl Into<SharedString>,
        state: SecretState,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            title: title.into(),
            reason: None,
            scope: SharedString::default(),
            host: "this PC".into(),
            state,
            unreachable: false,
            add_focus: None,
            on_action: None,
        }
    }

    /// Why the agent needs the secret, as plain text; only the first 200 characters are kept.
    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        let reason: SharedString = reason.into();
        let reason: String = reason.chars().take(200).collect();
        self.reason = Some(reason.into());
        self
    }

    /// Where the secret belongs, such as "for project oppm".
    pub fn scope(mut self, scope: impl Into<SharedString>) -> Self {
        self.scope = scope.into();
        self
    }

    /// The host that stores the value, named in the safety line.
    pub fn host(mut self, host: impl Into<SharedString>) -> Self {
        self.host = host.into();
        self
    }

    /// The host cannot be reached: Add is disabled and nothing can be saved; Decline still works.
    pub fn unreachable(mut self, unreachable: bool) -> Self {
        self.unreachable = unreachable;
        self
    }

    /// The focus handle of Add; focus returns here when the modal closes.
    pub fn add_focus(mut self, handle: FocusHandle) -> Self {
        self.add_focus = Some(handle);
        self
    }

    /// Runs with the action a press asks for.
    pub fn on_action(
        mut self,
        listener: impl Fn(SecretAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }
}

/// The variable name in a mono chip; it stays selectable and copyable text.
fn name_chip(name: &SharedString, p: &Palette) -> AnyElement {
    div()
        .flex_none()
        .px(px(SPACE_2))
        .py(px(crate::tokens::SPACE_HALF))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(p.border.subtle)
        .bg(p.surface.sunken)
        .text_style(TYPE_MONO_SM)
        .text_color(p.text.primary)
        .child(name.clone())
        .into_any_element()
}

/// A button wrapped so a test can find it by its selector.
fn wrapped(selector: &'static str, button: Button) -> AnyElement {
    div()
        .debug_selector(move || selector.into())
        .child(button)
        .into_any_element()
}

impl RenderOnce for SecretRequestCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = self.id.clone();
        let word = self.state.word();
        let ink: Color = match self.state {
            SecretState::Pending => p.agent.waiting,
            SecretState::Failed { .. } => p.status.danger_fg,
            SecretState::Stored => p.status.success_fg,
            _ => p.text.secondary,
        };
        let state_line = div()
            .id((id.clone(), "state"))
            .role(Role::Status)
            .flex_none()
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .text_style(TYPE_CAPTION)
            .text_color(ink)
            .child(
                Icon::new(self.state.glyph())
                    .size(IconSize::XSmall)
                    .color(ink),
            )
            .child(word.clone());
        let handler = self.on_action.clone();

        if self.state.compact() {
            let mut row = div()
                .id(id.clone())
                .role(Role::Group)
                .aria_label(format!("Secret request: {}", self.name))
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .min_h(px(SIZE_CONTROL_COMFORTABLE))
                .px(px(SPACE_3))
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(p.border.default)
                .bg(p.surface.raised)
                .child(state_line)
                .child(name_chip(&self.name, &p))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.secondary)
                        .child(self.scope.clone()),
                );
            if matches!(self.state, SecretState::Stored | SecretState::Linked) {
                row = row.child(
                    div()
                        .id((id.clone(), "vault"))
                        .flex_none()
                        .cursor_pointer()
                        .text_style(TYPE_LABEL)
                        .text_color(p.accent.fg)
                        .on_click(emit_click(&handler, SecretAction::OpenVault))
                        .child("Open vault"),
                );
            }
            return row.into_any_element();
        }

        let entering = self.state == SecretState::Entering;
        let valid = is_valid_name(&self.name);
        let add_enabled = valid && !self.unreachable && !entering;
        let pending_fill = matches!(self.state, SecretState::Pending | SecretState::Entering);
        let fill = if pending_fill {
            p.agent.waiting_bg
        } else {
            p.surface.raised
        };
        let mut actions = div().flex().justify_end().gap(px(SPACE_2));
        match self.state {
            SecretState::Pending | SecretState::Entering => {
                let mut add = Button::new((id.clone(), "add"), "Add secret")
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Default)
                    .disabled(!add_enabled)
                    .on_click(emit(&handler, SecretAction::Add));
                if let Some(focus) = self.add_focus.clone() {
                    add = add.focus_handle(focus);
                }
                actions = actions.child(wrapped("add", add)).child(wrapped(
                    "decline",
                    Button::new((id.clone(), "decline"), "Decline")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Default)
                        .disabled(entering)
                        .on_click(emit(&handler, SecretAction::Decline)),
                ));
            }
            SecretState::Failed { .. } => {
                actions = actions
                    .child(wrapped(
                        "retry",
                        Button::new((id.clone(), "retry"), "Retry")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Default)
                            .disabled(self.unreachable || !valid)
                            .on_click(emit(&handler, SecretAction::Retry)),
                    ))
                    .child(wrapped(
                        "decline",
                        Button::new((id.clone(), "decline"), "Decline")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Default)
                            .on_click(emit(&handler, SecretAction::Decline)),
                    ));
            }
            _ => {}
        }

        let header = div()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .child(
                div()
                    .flex_none()
                    .size(px(SIZE_AVATAR_MD))
                    .rounded(px(RADIUS_MD))
                    .bg(p.surface.sunken)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::KeyRound)
                            .size(IconSize::Medium)
                            .color(p.text.secondary),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_BODY_STRONG)
                    .child(self.title.clone()),
            )
            .child(state_line);
        let mut card = div()
            .id(id.clone())
            .role(Role::Group)
            .aria_label(format!("Secret request: {}", self.name))
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .p(px(SPACE_3))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
            .bg(fill)
            .text_color(p.text.primary)
            .child(header)
            .child(name_chip(&self.name, &p));
        if let Some(reason) = &self.reason {
            card = card.child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .line_clamp(3)
                    .child(reason.clone()),
            );
        }
        card = card.child(actions).child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_1))
                .text_style(TYPE_CAPTION)
                .text_color(p.text.tertiary)
                .child(
                    Icon::new(IconName::Lock)
                        .size(IconSize::XSmall)
                        .color(p.text.tertiary),
                )
                .child(format!(
                    "The value is stored on {} and never goes into the chat.",
                    self.host
                )),
        );
        if self.unreachable {
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.agent.unreachable)
                    .child(
                        Icon::new(IconName::WifiOff)
                            .size(IconSize::XSmall)
                            .color(p.agent.unreachable),
                    )
                    .child(format!(
                        "Can't reach {}. Nothing can be saved until it is back.",
                        self.host
                    )),
            );
        }
        card.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn names_follow_the_variable_rule() {
        assert!(is_valid_name("OPENAI_API_KEY"));
        assert!(is_valid_name("_token2"));
        assert!(is_valid_name(&"A".repeat(MAX_NAME_LEN)));
        assert!(!is_valid_name("2fa"));
        assert!(!is_valid_name("API-KEY"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name(&"A".repeat(MAX_NAME_LEN + 1)));
    }

    #[test]
    fn states_carry_the_spec_words_and_settle_into_rows() {
        assert_eq!(SecretState::Pending.word(), "Waiting for you");
        assert_eq!(SecretState::Stored.word(), "Stored");
        assert_eq!(
            SecretState::Failed {
                reason: "disk full".into()
            }
            .word(),
            "Not saved: disk full"
        );
        assert!(SecretState::Linked.compact());
        assert!(!SecretState::Pending.compact());
        assert!(!SecretState::Entering.compact());
    }

    #[test]
    fn a_reason_keeps_its_first_two_hundred_characters() {
        let card =
            SecretRequestCard::new("c", "KEY", "Claude needs a secret", SecretState::Pending)
                .reason("x".repeat(250));
        assert_eq!(card.reason.as_ref().map(|r| r.len()), Some(200));
    }

    #[gpui::test]
    fn add_and_decline_send_their_actions(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| ActionHarness {
            seen: seen.clone(),
            unreachable: false,
        });
        cx.run_until_parked();

        let add = cx.debug_bounds("add").expect("add is drawn").center();
        cx.simulate_click(add, gpui::Modifiers::none());
        let decline = cx
            .debug_bounds("decline")
            .expect("decline is drawn")
            .center();
        cx.simulate_click(decline, gpui::Modifiers::none());

        assert_eq!(
            *seen.borrow(),
            vec![SecretAction::Add, SecretAction::Decline]
        );
    }

    #[gpui::test]
    fn add_is_disabled_while_the_host_is_unreachable(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| ActionHarness {
            seen: seen.clone(),
            unreachable: true,
        });
        cx.run_until_parked();

        let add = cx.debug_bounds("add").expect("add is drawn").center();
        cx.simulate_click(add, gpui::Modifiers::none());
        assert!(seen.borrow().is_empty());
    }

    /// Hosts one pending request, reachable or not, and records what its buttons send.
    struct ActionHarness {
        seen: Rc<RefCell<Vec<SecretAction>>>,
        unreachable: bool,
    }

    impl gpui::Render for ActionHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            div().child(
                SecretRequestCard::new(
                    "key",
                    "OPENAI_API_KEY",
                    "Claude needs a secret",
                    SecretState::Pending,
                )
                .reason("Calls the OpenAI API for the project.")
                .unreachable(self.unreachable)
                .on_action(move |action, _, _| seen.borrow_mut().push(action)),
            )
        }
    }
}
