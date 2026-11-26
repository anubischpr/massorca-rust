//! The strip above a session's composer while a child writes in the session's own worktree (decision 0170): the
//! child's agent, how long it has run, and Open child, which opens the child.

use gpui::prelude::*;
use gpui::{AnyElement, WeakEntity, div, px};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, SPACE_2, SPACE_4, TYPE_BODY_SM, TYPE_CAPTION,
};
use massorca_ui::{TextStyled, Theme};

use super::Shell;

/// What the strip says about the child the session waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildWaitStrip {
    /// The child's session id, which Open child opens.
    pub child: String,
    /// The child's agent, named by its kind.
    pub agent: String,
    /// How long the child has run, in words.
    pub elapsed: String,
}

/// The strip: "Waiting for <agent> to finish in this worktree", the elapsed time and Open child.
pub fn child_wait_strip(
    wait: &ChildWaitStrip,
    shell: WeakEntity<Shell>,
    theme: &Theme,
) -> AnyElement {
    let p = theme.palette;
    let child = wait.child.clone();
    div()
        .debug_selector(|| "session-child-wait".to_owned())
        .flex_none()
        .px(px(SPACE_4))
        .pb(px(SPACE_2))
        .child(
            div()
                .w_full()
                .max_w(px(LAYOUT_CONVERSATION_READING))
                .mx_auto()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.secondary)
                .child(format!(
                    "Waiting for {} to finish in this worktree",
                    wait.agent
                ))
                .child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .text_color(p.text.tertiary)
                        .child(wait.elapsed.clone()),
                )
                .child(div().flex_1())
                .child(
                    Button::new("session-open-child", "Open child")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Compact)
                        .on_click(move |_, cx| {
                            let _ = shell.update(cx, |shell, cx| shell.open_session(&child, cx));
                        }),
                ),
        )
        .into_any_element()
}
