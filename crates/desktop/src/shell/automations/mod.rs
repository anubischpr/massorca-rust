//! The Automations page (decision 0123): the scheduled automations the service holds. Each row shows its schedule, its
//! project and its next run, with a switch to enable it and Delete behind a confirm.

mod actions;

use super::Shell;
use crate::model::automations::{AutomationsState, next_run_text, schedule_summary};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, WeakEntity, div, prelude::*, px};
use massorca_protocol::methods::automations::Automation;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{
    SPACE_2, SPACE_3, SPACE_4, SPACE_6, TYPE_BODY, TYPE_BODY_SM, TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// What the Automations page draws: its state, whether a service backs it, the clock its next runs are read against,
/// and the shell the page's controls act on.
#[derive(Clone)]
pub(in crate::shell) struct AutomationsFrame {
    pub state: AutomationsState,
    pub connected: bool,
    pub now_ms: i64,
    pub shell: WeakEntity<Shell>,
}

/// The Automations page, filling the Canvas.
pub(in crate::shell) fn page(frame: &AutomationsFrame, cx: &gpui::App) -> AnyElement {
    let theme = *cx.theme();
    let shell = &frame.shell;
    let header = div().flex().items_center().justify_between().child(
        div()
            .text_style(TYPE_HEADING)
            .text_color(theme.palette.text.primary)
            .child("Automations"),
    );
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(header)
        .child(note(
            "An automation starts an agent on a schedule, in a project.",
            &theme,
        ));
    if !frame.connected {
        body = body.child(note(
            "Automations are kept by the service. Connect to MassOrca to see or change them.",
            &theme,
        ));
    } else if let Some(error) = frame.state.list_error() {
        body = body.child(note(
            &format!("The automations could not be read: {error}"),
            &theme,
        ));
    }
    if let Some(notice) = frame.state.notice() {
        body = body.child(note(notice, &theme));
    }
    let automations = frame.state.automations();
    if automations.is_empty() {
        if frame.connected && frame.state.is_read() && frame.state.list_error().is_none() {
            body = body.child(
                EmptyState::new("automations-empty", "No automations yet")
                    .icon(IconName::CalendarClock)
                    .explanation("No automation is defined in this profile.")
                    .into_any_element(),
            );
        }
    } else {
        let mut list = div().flex().flex_col().gap(px(SPACE_2));
        for automation in automations {
            list = list.child(automation_row(automation, frame.now_ms, shell, &theme));
        }
        body = body.child(list);
    }
    div()
        .id("automations-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(body)
        .into_any_element()
}

/// The confirm before an automation is deleted. Deleting is permanent, so the action is danger-styled.
pub(in crate::shell) fn delete_dialog(
    name: &str,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> Dialog {
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    Dialog::new("automation-delete", "Delete automation")
        .variant(DialogVariant::Destructive)
        .open(true)
        .child(note(
            &format!("Delete {name}? It is removed from the list and cannot be restored."),
            theme,
        ))
        .action("Delete")
        .action_icon(IconName::Trash2)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.confirm_automation_delete(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.cancel_automation_delete(cx));
        })
}

/// One automation: its name, schedule, project and next run, then its switch and its Delete control.
fn automation_row(
    automation: &Automation,
    now_ms: i64,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> impl IntoElement {
    let id = automation.id.clone();
    let project = automation
        .project_name
        .as_deref()
        .map_or_else(|| "No project".to_owned(), |name| format!("In {name}"));
    let switch = {
        let shell = shell.clone();
        let id = id.clone();
        Switch::new(format!("automation-switch-{id}"), automation.enabled).on_change(
            move |on, _, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.set_automation_enabled(id.clone(), on, cx)
                });
            },
        )
    };
    let delete = {
        let shell = shell.clone();
        let id = id.clone();
        Button::new(format!("automation-delete-{id}"), "Delete")
            .leading_icon(IconName::Trash2)
            .variant(ButtonVariant::DangerGhost)
            .on_click(move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.ask_automation_delete(id.clone(), cx));
            })
    };
    let text = div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_style(TYPE_BODY)
                .text_color(theme.palette.text.primary)
                .child(automation.name.clone()),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(schedule_summary(automation)),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(format!("{project} · {}", next_run_text(automation, now_ms))),
        );
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(SPACE_4))
        .p(px(SPACE_3))
        .child(text)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(switch)
                .child(delete),
        )
}

fn note(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}
