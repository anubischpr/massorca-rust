//! Scheduled automation definitions over the wire (decision 0123): creation and its refusals, and the host zone an
//! automation without a zone runs in.

use std::path::Path;

use massorca_client::Client;
use massorca_core_schedule::{ZoneSource, host_zone_used};
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{self, AutomationCadence, AutomationCreateParams};
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};

use super::common::{connect, remote_code, start, stop, temp_home};

fn create_params(project: &str, name: &str) -> AutomationCreateParams {
    AutomationCreateParams {
        project: project.to_owned(),
        name: name.to_owned(),
        prompt: "Summarise the open pull requests".to_owned(),
        agent: "claude".to_owned(),
        cadence: AutomationCadence::Daily,
        time: "09:00".to_owned(),
        weekday: None,
        cron: None,
        zone: Some("UTC".to_owned()),
        grace_minutes: None,
        creation_key: None,
    }
}

/// A folder project named `name`, at a folder of the same name inside `home`.
async fn folder_project(client: &Client, home: &Path, name: &str) -> Project {
    let folder = home.join(name);
    std::fs::create_dir_all(&folder).expect("create the folder");
    let params = ProjectAddParams {
        icon: None,
        path: folder.to_str().expect("utf-8 path").to_owned(),
        name: Some(name.to_owned()),
        kind: Some(ProjectKind::Folder),
        workspaces_root: None,
    };
    client
        .call(projects::ADD, &params)
        .await
        .expect("project.add")
}

#[tokio::test]
async fn an_unknown_agent_kind_or_project_is_refused_with_its_code() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;

    let mut unknown_agent = create_params("app", "Audit");
    unknown_agent.agent = "no-such-agent".to_owned();
    let error = client
        .automation_create(&unknown_agent)
        .await
        .expect_err("an unknown agent kind is refused");
    assert_eq!(remote_code(&error), codes::AGENT_KIND_NOT_FOUND);

    let error = client
        .automation_create(&create_params("missing", "Audit"))
        .await
        .expect_err("an unknown project is refused");
    assert_eq!(remote_code(&error), codes::PROJECT_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn a_cron_that_can_never_occur_and_an_unknown_zone_are_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;

    let mut impossible = create_params("app", "Impossible");
    impossible.cadence = AutomationCadence::Cron;
    impossible.cron = Some("0 0 30 2 *".to_owned());
    let error = client
        .automation_create(&impossible)
        .await
        .expect_err("30 February never occurs");
    assert_eq!(remote_code(&error), codes::AUTOMATION_SCHEDULE_INVALID);

    let mut unknown_zone = create_params("app", "Zone");
    unknown_zone.zone = Some("Mars/Olympus".to_owned());
    let error = client
        .automation_create(&unknown_zone)
        .await
        .expect_err("an unknown zone is refused");
    assert_eq!(remote_code(&error), codes::AUTOMATION_ZONE_INVALID);
    stop(running).await;
}

#[tokio::test]
async fn a_weekly_without_a_weekday_and_a_grace_over_one_week_are_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;

    let mut weekly = create_params("app", "Weekly");
    weekly.cadence = AutomationCadence::Weekly;
    let error = client
        .automation_create(&weekly)
        .await
        .expect_err("a weekly automation needs a weekday");
    assert_eq!(remote_code(&error), codes::AUTOMATION_SCHEDULE_INVALID);

    let mut long_grace = create_params("app", "Grace");
    long_grace.grace_minutes = Some(automations::MAX_GRACE_MINUTES + 1);
    let error = client
        .automation_create(&long_grace)
        .await
        .expect_err("a grace over one week is refused");
    assert_eq!(remote_code(&error), codes::AUTOMATION_SCHEDULE_INVALID);

    let mut longest = create_params("app", "Week");
    longest.grace_minutes = Some(automations::MAX_GRACE_MINUTES);
    let kept = client
        .automation_create(&longest)
        .await
        .expect("one week is allowed");
    assert_eq!(kept.grace_minutes, automations::MAX_GRACE_MINUTES);
    stop(running).await;
}

#[tokio::test]
async fn an_automation_without_a_zone_names_the_host_zone_it_runs_in() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;

    let mut no_zone = create_params("app", "Host zone");
    no_zone.zone = None;
    let (host, source) = host_zone_used();
    match (host, source) {
        (_, ZoneSource::HostUnknownUsedUtc) => {
            let error = client
                .automation_create(&no_zone)
                .await
                .expect_err("a host zone that cannot be read needs a zone");
            assert_eq!(remote_code(&error), codes::AUTOMATION_ZONE_INVALID);
        }
        (Some(name), _) => {
            let made = client
                .automation_create(&no_zone)
                .await
                .expect("create without a zone");
            assert_eq!(made.zone, None);
            assert_eq!(made.zone_used.as_deref(), Some(name.as_str()));
        }
        (None, _) => {}
    }
    stop(running).await;
}
