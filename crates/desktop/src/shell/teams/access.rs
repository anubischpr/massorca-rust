//! A team's access on its card and in the Board header: the line, the one-step change and the refusal of the
//! last change (decision 0072, amendment "team access").

use super::super::Shell;
use super::home::line;
use crate::model::teams::{Team, TeamError, TeamId, access_change, access_line};
use crate::service::ServiceHandle;
use gpui::{Context, IntoElement, ParentElement, Styled, WeakEntity, div, px};
use massorca_protocol::methods::teams::TeamAccess;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::tokens::{SPACE_1, TYPE_BODY_SM};
use massorca_ui::{TextStyled, Theme};

/// A team's access line, its one-step change and the refusal of its last change, when there was one.
pub(super) fn access_control(
    team: &Team,
    notice: Option<&str>,
    shell: &WeakEntity<Shell>,
    connected: bool,
    theme: &Theme,
) -> impl IntoElement {
    let mut control = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(line(access_line(team.access).to_owned(), theme));
    if let Some((label, target)) = access_change(team.access) {
        let shell = shell.clone();
        let id = team.id;
        control = control.child(
            Button::new(format!("team-access-{}", id.0), label)
                .variant(ButtonVariant::Ghost)
                .disabled(!connected)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.set_team_access(id, target, cx));
                }),
        );
    }
    if let Some(notice) = notice {
        control = control.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.status.danger_fg)
                .child(notice.to_owned()),
        );
    }
    control
}

impl Shell {
    /// Changes a team's access. A connected service writes it and the teams are read again; sample data changes
    /// locally. A refusal stays on the team's card and header until the next change.
    pub(super) fn set_team_access(
        &mut self,
        team: TeamId,
        access: TeamAccess,
        cx: &mut Context<Self>,
    ) {
        match self.service.clone() {
            Some(service) => self.send_team_access(service, team, access, cx),
            None => {
                let refusal = self
                    .model
                    .teams_mut()
                    .set_access(team, access)
                    .err()
                    .map(|error| error.message().to_owned());
                self.model.teams_mut().set_access_notice(team, refusal);
            }
        }
        cx.notify();
    }

    fn send_team_access(
        &mut self,
        service: ServiceHandle,
        team: TeamId,
        access: TeamAccess,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.team_key(team, cx) else {
            let gone = TeamError::Missing.message().to_owned();
            self.model.teams_mut().set_access_notice(team, Some(gone));
            return;
        };
        cx.spawn(async move |this, cx| {
            let answer = service.set_team_access(key, access).await;
            this.update(cx, |shell, cx| {
                let notice = answer.err().map(|refusal| refusal.message());
                shell.model.teams_mut().set_access_notice(team, notice);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
