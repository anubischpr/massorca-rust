//! The routines of a host through the binding, against a real in-process server: a phone lists the teams, their bots and
//! routines with each schedule, pauses and resumes a routine, and reads its runs (AUTO-28).

mod common;

use std::path::Path;
use std::sync::Arc;

use massorca_client_ffi::{Host, MassOrca};
use massorca_protocol::methods::teams::{
    BotAddParams, BotRemoveParams, RoutineAddParams, RoutineCadence, TeamCreateParams,
};
use tokio::runtime::Runtime;

use common::{app, offer, owner, start, stop, temp_home};

/// Team `alpha` with bot `mara` and a daily 09:00 routine `morning` in UTC, made by the owner. Returns their ids.
fn seed(test: &Runtime, home: &Path) -> (String, String, String) {
    test.block_on(async {
        let client = owner(home).await;
        let team = client
            .team_create(&TeamCreateParams {
                name: "alpha".into(),
                purpose: None,
                character: None,
                access: None,
                workspaces: None,
            })
            .await
            .expect("a team");
        let bot = client
            .team_bot_add(&BotAddParams {
                team: team.id.clone(),
                name: "mara".into(),
                job: "Reads the notes.".into(),
                kind: "claude".into(),
                model: None,
                effort: None,
                lead: true,
                skills: Vec::new(),
            })
            .await
            .expect("a bot");
        let routine = client
            .team_routine_add(&RoutineAddParams {
                team: team.id.clone(),
                bot: bot.id.clone(),
                name: "morning".into(),
                prompt: "Read the notes.".into(),
                cadence: RoutineCadence::Daily,
                time: "09:00".into(),
                weekday: None,
                month_day: None,
                run_date: None,
                zone: Some("UTC".into()),
            })
            .await
            .expect("a routine");
        (team.id, bot.id, routine.id)
    })
}

/// The app, paired with the host under `home` as `office` and connected. The app is returned too: a host needs its runtime.
fn paired_host(test: &Runtime, home: &Path) -> (Arc<MassOrca>, Arc<Host>) {
    let massorca = app();
    let text = test.block_on(offer(home));
    test.block_on(massorca.pair(text, "office".into()))
        .expect("paired");
    let host = test
        .block_on(massorca.connect("office".into()))
        .expect("connected");
    (massorca, host)
}

#[test]
fn a_phone_lists_the_teams_bots_and_routines_with_their_schedule() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let (team, bot, routine) = seed(&test, &home);
    let (_massorca, host) = paired_host(&test, &home);

    let teams = test.block_on(host.teams()).expect("teams");
    let names: Vec<&str> = teams.iter().map(|listed| listed.name.as_str()).collect();
    assert_eq!(names, ["alpha"]);

    let bots = test.block_on(host.team_bots(team.clone())).expect("bots");
    assert_eq!(bots.len(), 1);
    assert_eq!(bots[0].name, "mara");
    assert!(bots[0].lead);

    let routines = test
        .block_on(host.team_routines(team.clone()))
        .expect("routines");
    assert_eq!(routines.len(), 1);
    let listed = &routines[0];
    assert_eq!(listed.id, routine);
    assert_eq!(listed.bot_id.as_deref(), Some(bot.as_str()));
    assert_eq!(listed.cadence, "daily");
    assert_eq!(listed.time, "09:00");
    assert_eq!(listed.zone.as_deref(), Some("UTC"));
    assert!(!listed.paused);
    assert!(listed.next_run_at_ms.is_some(), "{listed:?}");
    test.block_on(stop(running));
}

#[test]
fn pausing_a_routine_takes_away_its_next_run_and_resuming_gives_one_back() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let (team, _bot, routine) = seed(&test, &home);
    let (_massorca, host) = paired_host(&test, &home);

    let paused = test
        .block_on(host.pause_routine(team.clone(), routine.clone()))
        .expect("paused");
    assert!(paused.paused, "{paused:?}");
    assert_eq!(paused.next_run_at_ms, None);
    let listed = test
        .block_on(host.team_routines(team.clone()))
        .expect("routines");
    assert!(listed[0].paused);
    assert_eq!(listed[0].next_run_at_ms, None);

    let resumed = test
        .block_on(host.resume_routine(team, routine))
        .expect("resumed");
    assert!(!resumed.paused, "{resumed:?}");
    assert!(resumed.next_run_at_ms.is_some(), "{resumed:?}");
    test.block_on(stop(running));
}

#[test]
fn a_run_made_now_is_recorded_even_when_its_bot_is_gone() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let (team, bot, routine) = seed(&test, &home);
    // A removed bot leaves the routine with none, so the run ends before it makes a task: no agent starts.
    test.block_on(async {
        owner(&home)
            .await
            .team_bot_remove(&BotRemoveParams {
                team: team.clone(),
                bot,
                unassign: false,
            })
            .await
            .expect("the bot is removed");
    });
    let (_massorca, host) = paired_host(&test, &home);

    let listed = test
        .block_on(host.team_routines(team.clone()))
        .expect("routines");
    assert_eq!(listed[0].bot_id, None);

    let run = test
        .block_on(host.run_routine(team.clone(), routine.clone()))
        .expect("a run");
    assert_eq!(run.routine_id, routine);
    assert_eq!(run.task_id, None, "{run:?}");
    assert_eq!(run.error_code.as_deref(), Some("bot.not_found"), "{run:?}");

    let runs = test
        .block_on(host.routine_runs(team, routine, Some(1)))
        .expect("runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, run.id);
    assert_eq!(runs[0].error_code.as_deref(), Some("bot.not_found"));
    test.block_on(stop(running));
}

#[test]
fn an_unknown_team_or_routine_is_named_by_its_protocol_code() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let (team, _bot, _routine) = seed(&test, &home);
    let (_massorca, host) = paired_host(&test, &home);

    let no_team = test.block_on(host.team_routines("no-such-team".into()));
    assert_eq!(no_team.unwrap_err().code(), "team.not_found");

    let no_routine = test.block_on(host.pause_routine(team, "no-such-routine".into()));
    assert_eq!(no_routine.unwrap_err().code(), "team.routine_not_found");
    test.block_on(stop(running));
}
