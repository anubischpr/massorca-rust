//! The run table of one automation (decision 0125, artboard `ws-automations.dc.html`): its runs newest first, with when
//! each started, its status, how long it ran, and its result or why it did not run.

use gpui::{AnyElement, IntoElement, ParentElement, Styled, WeakEntity, div, prelude::*, px};
use massorca_protocol::methods::automations::AutomationRun;
use massorca_ui::components::badge::{Badge, BadgeTone};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::tokens::{SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_LABEL, TYPE_TITLE_SM};
use massorca_ui::{IconName, TextStyled, Theme};

use super::super::Shell;
use crate::model::automations::{
    RunHistory, run_detail_text, run_duration_text, run_started_text, run_status_text,
};

/// Widths of the Started, Status and Duration columns; the result takes the rest.
const STARTED_WIDTH: f32 = 120.0;
const STATUS_WIDTH: f32 = 112.0;
const DURATION_WIDTH: f32 = 88.0;

/// The table under the list, titled with the automation's name and its run count.
pub(super) fn runs_table(
    history: &RunHistory,
    name: &str,
    now_ms: i64,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> AnyElement {
    let close = {
        let shell = shell.clone();
        Button::new("automation-runs-close", "Close")
            .variant(ButtonVariant::Ghost)
            .on_click(move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.close_automation_runs(cx));
            })
    };
    let count = u32::try_from(history.runs().len()).unwrap_or(u32::MAX);
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(
                    div()
                        .text_style(TYPE_TITLE_SM)
                        .text_color(theme.palette.text.primary)
                        .child(format!("Runs of {name}")),
                )
                .child(Badge::count(count)),
        )
        .child(close);
    let mut table = div()
        .id("automation-runs")
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(header);
    if let Some(error) = history.error() {
        return table
            .child(note(&format!("The runs could not be read: {error}"), theme))
            .into_any_element();
    }
    if !history.is_read() {
        return table
            .child(note("Reading the runs…", theme))
            .into_any_element();
    }
    if history.runs().is_empty() {
        return table
            .child(note("No runs yet. Run now starts one.", theme))
            .into_any_element();
    }
    table = table.child(columns(theme));
    for run in history.runs() {
        table = table.child(run_row(run, now_ms, theme));
    }
    table.into_any_element()
}

fn columns(theme: &Theme) -> impl IntoElement {
    let label = |text: &'static str| {
        div()
            .text_style(TYPE_LABEL)
            .text_color(theme.palette.text.tertiary)
            .child(text)
    };
    div()
        .flex()
        .gap(px(SPACE_3))
        .px(px(SPACE_3))
        .child(label("Started").w(px(STARTED_WIDTH)))
        .child(label("Status").w(px(STATUS_WIDTH)))
        .child(label("Duration").w(px(DURATION_WIDTH)))
        .child(label("Result").flex_1())
}

fn run_row(run: &AutomationRun, now_ms: i64, theme: &Theme) -> impl IntoElement {
    let status = run_status_text(run);
    let tone = match status {
        "Done" => BadgeTone::Success,
        "Failed" => BadgeTone::Danger,
        "Missed" | "Not started" => BadgeTone::Warning,
        "Starting" | "Running" => BadgeTone::Info,
        _ => BadgeTone::Neutral,
    };
    let cell = |text: String| {
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.secondary)
            .child(text)
    };
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_3))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .child(cell(run_started_text(run, now_ms)).w(px(STARTED_WIDTH)))
        .child(
            div()
                .w(px(STATUS_WIDTH))
                .child(Badge::text(status).tone(tone)),
        )
        .child(cell(run_duration_text(run).unwrap_or_default()).w(px(DURATION_WIDTH)))
        .child(cell(run_detail_text(run)).flex_1().truncate())
}

fn note(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .pt(px(SPACE_4))
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}

/// "Run now" on a row: it needs the service to start a run.
pub(super) fn run_now_button(id: &str, connected: bool, shell: &WeakEntity<Shell>) -> Button {
    let shell = shell.clone();
    let target = id.to_owned();
    let button = Button::new(format!("automation-run-now-{id}"), "Run now")
        .leading_icon(IconName::Play)
        .variant(ButtonVariant::Ghost)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.run_automation_now(target.clone(), cx));
        });
    if connected {
        button
    } else {
        button
            .disabled(true)
            .tooltip("Connect to MassOrca to run automations.")
    }
}

/// "Runs" on a row: opens or closes that automation's run table.
pub(super) fn runs_button(id: &str, shell: &WeakEntity<Shell>) -> Button {
    let shell = shell.clone();
    let target = id.to_owned();
    Button::new(format!("automation-runs-{id}"), "Runs")
        .variant(ButtonVariant::Ghost)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.toggle_automation_runs(target.clone(), cx)
            });
        })
}
