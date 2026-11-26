//! The page that draws the New and Edit routine form (screens 4.7): the fields, the schedule choice, the preview line
//! and the footer with Cancel, the save button and, for an edit, Delete routine.

use super::super::Shell;
use super::routine_form::RoutineForm;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{ScheduleKind, WEEKDAYS, Weekday};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, WeakEntity, div, prelude::*, px};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::tokens::{
    SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// The New or Edit routine page on screen.
pub(in crate::shell) fn routine_form_page(
    form: &RoutineForm,
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let Some(team) = model.teams().team(form.team) else {
        return note("This team is gone.", &theme).into_any_element();
    };
    let title = if form.routine.is_some() {
        "Edit routine"
    } else {
        "New routine"
    };
    let back = {
        let shell = shell.clone();
        IconButton::new("routine-form-back", IconName::ArrowLeft, "Back").on_click(move |_, cx| {
            let _ = shell.update(cx, |shell, cx| shell.close_routine_form(cx));
        })
    };
    let header = div()
        .flex()
        .items_center()
        .gap(px(SPACE_3))
        .child(back)
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child(title),
        );
    let mut fields = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(form.name.clone())
        .child(form.prompt.clone())
        .child(form.bot.clone())
        .child(when_group(form, shell))
        .child(form.time.clone())
        .child(note(
            &format!("Runs on {}, the host {} uses.", team.host, team.name),
            &theme,
        ))
        .child(
            div()
                .debug_selector(|| "routine-preview".to_owned())
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(form.draft.preview(&team.host)),
        );
    if let Some(error) = &form.error {
        fields = fields.child(
            div()
                .debug_selector(|| "routine-form-error".to_owned())
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.status.danger_fg)
                .child(error.clone()),
        );
    }
    div()
        .id("routine-form-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(header)
        .child(note(
            &format!(
                "{} · a simple form. Routines are plain-language automations.",
                team.name
            ),
            &theme,
        ))
        .child(fields)
        .child(footer(form, shell))
        .into_any_element()
}

/// The schedule choice and the field that goes with it: the day chips, the day of the month, or the date.
fn when_group(form: &RoutineForm, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let segments: Vec<Segment> = ScheduleKind::ALL
        .iter()
        .map(|kind| Segment::text(kind.label()))
        .collect();
    let selected = ScheduleKind::ALL
        .iter()
        .position(|kind| *kind == form.draft.kind)
        .unwrap_or(0);
    let on_change = shell.clone();
    let kind = SegmentedControl::new("routine-kind", "When", segments)
        .selected(selected)
        .on_change(move |index, _, cx| {
            let _ = on_change.update(cx, |shell, cx| shell.routine_kind_chosen(index, cx));
        });
    let detail = match form.draft.kind {
        ScheduleKind::Once => form.run_date.clone().into_any_element(),
        ScheduleKind::Daily => div().into_any_element(),
        ScheduleKind::Weekly => day_chips(form, shell).into_any_element(),
        ScheduleKind::Monthly => form.month_day.clone().into_any_element(),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(kind)
        .child(detail)
}

/// The seven day chips of a weekly routine, Monday first.
fn day_chips(form: &RoutineForm, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let mut row = div().flex().items_center().gap(px(SPACE_1));
    for (index, day) in WEEKDAYS.into_iter().enumerate() {
        let shell = shell.clone();
        row = row.child(
            Button::new(format!("routine-day-{index}"), day_letter(day))
                .variant(ButtonVariant::Secondary)
                .selected(day == form.draft.weekday)
                .tooltip(day.name())
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.routine_weekday_chosen(day, cx));
                }),
        );
    }
    row
}

fn day_letter(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "M",
        Weekday::Tuesday | Weekday::Thursday => "T",
        Weekday::Wednesday => "W",
        Weekday::Friday => "F",
        Weekday::Saturday | Weekday::Sunday => "S",
    }
}

/// Cancel and the save button; Delete routine, in the danger ghost style, when the page edits a routine.
fn footer(form: &RoutineForm, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let save_label = if form.routine.is_some() {
        "Save routine"
    } else {
        "Create routine"
    };
    let cancel = {
        let shell = shell.clone();
        Button::new("routine-form-cancel", "Cancel")
            .variant(ButtonVariant::Ghost)
            .on_click(move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.close_routine_form(cx));
            })
    };
    let save = {
        let shell = shell.clone();
        Button::new("routine-form-save", save_label)
            .variant(ButtonVariant::Primary)
            .disabled(form.busy)
            .on_click(move |_, cx| {
                let _ = shell.update(cx, |shell, cx| shell.save_routine_form(cx));
            })
    };
    let actions = div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .child(cancel)
        .child(save);
    let row = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(SPACE_2));
    match form.routine {
        Some(routine) => {
            let shell = shell.clone();
            let team = form.team;
            let delete = Button::new("routine-form-delete", "Delete routine")
                .variant(ButtonVariant::DangerGhost)
                .leading_icon(IconName::Trash2)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| {
                        shell.open_routine_confirm(team, routine, cx);
                    });
                });
            row.child(delete).child(actions)
        }
        None => row.child(div()).child(actions),
    }
}

fn note(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}
