//! A routine's run history page (AUTO-28): each run with its time, whether its task started, and why not when it did not.

use super::super::Shell;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{
    RoutineId, RunRow, RunsState, TeamId, TeamPage, run_rows, schedule_words,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, WeakEntity, div, prelude::*, px};
use jiff::tz::TimeZone;
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::tokens::{
    SPACE_1, SPACE_2, SPACE_3, SPACE_6, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

const ROUTINE_GONE_HISTORY: &str = "That routine is gone.";

/// The run history of one routine (AUTO-28).
pub(in crate::shell) fn routine_history_page(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    team: TeamId,
    routine: RoutineId,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let Some(team_data) = model.teams().team(team) else {
        return note("This team is gone.", &theme).into_any_element();
    };
    let Some(item) = team_data.routines.iter().find(|item| item.id == routine) else {
        return note(ROUTINE_GONE_HISTORY, &theme).into_any_element();
    };
    let back = {
        let shell = shell.clone();
        IconButton::new("routine-history-back", IconName::ArrowLeft, "Back").on_click(
            move |_, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.model.open_team_page(TeamPage::Routines(team));
                    cx.notify();
                });
            },
        )
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
                .child("Run history"),
        );
    let caption = format!(
        "{} · {}",
        item.name,
        schedule_words(item.cadence, &item.time, item.run_date.clone())
    );
    let runs = match model.teams().routine_history().map(|open| open.runs.clone()) {
        Some(RunsState::Loaded(rows)) if rows.is_empty() => EmptyState::new("history-empty", "No runs yet")
            .icon(IconName::Clock)
            .explanation(
                "Each run of this routine is kept here, with the task it started or why it did not start.",
            )
            .into_any_element(),
        Some(RunsState::Loaded(rows)) => runs_list(&rows, &theme).into_any_element(),
        Some(RunsState::Failed(message)) => note(&message, &theme).into_any_element(),
        Some(RunsState::Loading) | None => note("Loading the runs.", &theme).into_any_element(),
    };
    div()
        .id("routine-history-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(header)
        .child(note(&caption, &theme))
        .child(runs)
        .into_any_element()
}

/// The runs, newest first, one row each: when, what happened, and the reason when nothing started.
fn runs_list(rows: &[RunRow], theme: &massorca_ui::Theme) -> impl IntoElement {
    let mut list = div().flex().flex_col().gap(px(SPACE_3));
    for row in rows {
        let mut line = div()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(
                div()
                    .text_style(TYPE_BODY_STRONG)
                    .text_color(theme.palette.text.primary)
                    .child(row.when.clone()),
            )
            .child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(theme.palette.text.secondary)
                    .child(row.status),
            );
        if let Some(detail) = &row.detail {
            line = line.child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(theme.palette.text.tertiary)
                    .child(detail.clone()),
            );
        }
        list = list.child(line);
    }
    list
}

fn note(text: &str, theme: &massorca_ui::Theme) -> impl IntoElement {
    div()
        .pt(px(SPACE_2))
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}

impl Shell {
    /// Opens a routine's run history. The service's runs reach the model when it answers; the sample store has none.
    pub(super) fn open_routine_history(
        &mut self,
        team: TeamId,
        routine: RoutineId,
        cx: &mut gpui::Context<Self>,
    ) {
        self.model.teams_mut().open_routine_history(team, routine);
        self.model
            .open_team_page(TeamPage::RoutineHistory(team, routine));
        let Some(service) = self.service.clone() else {
            self.model.teams_mut().set_routine_history(
                team,
                routine,
                RunsState::Loaded(Vec::new()),
            );
            cx.notify();
            return;
        };
        let (Some(team_key), Some(routine_key)) =
            (self.team_key(team, cx), self.routine_key(routine, cx))
        else {
            self.model.teams_mut().set_routine_history(
                team,
                routine,
                RunsState::Failed(ROUTINE_GONE_HISTORY.to_owned()),
            );
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let answer = service.routine_runs(team_key, routine_key).await;
            this.update(cx, |shell, cx| {
                let runs = match answer {
                    Ok(runs) => RunsState::Loaded(run_rows(&runs, &TimeZone::system())),
                    Err(refusal) => RunsState::Failed(refusal.message()),
                };
                shell
                    .model
                    .teams_mut()
                    .set_routine_history(team, routine, runs);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}
