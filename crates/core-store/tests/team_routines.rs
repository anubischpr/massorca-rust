//! Team routines: a claim is a compare-and-swap on `next_run_at` that also records its run, pausing clears the next
//! run, deleting a routine removes its runs and never its tasks, and the team and bot removals cascade as documented.

use massorca_core_store::{
    BotRow, Store, TaskOrigin, TaskRow, TeamRoutineRow, TeamRoutineRunRow, TeamRow, new_id,
};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn team(store: &Store) -> TeamRow {
    let row = TeamRow {
        id: new_id(),
        name: "Desarrollo".to_owned(),
        purpose: String::new(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: 1,
    };
    store.insert_team(&row).unwrap();
    row
}

fn bot(store: &Store, team_id: &str) -> BotRow {
    let row = BotRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        name: "Mara".to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead: false,
        created_at: 1,
    };
    store.insert_bot(&row).unwrap();
    row
}

fn routine(team_id: &str, bot_id: Option<&str>, next_run_at: Option<i64>) -> TeamRoutineRow {
    TeamRoutineRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        bot_id: bot_id.map(str::to_owned),
        name: "Morning check".to_owned(),
        prompt: "Read the open issues.".to_owned(),
        cadence: "daily".to_owned(),
        time: "09:00".to_owned(),
        weekday: None,
        month_day: None,
        run_date: None,
        zone: Some("Europe/Madrid".to_owned()),
        paused: false,
        starts_as_admin: true,
        next_run_at,
        created_at: 1,
    }
}

fn run(routine_id: &str, scheduled_for: i64) -> TeamRoutineRunRow {
    TeamRoutineRunRow {
        id: new_id(),
        routine_id: routine_id.to_owned(),
        scheduled_for,
        task_id: None,
        error: None,
        created_at: scheduled_for,
    }
}

fn task(team_id: &str, bot_id: &str) -> TaskRow {
    TaskRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        project_id: None,
        bot_id: Some(bot_id.to_owned()),
        title: "Morning check".to_owned(),
        body: String::new(),
        origin: TaskOrigin::Cli,
        session_id: None,
        outcome: None,
        error: None,
        error_code: None,
        workspace_id: None,
        branch: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn next_run_at(store: &Store, id: &str) -> Option<i64> {
    store.get_routine(id).unwrap().unwrap().next_run_at
}

#[test]
fn a_claim_wins_once_and_writes_its_run_with_the_next_occurrence() {
    let store = store();
    let team = team(&store);
    let routine = routine(&team.id, None, Some(100));
    store.insert_routine(&routine).unwrap();

    assert!(
        store
            .claim_routine_run(100, Some(200), &run(&routine.id, 100))
            .unwrap()
    );
    assert_eq!(next_run_at(&store, &routine.id), Some(200));
    assert_eq!(store.list_routine_runs(&routine.id, 10).unwrap().len(), 1);

    // Another caller holding the same stored time loses, and writes no run.
    assert!(
        !store
            .claim_routine_run(100, Some(300), &run(&routine.id, 100))
            .unwrap()
    );
    assert_eq!(next_run_at(&store, &routine.id), Some(200));
    assert_eq!(store.list_routine_runs(&routine.id, 10).unwrap().len(), 1);
}

#[test]
fn a_claim_after_several_missed_occurrences_records_one_run_for_the_latest() {
    let store = store();
    let team = team(&store);
    let routine = routine(&team.id, None, Some(100));
    store.insert_routine(&routine).unwrap();

    // Occurrences at 100, 200 and 300 were missed; the claim at 350 stands for 300 only.
    assert!(
        store
            .claim_routine_run(100, Some(400), &run(&routine.id, 300))
            .unwrap()
    );
    let runs = store.list_routine_runs(&routine.id, 10).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].scheduled_for, 300);
}

#[test]
fn due_routines_are_at_or_before_now_and_neither_paused_nor_finished() {
    let store = store();
    let team = team(&store);
    let bot = bot(&store, &team.id);
    let due = routine(&team.id, Some(&bot.id), Some(100));
    let later = routine(&team.id, Some(&bot.id), Some(500));
    let finished = routine(&team.id, Some(&bot.id), None);
    for row in [&due, &later, &finished] {
        store.insert_routine(row).unwrap();
    }

    let ids: Vec<String> = store
        .due_routines(100)
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(ids, vec![due.id]);
}

#[test]
fn pausing_clears_the_next_run_and_a_paused_routine_is_never_due() {
    let store = store();
    let team = team(&store);
    let routine = routine(&team.id, None, Some(100));
    store.insert_routine(&routine).unwrap();

    let mut paused = store.get_routine(&routine.id).unwrap().unwrap();
    paused.paused = true;
    paused.next_run_at = None;
    assert!(store.update_routine(&paused).unwrap());

    assert_eq!(next_run_at(&store, &routine.id), None);
    assert!(store.get_routine(&routine.id).unwrap().unwrap().paused);
    assert!(store.due_routines(1_000).unwrap().is_empty());
}

#[test]
fn deleting_a_routine_removes_its_runs_and_keeps_its_tasks() {
    let store = store();
    let team = team(&store);
    let bot = bot(&store, &team.id);
    let routine = routine(&team.id, Some(&bot.id), None);
    store.insert_routine(&routine).unwrap();
    let task = task(&team.id, &bot.id);
    store.insert_task(&task).unwrap();
    let mut linked = run(&routine.id, 100);
    linked.task_id = Some(task.id.clone());
    store.insert_routine_run(&linked).unwrap();

    assert!(store.delete_routine(&routine.id).unwrap());

    assert!(store.get_routine(&routine.id).unwrap().is_none());
    assert!(store.list_routine_runs(&routine.id, 10).unwrap().is_empty());
    assert!(store.get_task(&task.id).unwrap().is_some());
}

#[test]
fn removing_a_team_removes_its_routines_and_their_runs() {
    let store = store();
    let team = team(&store);
    let routine = routine(&team.id, None, None);
    store.insert_routine(&routine).unwrap();
    store.insert_routine_run(&run(&routine.id, 100)).unwrap();

    assert!(store.delete_team(&team.id).unwrap());

    assert!(store.get_routine(&routine.id).unwrap().is_none());
    assert!(store.list_routine_runs(&routine.id, 10).unwrap().is_empty());
}

#[test]
fn removing_a_bot_keeps_its_routine_with_no_bot() {
    let store = store();
    let team = team(&store);
    let bot = bot(&store, &team.id);
    let routine = routine(&team.id, Some(&bot.id), Some(100));
    store.insert_routine(&routine).unwrap();

    assert!(store.delete_bot(&bot.id).unwrap());

    let kept = store.get_routine(&routine.id).unwrap().unwrap();
    assert_eq!(kept.bot_id, None);
    assert_eq!(kept.next_run_at, Some(100));
}

#[test]
fn runs_list_newest_first_and_a_limit_keeps_the_newest() {
    let store = store();
    let team = team(&store);
    let routine = routine(&team.id, None, None);
    store.insert_routine(&routine).unwrap();
    for scheduled_for in [100, 300, 200] {
        store
            .insert_routine_run(&run(&routine.id, scheduled_for))
            .unwrap();
    }

    let newest_two: Vec<i64> = store
        .list_routine_runs(&routine.id, 2)
        .unwrap()
        .iter()
        .map(|run| run.scheduled_for)
        .collect();
    assert_eq!(newest_two, vec![300, 200]);
}

#[test]
fn a_run_records_its_task_and_its_error() {
    let store = store();
    let team = team(&store);
    let bot = bot(&store, &team.id);
    let routine = routine(&team.id, Some(&bot.id), None);
    store.insert_routine(&routine).unwrap();
    let task = task(&team.id, &bot.id);
    store.insert_task(&task).unwrap();
    let pending = run(&routine.id, 100);
    store.insert_routine_run(&pending).unwrap();

    assert!(
        store
            .set_routine_run_result(&pending.id, Some(&task.id), Some("bot.busy"))
            .unwrap()
    );

    let saved = store.get_routine_run(&pending.id).unwrap().unwrap();
    assert_eq!(saved.task_id.as_deref(), Some(task.id.as_str()));
    assert_eq!(saved.error.as_deref(), Some("bot.busy"));
}

#[test]
fn the_store_refuses_an_unknown_cadence_and_a_month_day_outside_one_to_twenty_eight() {
    let store = store();
    let team = team(&store);

    let mut hourly = routine(&team.id, None, None);
    hourly.cadence = "hourly".to_owned();
    assert!(store.insert_routine(&hourly).is_err());

    let mut day_29 = routine(&team.id, None, None);
    day_29.cadence = "monthly".to_owned();
    day_29.month_day = Some(29);
    assert!(store.insert_routine(&day_29).is_err());
}
