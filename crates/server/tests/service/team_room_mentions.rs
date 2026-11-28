//! Typed mentions of the Conference room over the wire (AUTO-13): `team.room_resolve` names what each mention of a line
//! reaches and refuses an unknown one or a file outside its project with a typed code; `team.room_mentions` suggests
//! what an `@` can name; and a posted line's mentions steer it: a team mention files the request in that team's Todo,
//! and a project or file mention links the project and gives the task the files. None of these lines starts an agent.

use std::path::Path;

use massorca_client::Client;
use massorca_protocol::methods::projects::ProjectKind;
use massorca_protocol::methods::tasks::TaskListParams;
use massorca_protocol::methods::team_room::{
    RoomMentionKind, TeamRoomListParams, TeamRoomMentionsParams, TeamRoomPostParams,
    TeamRoomResolveParams,
};
use massorca_protocol::methods::teams::{self, BotAddParams, Team, TeamProject};
use massorca_protocol::{capabilities, codes};

use super::common::{connect, remote_code, start, stop, temp_home};
use super::teams::{create_params, project_add, register_project, workspaces_switch};

async fn bot(client: &Client, team: &str, name: &str, lead: bool) -> String {
    client
        .team_bot_add(&BotAddParams {
            team: team.to_owned(),
            name: name.to_owned(),
            job: "Keeps the docs".to_owned(),
            kind: "claude".to_owned(),
            model: None,
            effort: None,
            lead,
            skills: Vec::new(),
        })
        .await
        .expect("team.bot_add")
        .id
}

/// The ids of what [`rooms`] made.
struct Ids {
    mara: String,
    iris: String,
    parser: String,
}

/// Team alpha (bots Mara and Bruno, the lead) with project Parser on `folder`, and team beta (its lead Iris).
async fn rooms(client: &Client, folder: &Path) -> Ids {
    std::fs::create_dir_all(folder.join("src")).expect("create src");
    std::fs::write(folder.join("src").join("lib.rs"), "pub fn parse() {}\n").expect("lib.rs");
    std::fs::write(folder.join("README.md"), "Parser\n").expect("readme");
    for name in ["alpha", "beta"] {
        client
            .call::<_, Team>(teams::CREATE, &create_params(name))
            .await
            .expect("team.create");
    }
    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(true))
        .await
        .expect("Workspaces on");
    let mara = bot(client, "alpha", "Mara", false).await;
    bot(client, "alpha", "Bruno", true).await;
    let iris = bot(client, "beta", "Iris", true).await;
    let source = register_project(client, folder, Some(ProjectKind::Folder)).await;
    let parser = client
        .call::<_, TeamProject>(teams::PROJECT_ADD, &project_add("Parser", Some(&source.id)))
        .await
        .expect("team.project_add")
        .id;
    Ids { mara, iris, parser }
}

fn resolve(text: &str) -> TeamRoomResolveParams {
    TeamRoomResolveParams {
        team: "alpha".to_owned(),
        text: text.to_owned(),
    }
}

fn held_post(text: &str) -> TeamRoomPostParams {
    TeamRoomPostParams {
        team: "alpha".to_owned(),
        text: text.to_owned(),
        bot: None,
        to: None,
        session: None,
        agent_token: None,
        start: Some(false),
        reply_to: None,
        ask: None,
        route: None,
    }
}

#[tokio::test]
async fn a_line_resolves_bots_teams_projects_and_files_and_refuses_unknown_ones() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    assert!(client.has_capability(capabilities::TEAM_ROOM_ROUTING));
    rooms(&client, &home.path().join("parser")).await;

    let resolved = client
        .team_room_resolve(&resolve(
            "@mara ask @team:Beta about @project:parser, see @file:src/lib.rs. Mail me@x.dev",
        ))
        .await
        .expect("team.room_resolve");
    let named: Vec<(RoomMentionKind, &str, &str)> = resolved
        .mentions
        .iter()
        .map(|mention| {
            (
                mention.kind,
                mention.label.as_str(),
                mention.insert.as_str(),
            )
        })
        .collect();
    assert_eq!(
        named,
        [
            (RoomMentionKind::Bot, "Mara", "@Mara"),
            (RoomMentionKind::Team, "beta", "@team:beta"),
            (RoomMentionKind::Project, "Parser", "@project:Parser"),
            (RoomMentionKind::File, "src/lib.rs", "@file:src/lib.rs"),
        ]
    );
    assert_eq!(resolved.mentions[3].project.as_deref(), Some("Parser"));

    for (text, code, mention) in [
        (
            "ask @team:gamma",
            codes::TEAM_ROOM_MENTION_UNKNOWN,
            "@team:gamma",
        ),
        (
            "see @project:nope",
            codes::TEAM_ROOM_MENTION_UNKNOWN,
            "@project:nope",
        ),
        (
            "see @file:src/main.rs",
            codes::TEAM_ROOM_MENTION_UNKNOWN,
            "@file:src/main.rs",
        ),
        (
            "see @file:../secrets.txt",
            codes::TEAM_ROOM_FILE_OUTSIDE,
            "@file:../secrets.txt",
        ),
        (
            "see @file:src/../../x",
            codes::TEAM_ROOM_FILE_OUTSIDE,
            "@file:src/../../x",
        ),
    ] {
        let refused = client
            .team_room_resolve(&resolve(text))
            .await
            .expect_err(text);
        assert_eq!(remote_code(&refused), code, "{text}");
        let massorca_client::ClientError::Remote(body) = &refused else {
            panic!("a remote refusal: {refused:?}");
        };
        assert_eq!(body.data.as_ref().unwrap()["mention"], mention, "{text}");
    }
    let outside = home.path().join("secrets.txt");
    std::fs::write(&outside, "no").expect("secret");
    let escape = home.path().join("parser").join("escape.txt");
    if make_link(&outside, &escape) {
        let refused = client
            .team_room_resolve(&resolve("see @file:escape.txt"))
            .await
            .expect_err("a link out of the project");
        assert_eq!(remote_code(&refused), codes::TEAM_ROOM_FILE_OUTSIDE);
    }
    stop(running).await;
}

/// A symbolic link at `link` to `target`; `false` where this host may not make one.
fn make_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}

#[tokio::test]
async fn an_at_suggests_bots_teams_projects_and_the_files_of_a_folder() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    rooms(&client, &home.path().join("parser")).await;
    let suggest = |query: &str| TeamRoomMentionsParams {
        team: "alpha".to_owned(),
        query: query.to_owned(),
        limit: None,
    };

    let all = client
        .team_room_mentions(&suggest(""))
        .await
        .expect("team.room_mentions");
    let inserts: Vec<&str> = all.mentions.iter().map(|m| m.insert.as_str()).collect();
    assert_eq!(
        inserts,
        ["@Mara", "@Bruno", "@team:beta", "@project:Parser", "@file:"]
    );
    let teams_only = client
        .team_room_mentions(&suggest("team:"))
        .await
        .expect("teams");
    assert_eq!(teams_only.mentions.len(), 1);
    assert_eq!(teams_only.mentions[0].kind, RoomMentionKind::Team);
    let bruno = client
        .team_room_mentions(&suggest("bru"))
        .await
        .expect("bru");
    assert_eq!(bruno.mentions[0].insert, "@Bruno");

    let root = client
        .team_room_mentions(&suggest("file:"))
        .await
        .expect("files");
    let files: Vec<&str> = root.mentions.iter().map(|m| m.insert.as_str()).collect();
    assert_eq!(files, ["@file:README.md", "@file:src/"]);
    let src = client
        .team_room_mentions(&suggest("file:src/l"))
        .await
        .expect("src");
    assert_eq!(src.mentions[0].insert, "@file:src/lib.rs");
    assert_eq!(src.mentions[0].project.as_deref(), Some("Parser"));
    let refused = client
        .team_room_mentions(&suggest("file:../"))
        .await
        .expect_err("no listing outside a project");
    assert_eq!(remote_code(&refused), codes::TEAM_ROOM_FILE_OUTSIDE);
    stop(running).await;
}

#[tokio::test]
async fn a_team_mention_sends_the_request_to_that_teams_todo() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let ids = rooms(&client, &home.path().join("parser")).await;

    let posted = client
        .team_room_post(&held_post("@team:beta redraw the parser diagram"))
        .await
        .expect("team.room_post");
    let [person, sent] = posted.messages.as_slice() else {
        panic!("the line and where it went: {posted:?}");
    };
    assert_eq!(person.text, "@team:beta redraw the parser diagram");
    assert_eq!(
        sent.text,
        "Sent to team beta, in its Todo: @team:beta redraw the parser diagram"
    );
    let task = sent.task.as_ref().expect("the task it made");
    let beta = client
        .task_list(&TaskListParams {
            team: Some("beta".to_owned()),
            ..TaskListParams::default()
        })
        .await
        .expect("task.list");
    assert_eq!(beta.tasks.len(), 1);
    assert_eq!(beta.tasks[0].id, task.id);
    assert_eq!(beta.tasks[0].bot_id.as_deref(), Some(ids.iris.as_str()));
    let alpha = client
        .task_list(&TaskListParams {
            team: Some("alpha".to_owned()),
            ..TaskListParams::default()
        })
        .await
        .expect("task.list");
    assert!(alpha.tasks.is_empty(), "the lead does not take it");
    stop(running).await;
}

#[tokio::test]
async fn project_and_file_mentions_link_the_project_and_give_the_task_the_files() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let ids = rooms(&client, &home.path().join("parser")).await;

    let posted = client
        .team_room_post(&held_post("@mara tidy @file:src/lib.rs in @project:Parser"))
        .await
        .expect("team.room_post");
    let task = posted.messages[1].task.as_ref().expect("a task");
    assert_eq!(task.bot_id.as_deref(), Some(ids.mara.as_str()));
    assert_eq!(task.project_id.as_deref(), Some(ids.parser.as_str()));
    assert!(
        task.body
            .ends_with("Mentioned in the room:\n- file src/lib.rs (in Parser)\n- project Parser"),
        "{}",
        task.body
    );

    let refused = client
        .team_room_post(&held_post("@mara tidy @file:src/none.rs"))
        .await
        .expect_err("an unknown file");
    assert_eq!(remote_code(&refused), codes::TEAM_ROOM_MENTION_UNKNOWN);
    let room = client
        .team_room_list(&TeamRoomListParams {
            team: "alpha".to_owned(),
            limit: None,
        })
        .await
        .expect("team.room_list");
    assert_eq!(room.messages.len(), 2, "a refused line stores nothing");
    stop(running).await;
}
