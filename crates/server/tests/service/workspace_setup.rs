//! Setup scripts on workspace create (decision 0086): the new worktree's `orca.yaml` setup runs in a service terminal in
//! the workspace with the workspace's variables; `no_setup` and a project without a script start no terminal.

use std::path::Path;
use std::time::{Duration, Instant};

use massorca_client::Client;
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams};
use massorca_protocol::methods::terminals::{self, Terminal, TerminalList, TerminalListParams};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCreateParams, WorkspaceSetup,
};

use super::common::{connect, start, stop, temp_home};
use super::git_fixture::git;

const OUTPUT: &str = "setup-out.txt";

/// A setup script that writes the workspace's variables, one per line, into `setup-out.txt` in its working directory.
fn env_script() -> &'static str {
    if cfg!(windows) {
        "echo %MASSORCA_WORKSPACE_PATH% >>setup-out.txt\n\
         echo %MASSORCA_WORKSPACE_NAME% >>setup-out.txt\n\
         echo %MASSORCA_ROOT_PATH% >>setup-out.txt\n\
         echo %MASSORCA_WORKSPACE_BRANCH% >>setup-out.txt\n\
         echo %ORCA_WORKTREE_PATH% >>setup-out.txt"
    } else {
        "echo \"$MASSORCA_WORKSPACE_PATH\" >> setup-out.txt\n\
         echo \"$MASSORCA_WORKSPACE_NAME\" >> setup-out.txt\n\
         echo \"$MASSORCA_ROOT_PATH\" >> setup-out.txt\n\
         echo \"$MASSORCA_WORKSPACE_BRANCH\" >> setup-out.txt\n\
         echo \"$ORCA_WORKTREE_PATH\" >> setup-out.txt"
    }
}

/// A repository whose committed `orca.yaml` holds `script` as `scripts.setup`, when given.
fn repo_with_setup(dir: &Path, script: Option<&str>) {
    std::fs::create_dir_all(dir).expect("create repository dir");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("write readme");
    git(dir, &["add", "README.md"]);
    if let Some(script) = script {
        let body: String = script.lines().map(|line| format!("    {line}\n")).collect();
        std::fs::write(
            dir.join("orca.yaml"),
            format!("scripts:\n  setup: |\n{body}"),
        )
        .expect("write orca.yaml");
        git(dir, &["add", "orca.yaml"]);
    }
    git(dir, &["commit", "-q", "-m", "initial"]);
}

async fn add_project(client: &Client, home: &Path, repo: &Path) -> Project {
    client
        .call(
            projects::ADD,
            &ProjectAddParams {
                path: repo.to_str().expect("utf-8").to_owned(),
                name: None,
                kind: None,
                icon: None,
                workspaces_root: Some(home.join("ws").to_str().expect("utf-8").to_owned()),
            },
        )
        .await
        .expect("add project")
}

async fn create(client: &Client, project: &str, name: &str, no_setup: bool) -> Workspace {
    client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                project: project.to_owned(),
                name: name.to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: None,
                no_setup,
                unique_name: false,
                name_generated: false,
            },
        )
        .await
        .expect("create")
}

async fn terminals_of(client: &Client, workspace: &str) -> Vec<Terminal> {
    let list: TerminalList = client
        .call(
            terminals::LIST,
            &TerminalListParams {
                workspace: Some(workspace.to_owned()),
            },
        )
        .await
        .expect("list terminals");
    list.terminals
}

async fn wait_for_exit(client: &Client, workspace: &str, terminal: &str) -> Terminal {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let found = terminals_of(client, workspace)
            .await
            .into_iter()
            .find(|candidate| candidate.id == terminal)
            .expect("the setup terminal is listed in its workspace");
        if found.exit.is_some() {
            return found;
        }
        assert!(Instant::now() < deadline, "the setup terminal never exited");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn a_setup_script_runs_in_a_listed_terminal_in_the_workspace_with_its_variables() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_setup(&repo, Some(env_script()));
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;

    let created = create(&client, &project.id, "feature", false).await;
    let Some(WorkspaceSetup::Started { terminal }) = created.setup.clone() else {
        panic!("setup did not start: {:?}", created.setup);
    };
    let exited = wait_for_exit(&client, &created.id, &terminal).await;
    assert_eq!(exited.title.as_deref(), Some("Setup"));
    assert_eq!(Path::new(&exited.cwd), Path::new(&created.path));

    let output = std::fs::read_to_string(Path::new(&created.path).join(OUTPUT))
        .expect("the script wrote its output in the workspace");
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert_eq!(
        lines,
        vec![
            created.path.as_str(),
            "feature",
            project.path.as_str(),
            created.branch.as_deref().expect("a branch"),
            created.path.as_str(),
        ]
    );
    stop(running).await;
}

#[tokio::test]
async fn a_project_without_a_setup_script_reports_no_setup_and_starts_no_terminal() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_setup(&repo, None);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;

    let created = create(&client, &project.id, "feature", false).await;
    assert_eq!(created.setup, None);
    assert!(terminals_of(&client, &created.id).await.is_empty());
    stop(running).await;
}

#[tokio::test]
async fn no_setup_skips_the_script_and_starts_no_terminal() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_setup(&repo, Some(env_script()));
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;

    let created = create(&client, &project.id, "feature", true).await;
    assert_eq!(created.setup, Some(WorkspaceSetup::Skipped));
    assert!(terminals_of(&client, &created.id).await.is_empty());
    assert!(!Path::new(&created.path).join(OUTPUT).exists());
    stop(running).await;
}

/// Git for Windows ships bash, and git runs these tests, so the `#!` runner always has its interpreter here.
#[cfg(windows)]
#[tokio::test]
async fn a_shebang_script_on_windows_runs_through_git_bash_with_posix_paths() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_setup(
        &repo,
        Some(
            "#!/usr/bin/env bash\necho \"$MASSORCA_WORKSPACE_NAME $BASH_VERSION\" > setup-out.txt\necho \"$MASSORCA_WORKSPACE_PATH\" >> setup-out.txt",
        ),
    );
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;

    let created = create(&client, &project.id, "feature", false).await;
    let Some(WorkspaceSetup::Started { terminal }) = created.setup.clone() else {
        panic!("setup did not start: {:?}", created.setup);
    };
    let exited = wait_for_exit(&client, &created.id, &terminal).await;
    assert!(
        exited.shell.to_ascii_lowercase().ends_with("bash.exe"),
        "{}",
        exited.shell
    );
    let output = std::fs::read_to_string(Path::new(&created.path).join(OUTPUT))
        .expect("the script wrote its output in the workspace");
    let mut lines = output.lines();
    let first = lines.next().expect("a first line");
    assert!(
        first.starts_with("feature ") && first.len() > "feature ".len(),
        "{first}"
    );
    let path = lines.next().expect("a second line");
    assert!(
        path.starts_with('/') && path.ends_with("/feature"),
        "{path}"
    );
    stop(running).await;
}

fn params(project: &str, name: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: name.to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
        unique_name: false,
        name_generated: false,
    }
}

/// The runner a worktree named `name` of `repo` gets: its admin folder under the main repository's git directory.
fn runner_of(repo: &Path, name: &str) -> std::path::PathBuf {
    let file = if cfg!(windows) {
        "setup-runner.cmd"
    } else {
        "setup-runner.sh"
    };
    repo.join(".git")
        .join("worktrees")
        .join(name)
        .join("massorca")
        .join(file)
}

#[tokio::test]
async fn a_repeated_create_starts_no_setup_and_leaves_the_runner_alone() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_setup(&repo, Some(env_script()));
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    let first_params = WorkspaceCreateParams {
        request_id: Some("create-once".to_owned()),
        ..params(&project.id, "feature")
    };

    let first: Workspace = client
        .call(workspaces::CREATE, &first_params)
        .await
        .expect("create");
    let Some(WorkspaceSetup::Started { terminal }) = first.setup.clone() else {
        panic!("setup did not start: {:?}", first.setup);
    };
    wait_for_exit(&client, &first.id, &terminal).await;
    let runner = runner_of(&repo, "feature");
    std::fs::write(&runner, "left by the first run").expect("mark the runner");

    let repeated: Workspace = client
        .call(workspaces::CREATE, &first_params)
        .await
        .expect("repeat");
    assert_eq!(repeated.id, first.id);
    assert_eq!(repeated.setup, None);
    assert_eq!(
        std::fs::read_to_string(&runner).expect("runner"),
        "left by the first run"
    );
    assert_eq!(terminals_of(&client, &first.id).await.len(), 1);
    stop(running).await;
}

/// cmd expands `%VAR%` before it reads `&`, so a branch like this would run a second command.
#[cfg(windows)]
#[tokio::test]
async fn a_cmd_runner_refuses_a_variable_cmd_would_reread() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_setup(&repo, Some(env_script()));
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;

    let created: Workspace = client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                branch: Some("x&y".to_owned()),
                ..params(&project.id, "feature")
            },
        )
        .await
        .expect("create");
    let Some(WorkspaceSetup::Failed { reason }) = created.setup.clone() else {
        panic!("setup was not refused: {:?}", created.setup);
    };
    assert!(reason.contains("MASSORCA_WORKSPACE_BRANCH"), "{reason}");
    assert!(terminals_of(&client, &created.id).await.is_empty());
    assert!(!runner_of(&repo, "feature").exists());
    stop(running).await;
}
