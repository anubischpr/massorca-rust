//! Real repositories and a real store for the tests. A fixture owns one temporary
//! directory, removed on drop, that holds the main repository, the store file and the
//! default workspaces root.

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use massorca_core_git::Git;
use massorca_core_store::Store;
use massorca_core_workspace::{DomainError, Workspaces};
use massorca_protocol::methods::projects::{Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use tempfile::TempDir;

pub struct Fixture {
    root: TempDir,
    #[allow(dead_code)] // used by the removal tests, which land later
    pub store: Store,
    pub workspaces: Arc<Workspaces>,
}

impl Fixture {
    /// A root holding a repository at `repo` (branch `main`, one commit) and an empty store.
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("create temp dir");
        init_repo(&root.path().join("repo"), "README.md", "hello\n");
        let store = Store::open(root.path().join("store.db")).expect("open store");
        let git = Git::discover().expect("git is on PATH");
        let workspaces = Arc::new(Workspaces::new(
            store.clone(),
            git,
            root.path().join("defaults"),
        ));
        Self {
            root,
            store,
            workspaces,
        }
    }

    /// The main repository.
    pub fn repo(&self) -> PathBuf {
        self.path("repo")
    }

    /// A path beside the main repository; nothing is created.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.path().join(relative)
    }

    /// Another repository beside the main one, with its own commit.
    #[allow(dead_code)] // used by the read tests, which land later
    pub fn second_repo(&self, name: &str) -> PathBuf {
        let repo = self.path(name);
        init_repo(&repo, "other.md", "other\n");
        repo
    }

    /// Registers the main repository as a git project named `repo`.
    pub fn repo_project(&self) -> Project {
        self.workspaces
            .add_project(project_add(&self.repo()))
            .expect("add repository project")
    }

    /// Registers `<root>/<name>`, holding `notes.txt`, as a folder project named `name`.
    pub fn folder_project(&self, name: &str) -> Project {
        let dir = self.path(name);
        std::fs::create_dir_all(&dir).expect("create folder");
        std::fs::write(dir.join("notes.txt"), "keep me").expect("write notes");
        let mut params = project_add(&dir);
        params.kind = Some(ProjectKind::Folder);
        self.workspaces
            .add_project(params)
            .expect("add folder project")
    }

    /// Creates a workspace named `name` in project `project` with default options.
    pub fn create(&self, project: &str, name: &str) -> Workspace {
        self.workspaces
            .create_workspace(create_params(project, name))
            .expect("create workspace")
    }
}

/// Creates `dir` as a repository on `main` with one commit containing `file`.
fn init_repo(dir: &Path, file: &str, contents: &str) {
    std::fs::create_dir_all(dir).expect("create repo dir");
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(dir, &["config", "core.autocrlf", "false"]);
    commit_file(dir, file, contents, "initial commit");
}

pub fn project_add(path: &Path) -> ProjectAddParams {
    ProjectAddParams {
        path: text(path),
        name: None,
        kind: None,
        workspaces_root: None,
    }
}

pub fn create_params(project: &str, name: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: name.to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        path: None,
        request_id: None,
    }
}

pub fn text(path: &Path) -> String {
    path.to_str().expect("temp paths are UTF-8").to_owned()
}

/// The canonical form of an existing path, as the domain stores it.
pub fn canonical(path: &Path) -> PathBuf {
    dunce::canonicalize(path).expect("path exists")
}

/// Asserts that `result` is a refusal with `code`.
pub fn expect_code<T: Debug>(result: Result<T, DomainError>, code: &str) {
    match result {
        Err(error) => assert_eq!(
            error.code(),
            code,
            "unexpected refusal: {}",
            error.message()
        ),
        Ok(value) => panic!("expected {code}, got success: {value:?}"),
    }
}

/// Runs git in `dir` with a fixed identity and returns its stdout; panics on failure.
pub fn run_git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git output is UTF-8")
}

/// Writes `file` in `dir` and commits it on the current branch.
pub fn commit_file(dir: &Path, file: &str, contents: &str, message: &str) {
    std::fs::write(dir.join(file), contents).expect("write file");
    run_git(dir, &["add", "--", file]);
    // A global hooks directory must not run during test commits.
    let hooks = format!(
        "core.hooksPath={}",
        dir.join(".git").join("hooks").display()
    );
    run_git(dir, &["-c", hooks.as_str(), "commit", "-q", "-m", message]);
}

/// Whether the local branch `branch` exists in `repo`.
pub fn branch_exists(repo: &Path, branch: &str) -> bool {
    Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .current_dir(repo)
        .status()
        .expect("spawn git")
        .success()
}

/// How many worktrees git lists for `repo`, the main worktree included.
pub fn worktree_count(repo: &Path) -> usize {
    run_git(repo, &["worktree", "list", "--porcelain"])
        .lines()
        .filter(|line| line.starts_with("worktree "))
        .count()
}

/// The sorted names of the entries directly inside `dir`.
pub fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("read entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}
