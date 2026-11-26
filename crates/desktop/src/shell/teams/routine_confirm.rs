//! The confirm that deletes a routine (ruling P5): its runs go, and the tasks those runs started stay on the Board.

use super::super::Shell;
use super::routine_service::ROUTINE_GONE;
use crate::model::canvas::Canvas;
use crate::model::teams::{RoutineId, TeamId, TeamPage};
use gpui::{App, Context, ParentElement, Styled, WeakEntity, div};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::TYPE_BODY_SM;
use massorca_ui::{ActiveTheme, IconName, TextStyled};

/// The delete confirm on screen: the routine it deletes, the sentence the last delete ended on, and whether one waits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::shell) struct RoutineConfirm {
    pub(in crate::shell) team: TeamId,
    pub(in crate::shell) routine: RoutineId,
    pub(in crate::shell) name: String,
    pub(in crate::shell) error: Option<String>,
    pub(in crate::shell) busy: bool,
}

/// The delete confirm as a dialog over the page.
pub(in crate::shell) fn routine_confirm_view(
    confirm: &RoutineConfirm,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let theme = *cx.theme();
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let dialog = Dialog::new(
        "delete-routine",
        format!("Delete routine {}?", confirm.name),
    )
    .variant(DialogVariant::Destructive)
    .open(true)
    .busy(confirm.busy)
    .child(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.secondary)
            .child(
                "Its runs are removed from its history. The tasks the runs started stay on the Board.",
            ),
    )
    .action("Delete routine")
    .action_icon(IconName::Trash2)
    .on_action(move |_, cx| {
        let _ = on_action.update(cx, |shell, cx| shell.confirm_routine_delete(cx));
    })
    .on_dismiss(move |_, cx| {
        let _ = on_dismiss.update(cx, |shell, cx| shell.close_routine_confirm(cx));
    });
    match &confirm.error {
        Some(message) => dialog.error(message.clone()),
        None => dialog,
    }
}

impl Shell {
    /// Asks before a routine is deleted; the confirm names it.
    pub(super) fn open_routine_confirm(
        &mut self,
        team: TeamId,
        routine: RoutineId,
        cx: &mut Context<Self>,
    ) {
        let name = self
            .model
            .teams()
            .team(team)
            .and_then(|team| team.routines.iter().find(|item| item.id == routine))
            .map(|item| item.name.clone());
        let Some(name) = name else {
            return;
        };
        self.routine_confirm = Some(RoutineConfirm {
            team,
            routine,
            name,
            error: None,
            busy: false,
        });
        cx.notify();
    }

    pub(super) fn close_routine_confirm(&mut self, cx: &mut Context<Self>) {
        self.routine_confirm = None;
        cx.notify();
    }

    /// Deletes the routine the confirm names: through the service when one is attached, else in the sample store.
    pub(super) fn confirm_routine_delete(&mut self, cx: &mut Context<Self>) {
        let Some(confirm) = self.routine_confirm.as_mut() else {
            return;
        };
        if confirm.busy {
            return;
        }
        let (team, routine) = (confirm.team, confirm.routine);
        let Some(service) = self.service.clone() else {
            let answer = self.model.teams_mut().remove_routine(team, routine);
            let error = answer.err().map(|error| error.message().to_owned());
            self.finish_routine_delete(team, routine, error, cx);
            return;
        };
        let (Some(team_key), Some(routine_key)) =
            (self.team_key(team, cx), self.routine_key(routine, cx))
        else {
            self.finish_routine_delete(team, routine, Some(ROUTINE_GONE.to_owned()), cx);
            return;
        };
        if let Some(confirm) = self.routine_confirm.as_mut() {
            confirm.busy = true;
        }
        cx.spawn(async move |this, cx| {
            let answer = service.remove_routine(team_key, routine_key).await;
            this.update(cx, |shell, cx| {
                let error = answer.err().map(|refusal| refusal.message());
                shell.finish_routine_delete(team, routine, error, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Closes the confirm and leaves the routine's pages when it is gone; otherwise the confirm says why not.
    fn finish_routine_delete(
        &mut self,
        team: TeamId,
        routine: RoutineId,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        match error {
            None => {
                self.routine_confirm = None;
                let on_its_page = matches!(
                    self.model.canvas(),
                    Canvas::Team(
                        TeamPage::EditRoutine(_, id) | TeamPage::RoutineHistory(_, id)
                    ) if id == routine
                );
                if on_its_page {
                    self.routine_form = None;
                    self.model.open_team_page(TeamPage::Routines(team));
                }
            }
            Some(message) => {
                if let Some(confirm) = self.routine_confirm.as_mut() {
                    confirm.busy = false;
                    confirm.error = Some(message);
                }
            }
        }
        cx.notify();
    }
}
