//! The Calendar over the wire (decision 0124): the capability is offered, a window longer than 62 days is refused with its
//! code, and routines and automations share one window that the kind filter separates.

use std::path::Path;

use massorca_client::Client;
use massorca_protocol::capabilities;
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{AutomationCadence, AutomationCreateParams};
use massorca_protocol::methods::calendar::{
    self, CalendarKind, CalendarOccurrenceList, CalendarOccurrencesParams,
};
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::teams::{
    BotAddParams, RoutineAddParams, RoutineCadence, TeamCreateParams,
};

use super::common::{connect, remote_code, start, stop, temp_home};

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 24 * HOUR_MS;
/// 2030-01-01 00:00 UTC, far enough ahead that every run in the window is upcoming.
const JAN_1_2030: i64 = 1_893_456_000_000;

fn window(kind: Option<CalendarKind>) -> CalendarOccurrencesParams {
    CalendarOccurrencesParams {
        from_ms: JAN_1_2030,
        to_ms: JAN_1_2030 + DAY_MS,
        kind,
        include_done: false,
    }
}

async fn list(client: &Client, params: CalendarOccurrencesParams) -> CalendarOccurrenceList {
    client
        .call(calendar::OCCURRENCES, &params)
        .await
        .expect("calendar.occurrences")
}

/// A folder project named `name` inside `home`.
async fn folder_project(client: &Client, home: &Path, name: &str) -> Project {
    let folder = home.join(name);
    std::fs::create_dir_all(&folder).expect("create the folder");
    client
        .call(
            projects::ADD,
            &ProjectAddParams {
                icon: None,
                path: folder.to_str().expect("utf-8 path").to_owned(),
                name: Some(name.to_owned()),
                kind: Some(ProjectKind::Folder),
                workspaces_root: None,
            },
        )
        .await
        .expect("project.add")
}

#[tokio::test]
async fn the_calendar_is_offered_and_a_window_longer_than_62_days_is_refused_with_its_code() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    assert!(client.has_capability(capabilities::CALENDAR));

    let too_long = CalendarOccurrencesParams {
        to_ms: JAN_1_2030 + 62 * DAY_MS + 1,
        ..window(None)
    };
    let error = client
        .call::<_, CalendarOccurrenceList>(calendar::OCCURRENCES, &too_long)
        .await
        .expect_err("a window of 62 days and a millisecond is refused");
    assert_eq!(remote_code(&error), codes::CALENDAR_WINDOW_INVALID);

    let exact = CalendarOccurrencesParams {
        to_ms: JAN_1_2030 + 62 * DAY_MS,
        ..window(None)
    };
    list(&client, exact).await;
    stop(running).await;
}

#[tokio::test]
async fn routines_and_automations_share_one_window_and_the_kind_filter_separates_them() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;
    client
        .automation_create(&AutomationCreateParams {
            project: "app".to_owned(),
            name: "Morning PRs".to_owned(),
            prompt: "Summarise the open pull requests".to_owned(),
            agent: "claude".to_owned(),
            cadence: AutomationCadence::Daily,
            time: "09:00".to_owned(),
            weekday: None,
            cron: None,
            zone: Some("UTC".to_owned()),
            grace_minutes: None,
            creation_key: None,
        })
        .await
        .expect("automation.create");
    client
        .team_create(&TeamCreateParams {
            name: "Desarrollo".to_owned(),
            purpose: None,
            character: None,
            access: None,
            workspaces: None,
        })
        .await
        .expect("team.create");
    client
        .team_bot_add(&BotAddParams {
            team: "Desarrollo".to_owned(),
            name: "Mara".to_owned(),
            job: "Writes the parser".to_owned(),
            kind: "claude".to_owned(),
            model: None,
            effort: None,
            lead: false,
            skills: Vec::new(),
        })
        .await
        .expect("team.bot_add");
    client
        .team_routine_add(&RoutineAddParams {
            team: "Desarrollo".to_owned(),
            bot: "Mara".to_owned(),
            name: "Weekly backup".to_owned(),
            prompt: "Back up the notes.".to_owned(),
            cadence: RoutineCadence::Daily,
            time: "09:00".to_owned(),
            weekday: None,
            month_day: None,
            run_date: None,
            zone: Some("Europe/Madrid".to_owned()),
        })
        .await
        .expect("team.routine_add");

    let both = list(&client, window(None)).await;
    let shown: Vec<(String, CalendarKind, i64, String, Option<String>)> = both
        .occurrences
        .iter()
        .map(|run| {
            (
                run.name.clone(),
                run.kind,
                run.at_ms,
                run.local_time.clone(),
                run.zone.clone(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        vec![
            (
                "Weekly backup".to_owned(),
                CalendarKind::Routine,
                JAN_1_2030 + 8 * HOUR_MS,
                "09:00".to_owned(),
                Some("Europe/Madrid".to_owned()),
            ),
            (
                "Morning PRs".to_owned(),
                CalendarKind::Automation,
                JAN_1_2030 + 9 * HOUR_MS,
                "09:00".to_owned(),
                Some("UTC".to_owned()),
            ),
        ]
    );
    assert!(both.occurrences.iter().all(|run| !run.done));

    let automations = list(&client, window(Some(CalendarKind::Automation))).await;
    let names: Vec<&str> = automations
        .occurrences
        .iter()
        .map(|run| run.name.as_str())
        .collect();
    assert_eq!(names, vec!["Morning PRs"]);

    let routines = list(&client, window(Some(CalendarKind::Routine))).await;
    let names: Vec<&str> = routines
        .occurrences
        .iter()
        .map(|run| run.name.as_str())
        .collect();
    assert_eq!(names, vec!["Weekly backup"]);
    stop(running).await;
}
