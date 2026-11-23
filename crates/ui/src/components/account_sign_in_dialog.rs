//! Account sign-in dialog: one dialog that follows a sign-in through each of its stages.

use crate::components::dialog::Dialog;
use crate::components::error_state::{ErrorState, ErrorStateVariant};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::TYPE_MONO_SM;
use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, px,
};
use std::rc::Rc;

/// The height of the sign-in terminal inside the dialog.
const TERMINAL_HEIGHT: f32 = 320.0;

/// A press handler of the dialog.
type Listener = Rc<dyn Fn(&mut Window, &mut App)>;

/// Where an account sign-in is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInStage {
    /// The flow is starting; the sign-in page is not ready yet.
    Starting,
    /// The sign-in page at `url` is ready to open.
    WaitingBrowser { url: SharedString },
    /// The sign-in page shows a code that the user pastes here.
    WaitingCode,
    /// The sign-in runs in the service's terminal, shown in the dialog.
    InTerminal,
    /// The terminal has ended; the service is deciding whether to keep the account.
    Checking,
    /// The account was added and is ready to use.
    Done,
    /// The flow failed; `reason` says why.
    Failed { reason: SharedString },
}

/// What a stage's primary button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    /// The sign-in page is still being prepared, so there is nothing to open yet.
    Wait,
    /// The sign-in runs in the terminal, so the dialog waits for it to end.
    Signing,
    /// The service is deciding the account.
    Deciding,
    OpenPage,
    Submit,
    Retry,
    Finish,
}

impl Next {
    /// The words on the primary button.
    fn label(self) -> &'static str {
        match self {
            Self::Wait => "Starting...",
            Self::Signing => "Signing in...",
            Self::Deciding => "Checking...",
            Self::OpenPage => "Open sign-in page",
            Self::Submit => "Continue",
            Self::Retry => "Try again",
            Self::Finish => "Done",
        }
    }
}

/// The primary action of a stage, and whether it can be pressed yet (it cannot while starting).
fn primary(stage: &SignInStage) -> (Next, bool) {
    match stage {
        SignInStage::Starting => (Next::Wait, false),
        SignInStage::WaitingBrowser { .. } => (Next::OpenPage, true),
        SignInStage::WaitingCode => (Next::Submit, true),
        SignInStage::InTerminal => (Next::Signing, false),
        SignInStage::Checking => (Next::Deciding, false),
        SignInStage::Done => (Next::Finish, true),
        SignInStage::Failed { .. } => (Next::Retry, true),
    }
}

/// The body copy of a stage; the failure stage shows its ErrorState instead.
fn body_copy(stage: &SignInStage) -> Option<&'static str> {
    match stage {
        SignInStage::Starting => Some("Starting sign-in..."),
        SignInStage::WaitingBrowser { .. } => {
            Some("Sign in on the page that opens. This window waits until you finish.")
        }
        SignInStage::WaitingCode => Some("Paste the code the sign-in page shows."),
        SignInStage::InTerminal => {
            Some("Finish the sign-in in the terminal below. The account is kept once it signs in.")
        }
        SignInStage::Checking => Some("Checking the sign-in..."),
        SignInStage::Done => Some("Signed in. The account is ready to use."),
        SignInStage::Failed { .. } => None,
    }
}

/// The dismiss button: Close once the account is ready, otherwise Cancel.
fn dismiss_label(stage: &SignInStage) -> &'static str {
    match stage {
        SignInStage::Done => "Close",
        _ => "Cancel",
    }
}

/// The dialog title, naming the agent.
fn title(agent: &str) -> String {
    format!("Add account to {agent}")
}

/// The failure stage: an inline ErrorState titled "Sign-in failed" with the reason and Retry.
fn failed_state(
    id: impl Into<ElementId>,
    reason: SharedString,
    on_retry: Option<Listener>,
) -> ErrorState {
    let id: ElementId = id.into();
    let mut state = ErrorState::new((id, "error"), "Sign-in failed")
        .variant(ErrorStateVariant::Inline)
        .explanation(reason);
    if let Some(listener) = on_retry {
        state = state.on_retry(move |window, cx| listener(window, cx));
    }
    state
}

/// A dialog that adds an account to an agent and follows the sign-in through its stages.
#[derive(IntoElement)]
pub struct AccountSignInDialog {
    id: ElementId,
    agent: SharedString,
    stage: SignInStage,
    code_field: Option<AnyElement>,
    terminal: Option<AnyElement>,
    on_open_page: Option<Listener>,
    on_submit: Option<Listener>,
    on_retry: Option<Listener>,
    on_done: Option<Listener>,
    on_cancel: Option<Listener>,
}

impl std::fmt::Debug for AccountSignInDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountSignInDialog")
            .field("id", &self.id)
            .field("agent", &self.agent)
            .field("stage", &self.stage)
            .finish_non_exhaustive()
    }
}

impl AccountSignInDialog {
    /// The dialog for `agent` at `stage`, titled "Add account to {agent}".
    pub fn new(
        id: impl Into<ElementId>,
        agent: impl Into<SharedString>,
        stage: SignInStage,
    ) -> Self {
        Self {
            id: id.into(),
            agent: agent.into(),
            stage,
            code_field: None,
            terminal: None,
            on_open_page: None,
            on_submit: None,
            on_retry: None,
            on_done: None,
            on_cancel: None,
        }
    }

    /// The caller's text field for the code, shown at the code stage.
    pub fn code_field(mut self, field: impl IntoElement) -> Self {
        self.code_field = Some(field.into_any_element());
        self
    }

    /// The caller's terminal surface where the sign-in runs, shown while it runs and while it is checked.
    pub fn terminal(mut self, surface: impl IntoElement) -> Self {
        self.terminal = Some(surface.into_any_element());
        self
    }

    /// Runs when "Open sign-in page" is pressed.
    pub fn on_open_page(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_open_page = Some(Rc::new(listener));
        self
    }

    /// Runs when "Continue" is pressed with the code.
    pub fn on_submit(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Rc::new(listener));
        self
    }

    /// Runs when "Try again" is pressed, or the ErrorState's Retry.
    pub fn on_retry(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(listener));
        self
    }

    /// Runs when "Done" is pressed; closing a finished sign-in also runs it.
    pub fn on_done(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_done = Some(Rc::new(listener));
        self
    }

    /// Runs when Cancel is pressed, or Escape closes the dialog before the account is ready.
    pub fn on_cancel(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(listener));
        self
    }
}

/// The press of "Open sign-in page": it opens the page at the stage's url, then tells the caller.
fn open_page_press(stage: &SignInStage, then: Option<Listener>) -> Option<Listener> {
    let SignInStage::WaitingBrowser { url } = stage else {
        return then;
    };
    let url = url.clone();
    Some(Rc::new(move |window, cx| {
        cx.open_url(&url);
        if let Some(then) = &then {
            then(window, cx);
        }
    }))
}

impl RenderOnce for AccountSignInDialog {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let (next, enabled) = primary(&self.stage);
        let press = match (enabled, next) {
            (false, _) | (true, Next::Wait | Next::Signing | Next::Deciding) => None,
            (true, Next::OpenPage) => open_page_press(&self.stage, self.on_open_page.clone()),
            (true, Next::Submit) => self.on_submit.clone(),
            (true, Next::Retry) => self.on_retry.clone(),
            (true, Next::Finish) => self.on_done.clone(),
        };
        let dismiss = if matches!(self.stage, SignInStage::Done) {
            self.on_done.clone()
        } else {
            self.on_cancel.clone()
        };
        let mut dialog = Dialog::new(self.id.clone(), title(&self.agent))
            .open(true)
            .action(next.label())
            .action_disabled(!enabled)
            .cancel_label(dismiss_label(&self.stage))
            .on_action(move |window, cx| {
                if let Some(press) = &press {
                    press(window, cx);
                }
            })
            .on_dismiss(move |window, cx| {
                if let Some(dismiss) = &dismiss {
                    dismiss(window, cx);
                }
            });
        if let Some(copy) = body_copy(&self.stage) {
            dialog = dialog.child(copy);
        }
        if let SignInStage::WaitingBrowser { url } = &self.stage {
            dialog = dialog.child(
                div()
                    .text_style(TYPE_MONO_SM)
                    .text_color(theme.palette.text.secondary)
                    .child(url.clone()),
            );
        }
        if let (SignInStage::WaitingCode, Some(field)) = (&self.stage, self.code_field) {
            dialog = dialog.child(field);
        }
        if let (SignInStage::InTerminal | SignInStage::Checking, Some(surface)) =
            (&self.stage, self.terminal)
        {
            dialog = dialog.child(div().w_full().h(px(TERMINAL_HEIGHT)).child(surface));
        }
        if let SignInStage::Failed { reason } = &self.stage {
            dialog = dialog.child(failed_state(
                self.id.clone(),
                reason.clone(),
                self.on_retry.clone(),
            ));
        }
        dialog
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    fn browser() -> SignInStage {
        SignInStage::WaitingBrowser {
            url: "https://example.com/sign-in".into(),
        }
    }

    #[test]
    fn marketplace_account_sign_in_dialog_stage_maps_to_its_primary_action() {
        let failed = SignInStage::Failed {
            reason: "The code expired. Try again.".into(),
        };
        assert_eq!(primary(&SignInStage::Starting), (Next::Wait, false));
        assert_eq!(primary(&browser()), (Next::OpenPage, true));
        assert_eq!(primary(&SignInStage::WaitingCode), (Next::Submit, true));
        assert_eq!(primary(&SignInStage::Done), (Next::Finish, true));
        assert_eq!(primary(&failed), (Next::Retry, true));
        assert_eq!(primary(&SignInStage::InTerminal), (Next::Signing, false));
        assert_eq!(primary(&SignInStage::Checking), (Next::Deciding, false));
        assert_eq!(Next::Signing.label(), "Signing in...");
        assert_eq!(Next::Deciding.label(), "Checking...");
        assert_eq!(Next::Wait.label(), "Starting...");
        assert_eq!(Next::OpenPage.label(), "Open sign-in page");
        assert_eq!(Next::Submit.label(), "Continue");
        assert_eq!(Next::Retry.label(), "Try again");
        assert_eq!(Next::Finish.label(), "Done");
        assert_eq!(dismiss_label(&SignInStage::Done), "Close");
        assert_eq!(dismiss_label(&SignInStage::WaitingCode), "Cancel");
    }

    #[test]
    fn marketplace_account_sign_in_dialog_copy_matches_each_stage() {
        assert_eq!(
            body_copy(&SignInStage::Starting),
            Some("Starting sign-in...")
        );
        assert_eq!(
            body_copy(&browser()),
            Some("Sign in on the page that opens. This window waits until you finish.")
        );
        assert_eq!(
            body_copy(&SignInStage::WaitingCode),
            Some("Paste the code the sign-in page shows.")
        );
        assert_eq!(
            body_copy(&SignInStage::Done),
            Some("Signed in. The account is ready to use.")
        );
        let failed = SignInStage::Failed {
            reason: "The code expired.".into(),
        };
        assert_eq!(body_copy(&failed), None);
        assert_eq!(
            body_copy(&SignInStage::InTerminal),
            Some("Finish the sign-in in the terminal below. The account is kept once it signs in.")
        );
        assert_eq!(
            body_copy(&SignInStage::Checking),
            Some("Checking the sign-in...")
        );
        assert_eq!(title("Claude"), "Add account to Claude");
    }

    #[gpui::test]
    fn marketplace_account_sign_in_dialog_failure_is_an_alert_titled_sign_in_failed(
        cx: &mut TestAppContext,
    ) {
        let node = root(cx, |window, cx| {
            reported(
                failed_state("sign-in", "The code expired. Try again.".into(), None)
                    .render(window, cx),
            )
        })
        .expect("the failure is reported");
        assert_eq!(node.role(), Role::Alert);
        assert_eq!(node.label(), Some("Sign-in failed"));
    }
}
