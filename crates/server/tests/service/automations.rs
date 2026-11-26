//! Scheduled automation definitions over the wire (decision 0123): creation and its refusals, the creation key, the
//! list order, edits, enable and disable, removal, and what a removed project leaves behind.

use std::path::Path;

use massorca_client::Client;
use massorca_core_schedule::{ZoneSource, host_zone_used};
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{
    self, Automation, AutomationCadence, AutomationCreateParams, AutomationRef,
    AutomationUpdateParams,
};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectKind, ProjectRemoveParams,
};

use super::common::{connect, remote_code, start, stop, temp_home};

const DAY_MS: i64 = 86_400_000;
const NINE_AM_MS: i64 = 9 * 3_600_000;

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

/// Update params that change nothing until a field is set.
fn update_of(automation: &str) -> AutomationUpdateParams {
    AutomationUpdateParams {
        automation: automation.to_owned(),
        project: None,
        name: None,
        prompt: None,
        agent: None,
        cadence: None,
        time: None,
        weekday: None,
        cron: None,
        zone: None,
        grace_minutes: None,
    }
}

fn reference(automation: &str) -> AutomationRef {
    AutomationRef {
        automation: automation.to_owned(),
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
async fn an_automation_is_created_listed_shown_by_id_and_name_and_removed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = folder_project(&client, home.path(), "app").await;

    let created = client
        .automation_create(&create_params("app", "Morning PRs"))
        .await
        .expect("automation.create");
    assert_eq!(created.name, "Morning PRs");
    assert_eq!(created.project_id.as_deref(), Some(project.id.as_str()));
    assert_eq!(created.project_name.as_deref(), Some("app"));
    assert_eq!(created.cadence, AutomationCadence::Daily);
    assert_eq!(created.zone_used, None, "a named zone is not the host's");
    assert_eq!(created.grace_minutes, automations::DEFAULT_GRACE_MINUTES);
    assert!(created.enabled);
    let next = created
        .next_run_at_ms
        .expect("an enabled automation has a next run");
    assert_eq!(next % DAY_MS, NINE_AM_MS, "09:00 UTC");
    assert!(next > created.created_at_ms);
    assert_eq!(created.last_run_at_ms, None);

    let list = client.automation_list().await.expect("automation.list");
    assert_eq!(list.automations, vec![created.clone()]);
    let by_id = client
        .automation_show(&reference(&created.id))
        .await
        .expect("show by id");
    let by_name = client
        .automation_show(&reference("Morning PRs"))
        .await
        .expect("show by name");
    assert_eq!(by_id, created);
    assert_eq!(by_name, created);

    let removed = client
        .automation_remove(&reference(&created.id))
        .await
        .expect("automation.rm");
    assert_eq!(removed, created);
    assert!(
        client
            .automation_list()
            .await
            .expect("list after removal")
            .automations
            .is_empty()
    );
    stop(running).await;
}

#[tokio::test]
async fn a_repeated_create_with_the_same_creation_key_returns_the_first_automation() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;

    let mut first = create_params("app", "Morning PRs");
    first.creation_key = Some("client-key-1".to_owned());
    let made: Automation = client
        .automation_create(&first)
        .await
        .expect("first create");

    let mut repeat = create_params("app", "Renamed on retry");
    repeat.creation_key = Some("client-key-1".to_owned());
    let again = client
        .automation_create(&repeat)
        .await
        .expect("the repeat returns the first");
    assert_eq!(again, made);
    assert_eq!(
        client
            .automation_list()
            .await
            .expect("list")
            .automations
            .len(),
        1
    );
    stop(running).await;
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
async fn the_list_keeps_creation_order() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;

    for name in ["Beta", "Alpha", "Gamma"] {
        client
            .automation_create(&create_params("app", name))
            .await
            .expect("create");
    }
    let names: Vec<String> = client
        .automation_list()
        .await
        .expect("list")
        .automations
        .into_iter()
        .map(|automation| automation.name)
        .collect();
    assert_eq!(names, ["Beta", "Alpha", "Gamma"]);
    stop(running).await;
}

#[tokio::test]
async fn an_update_changes_only_the_fields_given_and_a_schedule_change_moves_the_next_run() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;
    let created = client
        .automation_create(&create_params("app", "Morning PRs"))
        .await
        .expect("create");

    let renamed = client
        .automation_update(&AutomationUpdateParams {
            name: Some("Renamed".to_owned()),
            ..update_of(&created.id)
        })
        .await
        .expect("rename");
    assert_eq!(renamed.name, "Renamed");
    assert_eq!(renamed.time, created.time);
    assert_eq!(renamed.next_run_at_ms, created.next_run_at_ms);

    let moved = client
        .automation_update(&AutomationUpdateParams {
            time: Some("10:30".to_owned()),
            ..update_of(&created.id)
        })
        .await
        .expect("move the time");
    assert_eq!(moved.time, "10:30");
    assert_eq!(
        moved.next_run_at_ms.expect("still enabled") % DAY_MS,
        10 * 3_600_000 + 30 * 60_000
    );
    stop(running).await;
}

#[tokio::test]
async fn disable_clears_the_next_run_and_enable_schedules_one_from_now() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;
    let created = client
        .automation_create(&create_params("app", "Morning PRs"))
        .await
        .expect("create");

    let disabled = client
        .automation_disable(&reference(&created.id))
        .await
        .expect("disable");
    assert!(!disabled.enabled);
    assert_eq!(disabled.next_run_at_ms, None);

    let enabled = client
        .automation_enable(&reference(&created.id))
        .await
        .expect("enable");
    assert!(enabled.enabled);
    let next = enabled.next_run_at_ms.expect("enabled again");
    assert!(next > enabled.updated_at_ms, "the next run is after now");
    assert_eq!(next % DAY_MS, NINE_AM_MS);
    stop(running).await;
}

#[tokio::test]
async fn removing_a_project_leaves_its_automation_without_a_project() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;
    let created = client
        .automation_create(&create_params("app", "Morning PRs"))
        .await
        .expect("create");

    let _: Project = client
        .call(
            projects::REMOVE,
            &ProjectRemoveParams {
                project: "app".to_owned(),
                forget_workspaces: false,
            },
        )
        .await
        .expect("project.remove");

    let orphan = client
        .automation_show(&reference(&created.id))
        .await
        .expect("the automation remains");
    assert_eq!(orphan.project_id, None);
    assert_eq!(orphan.project_name, None);
    stop(running).await;
}

#[tokio::test]
async fn a_name_shared_by_two_automations_is_refused_until_one_is_named_by_id() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    folder_project(&client, home.path(), "app").await;
    let first = client
        .automation_create(&create_params("app", "Twin"))
        .await
        .expect("first");
    client
        .automation_create(&create_params("app", "Twin"))
        .await
        .expect("second");

    let error = client
        .automation_show(&reference("Twin"))
        .await
        .expect_err("the name is shared");
    assert_eq!(remote_code(&error), codes::REQUEST_INVALID_PARAMS);
    let shown = client
        .automation_show(&reference(&first.id))
        .await
        .expect("by id");
    assert_eq!(shown, first);
    stop(running).await;
}

#[tokio::test]
async fn an_unknown_automation_is_not_found_by_every_method() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let error = client
        .automation_show(&reference("missing"))
        .await
        .expect_err("show");
    assert_eq!(remote_code(&error), codes::AUTOMATION_NOT_FOUND);
    let error = client
        .automation_update(&AutomationUpdateParams {
            name: Some("x".to_owned()),
            ..update_of("missing")
        })
        .await
        .expect_err("update");
    assert_eq!(remote_code(&error), codes::AUTOMATION_NOT_FOUND);
    let error = client
        .automation_remove(&reference("missing"))
        .await
        .expect_err("remove");
    assert_eq!(remote_code(&error), codes::AUTOMATION_NOT_FOUND);
    let error = client
        .automation_enable(&reference("missing"))
        .await
        .expect_err("enable");
    assert_eq!(remote_code(&error), codes::AUTOMATION_NOT_FOUND);
    let error = client
        .automation_disable(&reference("missing"))
        .await
        .expect_err("disable");
    assert_eq!(remote_code(&error), codes::AUTOMATION_NOT_FOUND);
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
