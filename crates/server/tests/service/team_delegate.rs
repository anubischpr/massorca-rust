//! `team.delegate` and `team.delegation_tree` over the wire (decision 0128). Only a bot's live agent run can delegate: a
//! call without a run, or with a token no live run holds, is refused and changes nothing. A task nobody delegated has an
//! empty tree.

use massorca_protocol::methods::tasks::{self, Task, TaskCreateParams, TaskRef};
use massorca_protocol::methods::teams::{
    self, DelegationList, DelegationTree, Team, TeamCreateParams, TeamDelegateParams,
    TeamDelegated, TeamRef,
};
use massorca_protocol::{capabilities, codes};

use super::common::{connect, remote_code, start, stop, temp_home};

fn delegate(session: Option<&str>, token: Option<&str>) -> TeamDelegateParams {
    TeamDelegateParams {
        bot: Some("mara".to_owned()),
        text: "Parse the headers".to_owned(),
        team: None,
        read_only: false,
        session: session.map(str::to_owned),
        agent_token: token.map(str::to_owned),
    }
}

#[tokio::test]
async fn a_delegation_outside_a_bot_run_is_refused_with_its_own_code() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let no_run = client
        .call::<_, TeamDelegated>(teams::DELEGATE, &delegate(None, None))
        .await
        .expect_err("no run");
    assert_eq!(remote_code(&no_run), codes::TEAM_DELEGATE_NOT_A_BOT);

    let token_only = client
        .call::<_, TeamDelegated>(teams::DELEGATE, &delegate(None, Some("token")))
        .await
        .expect_err("no session");
    assert_eq!(remote_code(&token_only), codes::TEAM_DELEGATE_NOT_A_BOT);

    stop(running).await;
}

#[tokio::test]
async fn a_run_token_no_live_run_holds_is_refused_and_creates_nothing() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let alpha: Team = client
        .call(
            teams::CREATE,
            &TeamCreateParams {
                name: "alpha".to_owned(),
                purpose: None,
                character: None,
                access: None,
                workspaces: None,
            },
        )
        .await
        .expect("team.create");
    let refused = client
        .call::<_, TeamDelegated>(
            teams::DELEGATE,
            &delegate(Some("no-such-session"), Some("token")),
        )
        .await
        .expect_err("no live run");
    assert_eq!(remote_code(&refused), codes::PERMISSION_DENIED);

    let list: massorca_protocol::methods::tasks::TaskList = client
        .call(
            tasks::LIST,
            &massorca_protocol::methods::tasks::TaskListParams::default(),
        )
        .await
        .expect("task.list");
    assert!(
        list.tasks.is_empty(),
        "a refused delegation creates no task in {}",
        alpha.name
    );

    stop(running).await;
}

#[tokio::test]
async fn a_task_nobody_delegated_has_an_empty_tree_and_an_unknown_task_is_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let team: Team = client
        .call(
            teams::CREATE,
            &TeamCreateParams {
                name: "alpha".to_owned(),
                purpose: None,
                character: None,
                access: None,
                workspaces: None,
            },
        )
        .await
        .expect("team.create");
    let task: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                team: team.id.clone(),
                title: "Lead the parser".to_owned(),
                body: None,
                bot: None,
                project: None,
                origin: None,
            },
        )
        .await
        .expect("task.create");

    let tree: DelegationTree = client
        .call(
            teams::DELEGATION_TREE,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("team.delegation_tree");
    assert_eq!(tree.task.id, task.id);
    assert!(tree.children.is_empty());

    let unknown = client
        .call::<_, DelegationTree>(
            teams::DELEGATION_TREE,
            &TaskRef {
                task: "no-such-task".to_owned(),
            },
        )
        .await
        .expect_err("unknown task");
    assert_eq!(remote_code(&unknown), codes::TASK_NOT_FOUND);

    stop(running).await;
}

#[tokio::test]
async fn a_team_without_delegations_lists_none_and_an_unknown_team_is_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let team: Team = client
        .call(
            teams::CREATE,
            &TeamCreateParams {
                name: "alpha".to_owned(),
                purpose: None,
                character: None,
                access: None,
                workspaces: None,
            },
        )
        .await
        .expect("team.create");

    let list: DelegationList = client
        .call(
            teams::DELEGATION_LIST,
            &TeamRef {
                team: team.id.clone(),
            },
        )
        .await
        .expect("team.delegation_list");
    assert!(list.links.is_empty());

    let unknown = client
        .call::<_, DelegationList>(
            teams::DELEGATION_LIST,
            &TeamRef {
                team: "no-such-team".to_owned(),
            },
        )
        .await
        .expect_err("unknown team");
    assert_eq!(remote_code(&unknown), codes::TEAM_NOT_FOUND);

    stop(running).await;
}

#[test]
fn the_delegation_capability_is_named_in_the_protocol() {
    assert_eq!(capabilities::TEAM_DELEGATE, "team-delegate/1");
}
