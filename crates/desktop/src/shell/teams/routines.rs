//! The Routines page of a team (D-AG-25, screens 4.6): each routine with its schedule, bot, host, last
//! run and an enable switch, and the team's web addresses with their tokens hidden. A routine's form
//! is not built yet; "New routine" opens the page that says so.

use super::super::Shell;
use super::super::change;
use crate::model::shell::HostReach;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{AddressId, Routine, RoutineId, Team, TeamId, TeamPage};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, WeakEntity, div, prelude::*, px};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::card::{Card, CardPadding};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{SPACE_2, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_CAPTION, TYPE_HEADING};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

/// The Routines page of one team (4.6).
pub(in crate::shell) fn routines_page(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    team: TeamId,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let Some(team_data) = model.teams().team(team) else {
        return note("This team is gone.", &theme).into_any_element();
    };
    let connected = model.teams().is_connected();
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("Routines"),
        )
        .child(new_routine_button(shell, team, connected));
    let routines = if team_data.routines.is_empty() {
        EmptyState::new("routines-empty", "No routines yet")
            .explanation(format!(
                "A routine starts a task on a schedule, with one bot in {}. Each run is kept in its history.",
                team_data.name
            ))
            .action(new_routine_button(shell, team, connected))
            .into_any_element()
    } else {
        let mut list = div().flex().flex_wrap().gap(px(SPACE_4));
        for routine in &team_data.routines {
            list = list.child(routine_card(routine, team, team_data, shell, &theme));
        }
        list.into_any_element()
    };
    let addresses = addresses(team_data, shell, &theme);
    div()
        .id("routines-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(header)
        .child(note(
            "A routine starts a task on a schedule. Each run is kept in its history.",
            &theme,
        ))
        .child(section("Routines", &theme))
        .child(routines)
        .child(section("Web addresses", &theme))
        .child(addresses)
        .child(note(
            &format!(
                "Runs on {}, the host {} uses.",
                team_data.host, team_data.name
            ),
            &theme,
        ))
        .into_any_element()
}

fn new_routine_button(shell: &WeakEntity<Shell>, team: TeamId, connected: bool) -> Button {
    let shell = shell.clone();
    Button::new("routines-new", "New routine")
        .variant(ButtonVariant::Primary)
        .disabled(!connected)
        .on_click(move |_, cx| {
            change(&shell, cx, move |shell| {
                shell.model.open_team_page(TeamPage::NewRoutine(team));
            });
        })
}

/// A routine card: its name, the schedule in words, the bot and the last run, and an enable switch.
fn routine_card(
    routine: &Routine,
    team: TeamId,
    team_data: &Team,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> impl IntoElement {
    let bot = routine
        .bot
        .and_then(|id| team_data.bot(id))
        .map_or("No bot", |bot| bot.name.as_str());
    let last = routine
        .last_run
        .map_or("No runs yet", |outcome| outcome.word());
    let schedule = routine.cadence.words(&routine.time);
    let switch = {
        let shell = shell.clone();
        let id = routine.id;
        Switch::new(format!("routine-switch-{}", routine.id.0), routine.enabled)
            .label(routine.name.clone())
            .helper(if routine.enabled { "On" } else { "Paused" })
            .on_change(move |on, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.set_routine_enabled(team, id, on, cx));
            })
    };
    let host_reach = team_data.reach;
    div().w(px(320.0)).child(
        Card::new(format!("routine-{}", routine.id.0))
            .padding(CardPadding::Comfortable)
            .title(routine.name.clone())
            .child(line(schedule, theme))
            .child(line(
                format!("Bot: {bot} · Runs on {}", routine.host),
                theme,
            ))
            .child(line(format!("Last run: {last}"), theme))
            .child(match host_reach {
                HostReach::Connected => div().into_any_element(),
                HostReach::Unreachable { minutes } => {
                    line(format!("Last known state, {minutes} min ago"), theme).into_any_element()
                }
            })
            .footer_action(switch),
    )
}

/// The web addresses of the team: a card each with its masked address, or an empty state.
fn addresses(team: &Team, shell: &WeakEntity<Shell>, theme: &Theme) -> AnyElement {
    if team.addresses.is_empty() {
        return EmptyState::new("addresses-empty", "No web addresses yet")
            .explanation(format!(
                "Another app can call a web address, a webhook, to start one task in {}.",
                team.name
            ))
            .into_any_element();
    }
    let mut list = div().flex().flex_wrap().gap(px(SPACE_4));
    for address in &team.addresses {
        let id: AddressId = address.id;
        let team_id = team.id;
        let delete = {
            let shell = shell.clone();
            Button::new(format!("address-delete-{}", address.id.0), "Delete")
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.delete_address(team_id, id, cx));
                })
        };
        list = list.child(
            div().w(px(320.0)).child(
                Card::new(format!("address-{}", address.id.0))
                    .padding(CardPadding::Comfortable)
                    .title(address.name.clone())
                    .child(line(
                        format!("Host {} · created {}", address.host, address.created),
                        theme,
                    ))
                    .child(line(address.masked(), theme))
                    .footer_action(delete),
            ),
        );
    }
    list.into_any_element()
}

fn section(title: &'static str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.secondary)
        .child(title)
}

fn line(text: String, theme: &Theme) -> impl IntoElement {
    div()
        .pt(px(SPACE_2))
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(text)
}

fn note(text: &str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}

impl Shell {
    /// Switches a routine on or off in the sample store.
    pub(super) fn set_routine_enabled(
        &mut self,
        team: TeamId,
        routine: RoutineId,
        enabled: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self
            .model
            .teams_mut()
            .set_routine_enabled(team, routine, enabled);
        cx.notify();
    }

    /// Deletes one web address of a team.
    pub(super) fn delete_address(
        &mut self,
        team: TeamId,
        address: AddressId,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.model.teams_mut().delete_address(team, address);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::canvas::Canvas;
    use crate::model::navigation::Mode;
    use crate::model::teams::SampleTeamsSource;
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use massorca_ui::{Appearance, Theme as UiTheme};

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| {
            UiTheme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        })
    }

    #[gpui::test]
    fn the_routines_page_switches_a_routine_off_and_back_on(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Routines(TeamId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let enabled = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                shell.model().teams().team(TeamId(1)).unwrap().routines[2].enabled
            })
        };
        assert!(!enabled(&shell, cx), "the third routine starts paused");
        shell.update(cx, |shell, cx| {
            shell.set_routine_enabled(TeamId(1), RoutineId(3), true, cx);
        });
        assert!(enabled(&shell, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[gpui::test]
    fn new_routine_opens_its_page_which_says_the_form_is_not_built(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::NewRoutine(TeamId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.model().canvas(),
                Canvas::Team(TeamPage::NewRoutine(TeamId(1)))
            );
            assert_eq!(shell.model().navigation().mode, Mode::BotTeams);
        });
    }
}
