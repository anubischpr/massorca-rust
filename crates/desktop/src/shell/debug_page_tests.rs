//! Element tests of the debug page: a request waits while its data is still arriving and opens once
//! it is here, and a dialog whose data the service has answered without falls back to its parent.

use super::service_tests::{draw, rig};
use super::*;
use crate::data::SampleDataSource;
use crate::launch_options::DebugPage;
use crate::model::navigation::{Mode, Navigation, Page};
use crate::model::settings_sections::SettingsSection;
use crate::model::teams::{SampleTeamsSource, TeamId, TeamPage, UnavailableTeams};
use crate::service::fixtures::team;
use crate::service::{StoreUpdate, TeamsRead};
use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_ui::{Appearance, Theme};

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn navigation(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Navigation {
    shell.read_with(cx, |shell, _| shell.model().navigation().clone())
}

fn sample_team_id(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> TeamId {
    shell
        .read_with(cx, |shell, _| {
            shell
                .model()
                .teams()
                .teams()
                .first()
                .map(|team| team.id.clone())
        })
        .expect("the sample team is listed")
}

#[gpui::test]
fn a_board_request_waits_for_the_teams_and_opens_once_they_arrive(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[]);
    let shell = rig.shell.clone();
    shell.update(cx, |shell, _| shell.start_debug_page(DebugPage::Board));
    draw(cx);
    assert_eq!(navigation(&shell, cx).page, Page::Home);
    assert!(
        shell.read_with(cx, |shell, _| shell.debug_page.is_some()),
        "the request waits while the teams are still arriving"
    );

    rig.updates
        .send(StoreUpdate::Teams(TeamsRead {
            number: 1,
            teams: Ok(vec![team("t1", "Desarrollo")]),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);

    let team = sample_team_id(&shell, cx);
    assert_eq!(
        navigation(&shell, cx).page,
        Page::Team(TeamPage::Board(team))
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.debug_page.is_none()),
        "the request is spent once it opens"
    );
}

#[gpui::test]
fn a_board_request_opens_on_the_first_frame_when_the_sample_teams_are_there(
    cx: &mut TestAppContext,
) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx);
        shell.start_debug_page(DebugPage::Board);
        shell
    });
    draw(cx);
    let team = sample_team_id(&shell, cx);
    assert_eq!(
        navigation(&shell, cx).page,
        Page::Team(TeamPage::Board(team))
    );
}

#[gpui::test]
fn a_remove_secret_request_without_a_secret_opens_the_vault_settings(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx);
        shell.start_debug_page(DebugPage::RemoveSecret);
        shell
    });
    draw(cx);
    assert_eq!(navigation(&shell, cx).page, Page::Settings);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.settings.section),
        SettingsSection::Vault
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.vault_remove.is_none()),
        "no confirm opens without a secret to remove"
    );
    assert!(shell.read_with(cx, |shell, _| shell.debug_page.is_none()));
}

#[gpui::test]
fn a_new_team_request_without_bot_teams_opens_the_bot_teams_home(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = Shell::new(&SampleDataSource, &UnavailableTeams, window, cx);
        shell.start_debug_page(DebugPage::NewTeam);
        shell
    });
    draw(cx);
    let nav = navigation(&shell, cx);
    assert_eq!((nav.mode, nav.page), (Mode::BotTeams, Page::Home));
    assert!(
        shell.read_with(cx, |shell, _| shell.team_dialog.is_none()),
        "no New team dialog opens without Bot Teams"
    );
}

#[gpui::test]
fn an_add_host_request_opens_the_hosts_dialog_without_data(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx);
        shell.start_debug_page(DebugPage::AddHost);
        shell
    });
    draw(cx);
    assert_eq!(navigation(&shell, cx).page, Page::Settings);
    assert!(shell.read_with(cx, |shell, _| shell.add_host.is_some()));
}
