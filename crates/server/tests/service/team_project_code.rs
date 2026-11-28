//! Team project code sources (decision 0126): clearing one needs no Workspaces, a project links a Workspaces project or
//! records a new folder but never both, and `team.project_show` reads one project with no Workspaces needed.

use massorca_protocol::codes;
use massorca_protocol::methods::projects::ProjectKind;
use massorca_protocol::methods::teams::{
    self, CodeSource, Team, TeamProject, TeamProjectAddParams, TeamProjectShowParams,
    TeamProjectUpdateParams,
};

use super::common::{connect, remote_code, start, stop, temp_home};
use super::teams::{
    create_params, project_add, project_update_source, register_project, workspaces_switch,
};

fn show(project: &str) -> TeamProjectShowParams {
    TeamProjectShowParams {
        team: "alpha".to_owned(),
        project: project.to_owned(),
    }
}

fn code_update(source: CodeSource) -> TeamProjectUpdateParams {
    TeamProjectUpdateParams {
        code_source: Some(source),
        ..project_update_source("parser", None)
    }
}

#[tokio::test]
async fn clearing_a_code_source_needs_no_workspaces() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(true))
        .await
        .expect("Workspaces on");
    client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                code_source: Some(CodeSource::ExistingRepository),
                ..project_add("parser", None)
            },
        )
        .await
        .expect("a recorded code source");
    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(false))
        .await
        .expect("Workspaces off");

    let refused = client
        .call::<_, TeamProject>(
            teams::PROJECT_UPDATE,
            &code_update(CodeSource::NewFolderWithGit),
        )
        .await
        .expect_err("recording a code source needs Workspaces");
    assert_eq!(remote_code(&refused), codes::TEAM_WORKSPACES_OFF);

    let cleared = client
        .call::<_, TeamProject>(teams::PROJECT_UPDATE, &code_update(CodeSource::None))
        .await
        .expect("clearing stays open with Workspaces off");
    assert_eq!(cleared.code_source, None);
    stop(running).await;
}

#[tokio::test]
async fn a_project_links_a_workspaces_project_or_a_new_folder_but_not_both() {
    let home = temp_home();
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(true))
        .await
        .expect("Workspaces on");
    let linked_project = register_project(&client, &folder, Some(ProjectKind::Folder)).await;

    let both_on_add = client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                code_source: Some(CodeSource::NewFolderWithGit),
                ..project_add("parser", Some(linked_project.id.as_str()))
            },
        )
        .await
        .expect_err("one code source, not two");
    assert_eq!(remote_code(&both_on_add), codes::REQUEST_INVALID_PARAMS);

    client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                code_source: Some(CodeSource::NewFolderWithGit),
                ..project_add("parser", None)
            },
        )
        .await
        .expect("a new folder on its own");
    let link_on_folder = client
        .call::<_, TeamProject>(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some(linked_project.id.as_str())),
        )
        .await
        .expect_err("a link cannot join a new folder");
    assert_eq!(remote_code(&link_on_folder), codes::REQUEST_INVALID_PARAMS);
    let unchanged: TeamProject = client
        .call(teams::PROJECT_SHOW, &show("parser"))
        .await
        .expect("shown");
    assert_eq!(
        (unchanged.source_project, unchanged.code_source),
        (None, Some(CodeSource::NewFolderWithGit))
    );

    client
        .call::<_, TeamProject>(teams::PROJECT_UPDATE, &code_update(CodeSource::None))
        .await
        .expect("clearing the folder kind");
    let linked: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some(linked_project.id.as_str())),
        )
        .await
        .expect("a link once the folder kind is cleared");
    assert_eq!(
        linked.source_project.as_deref(),
        Some(linked_project.id.as_str())
    );
    let folder_on_link = client
        .call::<_, TeamProject>(
            teams::PROJECT_UPDATE,
            &code_update(CodeSource::NewFolderWithGit),
        )
        .await
        .expect_err("a new folder cannot join a link");
    assert_eq!(remote_code(&folder_on_link), codes::REQUEST_INVALID_PARAMS);
    let existing: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &code_update(CodeSource::ExistingRepository),
        )
        .await
        .expect("an existing repository is the link");
    assert_eq!(existing.code_source, Some(CodeSource::ExistingRepository));
    stop(running).await;
}

#[tokio::test]
async fn team_project_show_reads_one_project_by_id_or_name_without_workspaces() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(true))
        .await
        .expect("Workspaces on");
    let added: TeamProject = client
        .call(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                code_source: Some(CodeSource::ExistingRepository),
                ..project_add("parser", None)
            },
        )
        .await
        .expect("parser");
    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(false))
        .await
        .expect("Workspaces off");

    let by_name: TeamProject = client
        .call(teams::PROJECT_SHOW, &show("parser"))
        .await
        .expect("by name");
    let by_id: TeamProject = client
        .call(teams::PROJECT_SHOW, &show(&added.id))
        .await
        .expect("by id");
    assert_eq!(by_name, added);
    assert_eq!(by_id, added);
    assert_eq!(by_name.code_source, Some(CodeSource::ExistingRepository));

    let missing_project = client
        .call::<_, TeamProject>(teams::PROJECT_SHOW, &show("nope"))
        .await
        .expect_err("no such project");
    assert_eq!(remote_code(&missing_project), codes::TEAM_PROJECT_NOT_FOUND);
    let missing_team = client
        .call::<_, TeamProject>(
            teams::PROJECT_SHOW,
            &TeamProjectShowParams {
                team: "beta".to_owned(),
                project: "parser".to_owned(),
            },
        )
        .await
        .expect_err("no such team");
    assert_eq!(remote_code(&missing_team), codes::TEAM_NOT_FOUND);
    stop(running).await;
}
