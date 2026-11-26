//! The Calendar page (decision 0124): the week's routine and automation runs, with the mode chips, the Show done switch
//! and a column per day. Each day lists its runs in time order; a click opens the routine or the automation. The
//! controls and the reads are in `actions`.

mod actions;

use gpui::{AnyElement, IntoElement, ParentElement, Styled, WeakEntity, div, prelude::*, px};
use massorca_protocol::methods::calendar::{CalendarKind, CalendarOccurrence};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{
    SPACE_2, SPACE_3, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_HEADING, TYPE_LABEL, TYPE_TITLE,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

use super::Shell;
use crate::model::calendar::{
    CalendarLoad, CalendarMode, CalendarState, day_heading, range_title, week_days,
};

/// What the Calendar page draws: its state, whether a service backs it, and the shell its controls act on.
#[derive(Clone)]
pub(in crate::shell) struct CalendarFrame {
    pub state: CalendarState,
    pub connected: bool,
    pub shell: WeakEntity<Shell>,
}

/// The Calendar page, filling the Canvas.
pub(in crate::shell) fn page(frame: &CalendarFrame, cx: &gpui::App) -> AnyElement {
    let theme = *cx.theme();
    let shell = &frame.shell;
    let state = &frame.state;
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("Calendar"),
        )
        .child(controls(frame, &theme))
        .child(mode_chips(state.mode(), shell, &theme));
    if !frame.connected {
        body = body.child(note(
            "Calendar runs come from the service. Connect to MassOrca to see them.",
            &theme,
        ));
    }
    match state.load() {
        CalendarLoad::Idle | CalendarLoad::Loading(_) => {
            body = body.child(note("Loading the calendar.", &theme));
        }
        CalendarLoad::Unsupported => {
            body = body.child(note(
                "This service does not offer the calendar yet. Update it to see routines and automations here.",
                &theme,
            ));
        }
        CalendarLoad::Failed(reason) => {
            body = body
                .child(note(
                    &format!("The calendar could not be read: {reason}"),
                    &theme,
                ))
                .child(retry_button(shell));
        }
        CalendarLoad::Loaded(list) => {
            let days = state.days();
            if days.iter().all(Vec::is_empty) {
                body = body.child(
                    EmptyState::new("calendar-empty", "Nothing scheduled this week")
                        .icon(IconName::CalendarClock)
                        .explanation(
                            "Routines and automations show here when they have a run time.",
                        ),
                );
            } else {
                let mut week = div().flex().gap(px(SPACE_3)).items_start();
                for (day, runs) in week_days(state.week()).into_iter().zip(days.iter()) {
                    week = week.child(day_column(day, runs, shell, &theme));
                }
                body = body.child(div().id("calendar-week").overflow_x_scroll().child(week));
            }
            if list.truncated {
                body = body.child(note(
                    "More runs than one read returns. Narrow the mode or show fewer days.",
                    &theme,
                ));
            }
        }
    }
    div()
        .id("calendar-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(body)
        .into_any_element()
}

/// Previous, Today and Next, the range title, and the Show done switch.
fn controls(frame: &CalendarFrame, theme: &Theme) -> impl IntoElement {
    let shell = &frame.shell;
    let show_done = {
        let shell = shell.clone();
        Switch::new("calendar-show-done", frame.state.show_done()).on_change(move |on, _, cx| {
            let _ = shell.update(cx, |shell, cx| shell.set_calendar_show_done(on, cx));
        })
    };
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(SPACE_4))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(navigation_button(
                    "calendar-previous",
                    "Previous",
                    shell,
                    |shell, cx| shell.calendar_previous_week(cx),
                ))
                .child(navigation_button(
                    "calendar-today",
                    "Today",
                    shell,
                    |shell, cx| shell.calendar_today(cx),
                ))
                .child(navigation_button(
                    "calendar-next",
                    "Next",
                    shell,
                    |shell, cx| shell.calendar_next_week(cx),
                ))
                .child(
                    div()
                        .text_style(TYPE_TITLE)
                        .text_color(theme.palette.text.primary)
                        .child(range_title(frame.state.week())),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(
                    div()
                        .text_style(TYPE_LABEL)
                        .text_color(theme.palette.text.secondary)
                        .child("Show done"),
                )
                .child(show_done),
        )
}

/// The mode chips: All, Team and Automations. The chip the page shows is the filled one.
fn mode_chips(
    selected: CalendarMode,
    shell: &WeakEntity<Shell>,
    _theme: &Theme,
) -> impl IntoElement {
    let mut chips = div().flex().items_center().gap(px(SPACE_2));
    for mode in CalendarMode::ALL {
        let variant = if mode == selected {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Ghost
        };
        let shell = shell.clone();
        chips = chips.child(
            Button::new(format!("calendar-mode-{}", mode.label()), mode.label())
                .variant(variant)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.set_calendar_mode(mode, cx));
                }),
        );
    }
    chips
}

fn navigation_button(
    id: &'static str,
    label: &'static str,
    shell: &WeakEntity<Shell>,
    act: fn(&mut Shell, &mut gpui::Context<Shell>),
) -> Button {
    let shell = shell.clone();
    Button::new(id, label)
        .variant(ButtonVariant::Ghost)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, act);
        })
}

fn retry_button(shell: &WeakEntity<Shell>) -> impl IntoElement {
    let shell = shell.clone();
    Button::new("calendar-retry", "Retry")
        .leading_icon(IconName::RotateCcw)
        .variant(ButtonVariant::Secondary)
        .on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.retry_calendar(cx));
        })
}

/// One day: its heading, then its runs in time order, or "Nothing scheduled" when it has none.
fn day_column(
    day: jiff::civil::Date,
    runs: &[CalendarOccurrence],
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> impl IntoElement {
    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .flex_1()
        .min_w(px(96.0))
        .child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(theme.palette.text.secondary)
                .child(day_heading(day)),
        );
    if runs.is_empty() {
        column = column.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child("Nothing scheduled"),
        );
    }
    for run in runs {
        column = column.child(run_entry(run, shell));
    }
    column
}

/// One run: its time, its kind, and its name, opening the routine or the automation it belongs to.
fn run_entry(run: &CalendarOccurrence, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let shell = shell.clone();
    let open = run.clone();
    let done = if run.done { " · Done" } else { "" };
    let label = format!(
        "{} {} {}{done}",
        run.local_time,
        kind_word(run.kind),
        run.name
    );
    Button::new(
        format!("calendar-run-{}-{}", run.source_id, run.at_ms),
        label,
    )
    .variant(ButtonVariant::Ghost)
    .on_click(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| shell.open_calendar_run(&open, cx));
    })
}

fn kind_word(kind: CalendarKind) -> &'static str {
    match kind {
        CalendarKind::Routine => "Routine",
        CalendarKind::Automation => "Automation",
        CalendarKind::Unknown => "Run",
    }
}

fn note(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(text.to_owned())
}
