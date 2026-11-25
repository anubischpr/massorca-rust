//! Element tests of the routine pages (AUTO-28, screens 4.6 and 4.7): the list, the form, the pause switch, the
//! delete confirm and the run history, on a service store whose calls are answered by hand, and on the sample store.

use super::super::service_tests::{Rig, draw, rig};
use super::*;
use crate::data::SampleDataSource;
use crate::model::canvas::Canvas;
use crate::model::teams::{
    RoutineField, RoutineId, RunsState, SampleTeamsSource, ScheduleKind, TeamId, TeamPage, Weekday,
};
use crate::service::fixtures::team;
use crate::service::{Command, RoutineCommand, StoreUpdate, TeamRecord, TeamsRead};
use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_protocol::methods::teams::{
    Bot as WireBot, Routine as WireRoutine, RoutineCadence, RoutineRun, RoutineWeekday,
};
use massorca_ui::{Appearance, Theme as UiTheme};

const NEXT_RUN_MS: i64 = 1_900_000_000_000;

fn wire_bot() -> WireBot {
    WireBot {
        id: "b1".into(),
        team_id: "t1".into(),
        name: "Mara".into(),
        job: "Reviews".into(),
        kind: "claude".into(),
        model: None,
        effort: None,
        lead: false,
        created_at_ms: 1,
    }
}

fn wire_routine(id: &str, name: &str) -> WireRoutine {
    WireRoutine {
        id: id.into(),
        team_id: "t1".into(),
        bot_id: Some("b1".into()),
        name: name.into(),
        prompt: "Summarize the board".into(),
        cadence: RoutineCadence::Daily,
        time: "09:00".into(),
        weekday: None,
        month_day: None,
        run_date: None,
        zone: None,
        zone_used: None,
        paused: false,
        starts_as_admin: false,
        next_run_at_ms: Some(NEXT_RUN_MS),
        created_at_ms: 1,
    }
}

/// Team "t1" with bot "b1" and routine "r1", as the service lists them. The store numbers them 1, 2 and 3.
fn team_with_routine() -> TeamRecord {
    let mut record = team("t1", "Desarrollo");
    record.bots = vec![wire_bot()];
    record.routines = vec![wire_routine("r1", "Morning digest")];
    record
}

fn send_team(rig: &Rig, cx: &mut VisualTestContext, record: TeamRecord) {
    rig.updates
        .send(StoreUpdate::Teams(TeamsRead {
            number: 1,
            teams: Ok(vec![record]),
        }))
        .expect("the store listens");
    cx.run_until_parked();
}

fn open_page(rig: &Rig, cx: &mut VisualTestContext, page: TeamPage) {
    rig.shell.update(cx, |shell, cx| {
        shell.model.press_mode_switch();
        shell.model.open_team_page(page);
        cx.notify();
    });
    draw(cx);
}

/// The Routines page of the service team, with its routine listed.
fn service_routines(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[]);
    send_team(&rig, cx, team_with_routine());
    open_page(&rig, cx, TeamPage::Routines(TeamId(1)));
    (rig, cx)
}

#[gpui::test]
fn the_service_routine_is_listed_with_its_next_run(cx: &mut TestAppContext) {
    let (rig, cx) = service_routines(cx);
    rig.shell.read_with(cx, |shell, _| {
        let team = shell.model().teams().team(TeamId(1)).expect("the team");
        let routine = &team.routines[0];
        assert_eq!(routine.name, "Morning digest");
        assert_eq!(routine.next_run_at_ms, Some(NEXT_RUN_MS));
        assert_eq!(
            team.bot(routine.bot.expect("a bot"))
                .map(|bot| bot.name.as_str()),
            Some("Mara")
        );
    });
    assert!(cx.debug_bounds("routines-body").is_some());
}

#[gpui::test]
fn a_new_weekly_routine_sends_its_fields_to_the_service(cx: &mut TestAppContext) {
    let (mut rig, cx) = service_routines(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.open_routine_form(TeamPage::NewRoutine(TeamId(1)), cx);
        shell.routine_text_changed(RoutineField::Name, "Weekly review".into(), cx);
        shell.routine_text_changed(RoutineField::Prompt, "Review the week".into(), cx);
        shell.routine_kind_chosen(2, cx);
        shell.routine_weekday_chosen(Weekday::Friday, cx);
        shell.save_routine_form(cx);
    });
    cx.run_until_parked();
    let Some(Command::Routine(RoutineCommand::Add { params, reply })) = rig.script.next() else {
        panic!("saving a new routine calls team.routine_add");
    };
    assert_eq!(params.team, "t1");
    assert_eq!(params.bot, "b1", "the team's first bot is chosen");
    assert_eq!(params.name, "Weekly review");
    assert_eq!(params.cadence, RoutineCadence::Weekly);
    assert_eq!(params.weekday, Some(RoutineWeekday::Friday));
    assert_eq!((params.month_day, params.run_date.as_deref()), (None, None));
    assert_eq!(params.time, "09:00");
    reply
        .send(Ok(wire_routine("r2", "Weekly review")))
        .expect("the form listens");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.model().canvas(),
            Canvas::Team(TeamPage::Routines(TeamId(1)))
        );
    });
}

#[gpui::test]
fn a_form_without_a_name_is_refused_before_the_service_sees_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = service_routines(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.open_routine_form(TeamPage::NewRoutine(TeamId(1)), cx);
        shell.save_routine_form(cx);
    });
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "no call leaves for an invalid draft"
    );
    rig.shell.read_with(cx, |shell, _| {
        let form = shell.routine_form.as_ref().expect("the form stays open");
        assert_eq!(form.error.as_deref(), Some("Give the routine a name."));
    });
}

#[gpui::test]
fn editing_a_routine_opens_its_draft_and_saves_only_what_changed(cx: &mut TestAppContext) {
    let (mut rig, cx) = service_routines(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.open_routine_form(TeamPage::EditRoutine(TeamId(1), RoutineId(3)), cx);
    });
    rig.shell.read_with(cx, |shell, _| {
        let form = shell.routine_form.as_ref().expect("the edit form is open");
        assert_eq!(form.draft.name, "Morning digest");
        assert_eq!(form.draft.kind, ScheduleKind::Daily);
    });
    rig.shell.update(cx, |shell, cx| {
        shell.routine_text_changed(RoutineField::Name, "Evening digest".into(), cx);
        shell.save_routine_form(cx);
    });
    cx.run_until_parked();
    let Some(Command::Routine(RoutineCommand::Update { params, .. })) = rig.script.next() else {
        panic!("saving an edit calls team.routine_update");
    };
    assert_eq!(
        (params.team.as_str(), params.routine.as_str()),
        ("t1", "r1")
    );
    assert_eq!(params.name.as_deref(), Some("Evening digest"));
    assert_eq!(params.zone, None, "an edit keeps the routine's zone");
}

#[gpui::test]
fn the_pause_switch_sends_pause_and_resume_to_the_service(cx: &mut TestAppContext) {
    let (mut rig, cx) = service_routines(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.set_routine_enabled(TeamId(1), RoutineId(3), false, cx);
    });
    cx.run_until_parked();
    let Some(Command::Routine(RoutineCommand::SetPaused {
        team,
        routine,
        paused,
        ..
    })) = rig.script.next()
    else {
        panic!("the switch calls team.routine_pause");
    };
    assert_eq!(
        (team.as_str(), routine.as_str(), paused),
        ("t1", "r1", true)
    );
    rig.shell.update(cx, |shell, cx| {
        shell.set_routine_enabled(TeamId(1), RoutineId(3), true, cx);
    });
    cx.run_until_parked();
    let Some(Command::Routine(RoutineCommand::SetPaused { paused, .. })) = rig.script.next() else {
        panic!("switching back on calls team.routine_resume");
    };
    assert!(!paused);
}

#[gpui::test]
fn deleting_a_routine_asks_first_and_then_sends_the_removal(cx: &mut TestAppContext) {
    let (mut rig, cx) = service_routines(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.open_routine_confirm(TeamId(1), RoutineId(3), cx);
    });
    assert!(
        rig.script.next().is_none(),
        "nothing is deleted before the confirm"
    );
    rig.shell
        .update(cx, |shell, cx| shell.confirm_routine_delete(cx));
    cx.run_until_parked();
    let Some(Command::Routine(RoutineCommand::Remove { team, routine, .. })) = rig.script.next()
    else {
        panic!("the confirm calls team.routine_rm");
    };
    assert_eq!((team.as_str(), routine.as_str()), ("t1", "r1"));
}

#[gpui::test]
fn the_run_history_asks_the_service_and_lists_each_run(cx: &mut TestAppContext) {
    let (mut rig, cx) = service_routines(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.open_routine_history(TeamId(1), RoutineId(3), cx);
    });
    cx.run_until_parked();
    let Some(Command::Routine(RoutineCommand::Runs {
        team,
        routine,
        reply,
    })) = rig.script.next()
    else {
        panic!("the history calls team.routine_runs");
    };
    assert_eq!((team.as_str(), routine.as_str()), ("t1", "r1"));
    reply
        .send(Ok(vec![RoutineRun {
            id: "run-1".into(),
            routine_id: "r1".into(),
            scheduled_for_ms: NEXT_RUN_MS,
            task_id: Some("k9".into()),
            error: None,
            error_code: None,
            created_at_ms: 1,
        }]))
        .expect("the history listens");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let open = shell
            .model()
            .teams()
            .routine_history()
            .expect("the history is open");
        match &open.runs {
            RunsState::Loaded(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].status, "Task started");
            }
            other => panic!("the runs are loaded, not {other:?}"),
        }
    });
}

/// A shell on the sample store, on the Routines page of the sample team.
fn sample_routines(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| {
        UiTheme::set(Appearance::Dark, cx);
        crate::shell::bind_keys(cx);
    });
    let (shell, cx) = cx.add_window_view(|window, cx| {
        Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
    });
    shell.update(cx, |shell, cx| {
        shell.model.press_mode_switch();
        shell.model.open_team_page(TeamPage::Routines(TeamId(1)));
        cx.notify();
    });
    draw(cx);
    (shell, cx)
}

#[gpui::test]
fn the_sample_store_saves_a_once_routine_without_a_service(cx: &mut TestAppContext) {
    let (shell, cx) = sample_routines(cx);
    let before = shell.read_with(cx, |shell, _| {
        shell
            .model()
            .teams()
            .team(TeamId(1))
            .expect("the sample team")
            .routines
            .len()
    });
    shell.update(cx, |shell, cx| {
        shell.open_routine_form(TeamPage::NewRoutine(TeamId(1)), cx);
        shell.routine_text_changed(RoutineField::Name, "Board review".into(), cx);
        shell.routine_text_changed(RoutineField::Prompt, "Read the board".into(), cx);
        shell.routine_kind_chosen(0, cx);
        shell.routine_text_changed(RoutineField::RunDate, "2026-12-01".into(), cx);
        shell.save_routine_form(cx);
    });
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        let routines = &shell
            .model()
            .teams()
            .team(TeamId(1))
            .expect("the team")
            .routines;
        assert_eq!(routines.len(), before + 1);
        let added = routines.last().expect("the new routine");
        assert_eq!(added.name, "Board review");
        assert_eq!(added.run_date.as_deref(), Some("2026-12-01"));
        assert_eq!(
            shell.model().canvas(),
            Canvas::Team(TeamPage::Routines(TeamId(1)))
        );
    });
}
