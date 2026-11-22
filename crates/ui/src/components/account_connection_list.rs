//! Connection list: an agent's accounts with state, identity, use and actions.

use crate::components::badge::{Badge, BadgeTone};
use crate::components::button::Button;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Palette, RADIUS_LG, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_BODY_STRONG,
};
use gpui::{
    App, Div, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};

/// The accessible name of the list of accounts.
const LIST_NAME: &str = "Accounts";
/// Fixed copy under the list: where the sign-in token lives.
const TOKEN_NOTE: &str = "Sign-in tokens stay in the secret vault on this computer. They never enter a chat, a prompt or a log.";

/// The state of an account, with the words the Marketplace shows for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountStatus {
    SignedIn,
    Expired,
    NeedsSignIn,
    NotSetUp,
    Migrated,
    Conflict,
    Unknown,
}

impl AccountStatus {
    /// The word for the state, such as "Needs sign-in".
    pub fn word(self) -> &'static str {
        match self {
            Self::SignedIn => "Signed in",
            Self::Expired => "Expired",
            Self::NeedsSignIn => "Needs sign-in",
            Self::NotSetUp => "Not set up",
            Self::Migrated => "Migrated",
            Self::Conflict => "Conflict",
            Self::Unknown => "Can't check",
        }
    }

    /// The badge tone: danger for a conflict, warning when the account needs a sign-in.
    fn tone(self) -> BadgeTone {
        match self {
            Self::SignedIn => BadgeTone::Success,
            Self::Expired | Self::NeedsSignIn => BadgeTone::Warning,
            Self::Migrated => BadgeTone::Info,
            Self::Conflict => BadgeTone::Danger,
            Self::NotSetUp | Self::Unknown => BadgeTone::Neutral,
        }
    }
}

/// What a row says after its label, for screen readers: state, Active, identity and use.
fn row_description(
    status: AccountStatus,
    active: bool,
    identity: Option<&str>,
    used_by: Option<&str>,
) -> String {
    let mut parts = vec![status.word()];
    if active {
        parts.push("Active");
    }
    parts.extend(identity);
    parts.extend(used_by);
    parts.join(", ")
}

/// A failure line under a row: an octagon glyph and the message, announced as an alert.
fn error_line(id: impl Into<ElementId>, text: SharedString, p: &Palette) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Alert)
        .aria_label(text.clone())
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .min_w_0()
        .text_style(TYPE_BODY_SM)
        .text_color(p.status.danger_fg)
        .child(
            Icon::new(IconName::OctagonAlert)
                .size(IconSize::XSmall)
                .color(p.status.danger_fg),
        )
        .child(text)
}

/// The bordered list of rows, named "Accounts", or the empty text when it has no rows.
fn account_list(
    id: ElementId,
    rows: Vec<AccountRow>,
    empty: Option<SharedString>,
    p: &Palette,
) -> Stateful<Div> {
    let count = rows.len();
    let empty_text = if rows.is_empty() { empty } else { None };
    let mut list = div()
        .id((id, "accounts"))
        .role(Role::List)
        .aria_label(LIST_NAME)
        .flex()
        .flex_col()
        .bg(p.surface.raised)
        .border_1()
        .border_color(p.border.default)
        .rounded(px(RADIUS_LG));
    if let Some(text) = empty_text {
        list = list.child(
            div()
                .px(px(SPACE_4))
                .py(px(SPACE_3))
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child(text),
        );
    }
    for (index, row) in rows.into_iter().enumerate() {
        let mut cell = div().w_full().child(row);
        if index + 1 < count {
            cell = cell.border_b_1().border_color(p.border.subtle);
        }
        list = list.child(cell);
    }
    list
}

/// One account: its label and state, where it is signed in, what uses it, and its actions.
#[derive(IntoElement)]
pub struct AccountRow {
    id: ElementId,
    label: SharedString,
    status: AccountStatus,
    identity: Option<SharedString>,
    active: bool,
    used_by: Option<SharedString>,
    error: Option<SharedString>,
    make_active: Option<Button>,
    check: Option<Button>,
    remove: Option<Button>,
}

impl std::fmt::Debug for AccountRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountRow")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("status", &self.status)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl AccountRow {
    /// `label` names the account, such as "Work account".
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        status: AccountStatus,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status,
            identity: None,
            active: false,
            used_by: None,
            error: None,
            make_active: None,
            check: None,
            remove: None,
        }
    }

    /// Where the account is signed in, such as "user@example.com".
    pub fn identity(mut self, text: impl Into<SharedString>) -> Self {
        self.identity = Some(text.into());
        self
    }

    /// Active shows an "Active" badge; otherwise the Make active button shows.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// What uses the account, such as "Used by Project one" or "Not used by any project".
    pub fn used_by(mut self, text: impl Into<SharedString>) -> Self {
        self.used_by = Some(text.into());
        self
    }

    /// A line under the row that says why an action failed.
    pub fn error(mut self, text: impl Into<SharedString>) -> Self {
        self.error = Some(text.into());
        self
    }

    /// The Make active button, shown while the account is not active.
    pub fn make_active(mut self, button: Button) -> Self {
        self.make_active = Some(button);
        self
    }

    /// The Check now button.
    pub fn check(mut self, button: Button) -> Self {
        self.check = Some(button);
        self
    }

    /// The Remove button.
    pub fn remove(mut self, button: Button) -> Self {
        self.remove = Some(button);
        self
    }
}

impl RenderOnce for AccountRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let description = SharedString::from(row_description(
            self.status,
            self.active,
            self.identity.as_deref(),
            self.used_by.as_deref(),
        ));
        let mut details = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_BODY_STRONG)
                    .text_color(p.text.primary)
                    .child(self.label.clone()),
            );
        if let Some(identity) = self.identity {
            details = details.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child(identity),
            );
        }
        if let Some(used_by) = self.used_by {
            details = details.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(used_by),
            );
        }
        let head = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(SPACE_2))
            .min_w_0()
            .child(details)
            .child(Badge::text(self.status.word()).tone(self.status.tone()));

        let mut actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(SPACE_2))
            .min_w_0();
        if self.active {
            actions = actions.child(Badge::text("Active").tone(BadgeTone::Accent));
        } else {
            actions = actions.children(self.make_active);
        }
        actions = actions.children(self.check).children(self.remove);

        let mut row = div()
            .id(self.id.clone())
            .role(Role::ListItem)
            .aria_label(self.label.clone())
            .aria_description(description)
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .min_w_0()
            .px(px(SPACE_4))
            .py(px(SPACE_3))
            .child(head)
            .child(actions);
        if let Some(error) = self.error {
            row = row.child(error_line((self.id.clone(), "error"), error, p));
        }
        row
    }
}

/// The accounts of one agent: their rows, the text for none, Add account and the token note.
#[derive(IntoElement)]
pub struct AccountConnectionList {
    id: ElementId,
    accounts: Vec<AccountRow>,
    empty: Option<SharedString>,
    add_account: Option<Button>,
}

impl std::fmt::Debug for AccountConnectionList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountConnectionList")
            .field("id", &self.id)
            .field("accounts", &self.accounts.len())
            .finish_non_exhaustive()
    }
}

impl AccountConnectionList {
    /// `id` names the list and its parts.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            accounts: Vec::new(),
            empty: None,
            add_account: None,
        }
    }

    /// One account row, in order.
    pub fn account(mut self, row: AccountRow) -> Self {
        self.accounts.push(row);
        self
    }

    /// The text shown when there are no accounts, such as "No accounts yet.".
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = Some(text.into());
        self
    }

    /// The Add account button, under the list.
    pub fn add_account(mut self, button: Button) -> Self {
        self.add_account = Some(button);
        self
    }
}

impl RenderOnce for AccountConnectionList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let mut footer = div().flex().flex_col().gap(px(SPACE_2)).min_w_0();
        if let Some(button) = self.add_account {
            footer = footer.child(div().flex().child(button));
        }
        footer = footer.child(
            div()
                .id((self.id.clone(), "token-note"))
                .role(Role::Group)
                .aria_label(TOKEN_NOTE)
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child(TOKEN_NOTE),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_3))
            .min_w_0()
            .child(account_list(self.id, self.accounts, self.empty, p))
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[test]
    fn marketplace_account_connection_list_status_words_match_the_model() {
        assert_eq!(AccountStatus::SignedIn.word(), "Signed in");
        assert_eq!(AccountStatus::Expired.word(), "Expired");
        assert_eq!(AccountStatus::NeedsSignIn.word(), "Needs sign-in");
        assert_eq!(AccountStatus::NotSetUp.word(), "Not set up");
        assert_eq!(AccountStatus::Migrated.word(), "Migrated");
        assert_eq!(AccountStatus::Conflict.word(), "Conflict");
        assert_eq!(AccountStatus::Unknown.word(), "Can't check");
    }

    #[test]
    fn marketplace_account_connection_list_row_description_names_state_active_identity_and_use() {
        assert_eq!(
            row_description(
                AccountStatus::SignedIn,
                true,
                Some("user@example.com"),
                Some("Used by Project one"),
            ),
            "Signed in, Active, user@example.com, Used by Project one"
        );
        assert_eq!(
            row_description(AccountStatus::Expired, false, None, None),
            "Expired"
        );
    }

    #[gpui::test]
    fn marketplace_account_connection_list_row_is_a_list_item_named_by_its_label(
        cx: &mut TestAppContext,
    ) {
        let node = root(cx, |window, cx| {
            reported(
                AccountRow::new("row", "Work account", AccountStatus::SignedIn)
                    .identity("user@example.com")
                    .active(true)
                    .used_by("Used by Project one")
                    .render(window, cx),
            )
        })
        .expect("the row is reported");
        assert_eq!(node.role(), Role::ListItem);
        assert_eq!(node.label(), Some("Work account"));
        assert_eq!(
            node.description(),
            Some("Signed in, Active, user@example.com, Used by Project one")
        );
    }

    #[gpui::test]
    fn marketplace_account_connection_list_is_a_list_named_accounts(cx: &mut TestAppContext) {
        let node = root(cx, |_, cx| {
            let p = cx.theme().palette;
            reported(account_list(
                ElementId::from("connection"),
                Vec::new(),
                Some("No accounts yet.".into()),
                &p,
            ))
        })
        .expect("the list is reported");
        assert_eq!(node.role(), Role::List);
        assert_eq!(node.label(), Some("Accounts"));
    }

    #[gpui::test]
    fn marketplace_account_connection_list_error_line_is_an_alert(cx: &mut TestAppContext) {
        let node = root(cx, |_, cx| {
            let p = cx.theme().palette;
            reported(error_line(
                "error",
                "A live session uses this account. Stop the session, then remove it.".into(),
                &p,
            ))
        })
        .expect("the error is reported");
        assert_eq!(node.role(), Role::Alert);
        assert_eq!(
            node.label(),
            Some("A live session uses this account. Stop the session, then remove it.")
        );
    }
}
