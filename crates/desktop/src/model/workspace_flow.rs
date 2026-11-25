//! The create-workspace form and the remove-workspace flow as plain state, free of GPUI
//! (decision 0028). The views draw them; the service answers decide how they move.

use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::workspaces::{
    Workspace, WorkspaceCreateParams, WorkspaceKind, WorkspaceRemoveParams,
};

use crate::model::start_page::GIT_LATER;
use crate::service::Refusal;

/// The last row of every project menu (A95).
pub const NO_PROJECT_LABEL: &str = "No project";

/// The confirm button of every destructive delete dialog (D-WS-16).
pub const DELETE_ACTION: &str = "Delete workspace";

/// Helper under the "Keep the branch" choice, which stays off until the service takes it.
pub const KEEP_BRANCH_LATER: &str = "Choosing to keep the branch comes later.";

/// Why a row of the workspace menu is off until the layer it needs is built.
pub const RENAME_LATER: &str = "Renaming comes later.";
pub const EDIT_LATER: &str = "Editing workspaces comes later.";
pub const TOOLS_LATER: &str = "Opening in other tools comes later.";
pub const WORK_PANEL_LATER: &str = "The Work panel comes later.";
/// Why a Git row is off in a folder that is not a Git repository (D-WS-14).
pub const NEEDS_GIT: &str = "Needs Git";
/// Why "Copy branch name" is off for a workspace with no branch.
pub const NO_BRANCH: &str = "This workspace has no branch.";

/// What a row of the project menu stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectChoice {
    Project(String),
    /// A conversation with no project and no worktree (A95).
    NoProject,
}

/// The glyph of a project menu row. The view maps each to a Lucide icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectIcon {
    Folder,
    /// A struck-through folder: "No project" (A95).
    FolderStruck,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectMenuRow {
    pub label: String,
    pub icon: ProjectIcon,
    pub choice: ProjectChoice,
}

/// The rows of the project menu: every project, then "No project" (A95).
pub fn project_menu_rows(projects: &[Project]) -> Vec<ProjectMenuRow> {
    let mut rows: Vec<ProjectMenuRow> = projects
        .iter()
        .map(|project| ProjectMenuRow {
            label: project.name.clone(),
            icon: ProjectIcon::Folder,
            choice: ProjectChoice::Project(project.id.clone()),
        })
        .collect();
    rows.push(ProjectMenuRow {
        label: NO_PROJECT_LABEL.to_owned(),
        icon: ProjectIcon::FolderStruck,
        choice: ProjectChoice::NoProject,
    });
    rows
}

/// The "New workspace" form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateForm {
    pub project: Option<ProjectChoice>,
    pub name: String,
    pub branch: String,
    pub base: String,
    pub existing_branch: bool,
    pub busy: bool,
    /// Shown under the name field: the service found the name taken or unusable.
    pub name_error: Option<String>,
    /// Shown above the footer: any other refusal.
    pub error: Option<String>,
}

impl CreateForm {
    /// An empty form. With exactly one project, it is already chosen.
    pub fn new(projects: &[Project]) -> Self {
        let project = match projects {
            [only] => Some(ProjectChoice::Project(only.id.clone())),
            _ => None,
        };
        Self {
            project,
            name: String::new(),
            branch: String::new(),
            base: String::new(),
            existing_branch: false,
            busy: false,
            name_error: None,
            error: None,
        }
    }

    /// The project button's text.
    pub fn project_label(&self, projects: &[Project]) -> String {
        match &self.project {
            None => "Choose a project".to_owned(),
            Some(ProjectChoice::NoProject) => NO_PROJECT_LABEL.to_owned(),
            Some(ProjectChoice::Project(id)) => projects
                .iter()
                .find(|project| &project.id == id)
                .map_or_else(|| "Choose a project".to_owned(), |p| p.name.clone()),
        }
    }

    /// What "No project" means here: it is off until the service can start an agent without a
    /// workspace, as the start page's menu says (A95).
    pub fn no_project_note(&self) -> Option<&'static str> {
        matches!(self.project, Some(ProjectChoice::NoProject))
            .then_some("No project needs a newer service. Choose a project to create a workspace.")
    }

    /// Asked of a branch to reuse when none is named.
    pub fn branch_error(&self) -> Option<&'static str> {
        (self.existing_branch && self.branch.trim().is_empty())
            .then_some("Name the branch to reuse.")
    }

    pub fn can_submit(&self) -> bool {
        !self.busy
            && !self.name.trim().is_empty()
            && matches!(self.project, Some(ProjectChoice::Project(_)))
            && self.branch_error().is_none()
    }

    /// The call this form asks for, or `None` while it cannot be submitted.
    pub fn params(&self) -> Option<WorkspaceCreateParams> {
        let Some(ProjectChoice::Project(project)) = &self.project else {
            return None;
        };
        if !self.can_submit() {
            return None;
        }
        Some(WorkspaceCreateParams {
            project: project.clone(),
            name: self.name.trim().to_owned(),
            branch: non_empty(&self.branch),
            base: non_empty(&self.base),
            existing_branch: self.existing_branch,
            refresh_base: false,
            path: None,
            request_id: None,
            no_setup: false,
        })
    }

    /// The call went out: busy until the service answers.
    pub fn begin(&mut self) {
        self.busy = true;
        self.name_error = None;
        self.error = None;
    }

    /// The service said no. A name problem sits under the name; anything else above the footer.
    pub fn refused(&mut self, refusal: &Refusal) {
        self.busy = false;
        match refusal {
            Refusal::NameProblem { message } => self.name_error = Some(message.clone()),
            other => self.error = Some(other.message()),
        }
    }
}

fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// Where the remove flow stands: which dialog it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoveStage {
    Confirm,
    /// `workspace.dirty`: asks to discard changes.
    Dirty,
    /// `workspace.ignored_files`: its own waiver.
    IgnoredFiles,
    /// `workspace.orphan`: asks to forget the row.
    Orphan,
    /// Nothing to waive: locked, the main worktree, or a waiver that was already given.
    Refused {
        message: String,
    },
}

/// The waivers the user confirmed so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Waivers {
    pub force: bool,
    pub discard_ignored: bool,
    pub forget_orphan: bool,
}

/// Which style of dialog a stage uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Destructive,
    Confirm,
    Error,
}

/// The words of one remove dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogCopy {
    pub kind: DialogKind,
    pub title: String,
    pub body: String,
    pub loss: Vec<String>,
    pub action: &'static str,
}

/// Removing one workspace: confirm, then one waiver dialog per refusal, each retrying the same
/// call with exactly one more flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveFlow {
    pub workspace_id: String,
    pub workspace_name: String,
    stage: RemoveStage,
    waivers: Waivers,
    busy: bool,
    error: Option<String>,
}

impl RemoveFlow {
    pub fn new(workspace_id: impl Into<String>, workspace_name: impl Into<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            workspace_name: workspace_name.into(),
            stage: RemoveStage::Confirm,
            waivers: Waivers::default(),
            busy: false,
            error: None,
        }
    }

    pub fn stage(&self) -> &RemoveStage {
        &self.stage
    }

    pub fn waivers(&self) -> Waivers {
        self.waivers
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The call as it stands: the workspace and the waivers given so far.
    pub fn params(&self) -> WorkspaceRemoveParams {
        WorkspaceRemoveParams {
            workspace: self.workspace_id.clone(),
            force: self.waivers.force,
            discard_ignored: self.waivers.discard_ignored,
            forget_orphan: self.waivers.forget_orphan,
        }
    }

    /// The primary action: gives the waiver this stage asks for and goes busy. False when the
    /// stage has nothing to run (a plain refusal) or a call is already out.
    pub fn confirm(&mut self) -> bool {
        if self.busy {
            return false;
        }
        match self.stage {
            RemoveStage::Confirm => {}
            RemoveStage::Dirty => self.waivers.force = true,
            RemoveStage::IgnoredFiles => self.waivers.discard_ignored = true,
            RemoveStage::Orphan => self.waivers.forget_orphan = true,
            RemoveStage::Refused { .. } => return false,
        }
        self.busy = true;
        self.error = None;
        true
    }

    /// The service answered. Returns true when the workspace was removed and the flow is over.
    pub fn answered(&mut self, result: Result<(), Refusal>) -> bool {
        self.busy = false;
        let Err(refusal) = result else {
            return true;
        };
        let next = match &refusal {
            Refusal::Dirty if !self.waivers.force => RemoveStage::Dirty,
            Refusal::IgnoredFiles if !self.waivers.discard_ignored => RemoveStage::IgnoredFiles,
            Refusal::Orphan if !self.waivers.forget_orphan => RemoveStage::Orphan,
            // A waiver already given and still refused, or nothing to waive: say so plainly.
            Refusal::Dirty
            | Refusal::IgnoredFiles
            | Refusal::Orphan
            | Refusal::Locked { .. }
            | Refusal::MainWorktree { .. } => RemoveStage::Refused {
                message: refusal.message(),
            },
            // The call itself failed; the dialog stays and shows why.
            Refusal::NameProblem { .. }
            | Refusal::PathProblem { .. }
            | Refusal::Failed { .. }
            | Refusal::GitRefused { .. }
            | Refusal::Unavailable { .. } => {
                self.error = Some(refusal.message());
                return false;
            }
        };
        self.stage = next;
        false
    }

    /// What the current dialog says.
    pub fn copy(&self) -> DialogCopy {
        let name = &self.workspace_name;
        match &self.stage {
            RemoveStage::Confirm => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete workspace {name}?"),
                body: "This removes the worktree folder from This PC.".to_owned(),
                loss: Vec::new(),
                action: DELETE_ACTION,
            },
            RemoveStage::Dirty => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete workspace {name}?"),
                body: "This workspace has changes that are not committed.".to_owned(),
                loss: vec!["Uncommitted and untracked changes are deleted.".to_owned()],
                action: DELETE_ACTION,
            },
            RemoveStage::IgnoredFiles => DialogCopy {
                kind: DialogKind::Destructive,
                title: format!("Delete workspace {name}?"),
                body: "This workspace holds files that git ignores.".to_owned(),
                loss: vec![
                    "Ignored files, such as build output and local settings, are deleted."
                        .to_owned(),
                ],
                action: DELETE_ACTION,
            },
            RemoveStage::Orphan => DialogCopy {
                kind: DialogKind::Confirm,
                title: format!("Forget {name}?"),
                body: "Its folder is not a git worktree any more. MassOrca forgets the workspace \
                       and leaves the folder where it is."
                    .to_owned(),
                loss: Vec::new(),
                action: "Forget workspace",
            },
            RemoveStage::Refused { message } => DialogCopy {
                kind: DialogKind::Error,
                title: format!("Cannot remove {name}"),
                body: message.clone(),
                loss: Vec::new(),
                action: "Close",
            },
        }
    }
}

/// The rows of a workspace's menu (D-WS-02). The view maps each row to its icon and runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceMenuId {
    NewSession,
    OpenIn,
    OpenFileManager,
    OpenTerminal,
    OpenEditor,
    OpenFolder,
    Rename,
    EditWorkspace,
    SourceControl,
    Developer,
    CopyPath,
    CopyBranch,
    RevealInWorkPanel,
    Delete,
}

/// One entry of a workspace's menu. `off` is why an item cannot be chosen yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceMenuEntry {
    Item {
        id: WorkspaceMenuId,
        label: &'static str,
        off: Option<&'static str>,
    },
    /// A row that opens its items as a submenu of one level.
    Submenu {
        id: WorkspaceMenuId,
        label: &'static str,
        items: Vec<WorkspaceMenuEntry>,
    },
    Divider,
}

fn item(id: WorkspaceMenuId, label: &'static str, off: Option<&'static str>) -> WorkspaceMenuEntry {
    WorkspaceMenuEntry::Item { id, label, off }
}

/// The menu of one workspace, top to bottom (D-WS-02). A row whose layer is not built yet is off
/// with its reason; Git rows say so in a folder that has no Git.
pub fn workspace_menu(workspace: &Workspace) -> Vec<WorkspaceMenuEntry> {
    let folder = workspace.kind == WorkspaceKind::Folder;
    let git_off = if folder { NEEDS_GIT } else { GIT_LATER };
    let branch_off = if folder {
        Some(NEEDS_GIT)
    } else if workspace.branch.is_none() {
        Some(NO_BRANCH)
    } else {
        None
    };
    vec![
        item(WorkspaceMenuId::NewSession, "New session", None),
        WorkspaceMenuEntry::Submenu {
            id: WorkspaceMenuId::OpenIn,
            label: "Open in",
            items: vec![
                item(
                    WorkspaceMenuId::OpenFileManager,
                    "File manager",
                    Some(TOOLS_LATER),
                ),
                item(WorkspaceMenuId::OpenTerminal, "Terminal", Some(TOOLS_LATER)),
                item(WorkspaceMenuId::OpenEditor, "Editor", Some(TOOLS_LATER)),
            ],
        },
        item(WorkspaceMenuId::Rename, "Rename", Some(RENAME_LATER)),
        item(
            WorkspaceMenuId::EditWorkspace,
            "Edit workspace",
            Some(EDIT_LATER),
        ),
        item(
            WorkspaceMenuId::SourceControl,
            "Open in Source control",
            Some(git_off),
        ),
        WorkspaceMenuEntry::Submenu {
            id: WorkspaceMenuId::Developer,
            label: "Developer",
            items: vec![
                item(WorkspaceMenuId::CopyPath, "Copy path", None),
                item(WorkspaceMenuId::CopyBranch, "Copy branch name", branch_off),
                item(
                    WorkspaceMenuId::RevealInWorkPanel,
                    "Reveal in Work panel",
                    Some(WORK_PANEL_LATER),
                ),
            ],
        },
        WorkspaceMenuEntry::Divider,
        item(WorkspaceMenuId::Delete, "Delete workspace", None),
    ]
}

/// The menu of a session's row (A97): Copy path, Open folder and Reveal in Work panel. A row whose layer
/// is not built yet is off with its reason.
pub fn session_menu() -> Vec<WorkspaceMenuEntry> {
    vec![
        item(WorkspaceMenuId::CopyPath, "Copy path", None),
        item(
            WorkspaceMenuId::OpenFolder,
            "Open folder",
            Some(TOOLS_LATER),
        ),
        item(
            WorkspaceMenuId::RevealInWorkPanel,
            "Reveal in Work panel",
            Some(WORK_PANEL_LATER),
        ),
    ]
}

/// The item the user chose, when it can run: a top-level row, or a row of the submenu it opened.
pub fn chosen_workspace_item(
    entries: &[WorkspaceMenuEntry],
    row: usize,
    child: Option<usize>,
) -> Option<WorkspaceMenuId> {
    match (entries.get(row)?, child) {
        (WorkspaceMenuEntry::Item { id, off: None, .. }, None) => Some(*id),
        (WorkspaceMenuEntry::Submenu { items, .. }, Some(child)) => match items.get(child)? {
            WorkspaceMenuEntry::Item { id, off: None, .. } => Some(*id),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::{project, workspace};

    fn two_projects() -> Vec<Project> {
        vec![project("p1", "alpha"), project("p2", "beta")]
    }

    #[test]
    fn the_project_menu_ends_with_no_project_and_a_struck_folder() {
        let rows = project_menu_rows(&two_projects());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].icon, ProjectIcon::Folder);
        let last = rows.last().expect("a last row");
        assert_eq!(last.label, "No project");
        assert_eq!(last.icon, ProjectIcon::FolderStruck);
        assert_eq!(last.choice, ProjectChoice::NoProject);
        // With no projects the menu still offers it.
        assert_eq!(project_menu_rows(&[]).len(), 1);
    }

    #[test]
    fn a_lone_project_is_chosen_for_you() {
        assert_eq!(
            CreateForm::new(&[project("p1", "alpha")]).project,
            Some(ProjectChoice::Project("p1".into()))
        );
        assert_eq!(CreateForm::new(&two_projects()).project, None);
    }

    #[test]
    fn create_needs_a_project_and_a_name() {
        let projects = two_projects();
        let mut form = CreateForm::new(&projects);
        assert!(!form.can_submit());
        form.project = Some(ProjectChoice::Project("p2".into()));
        assert!(!form.can_submit(), "no name yet");
        form.name = "  fix-login  ".into();
        assert!(form.can_submit());
        let params = form.params().expect("params");
        assert_eq!(params.project, "p2");
        assert_eq!(params.name, "fix-login");
        assert_eq!(params.branch, None);
        assert_eq!(params.base, None);
    }

    #[test]
    fn branch_and_base_are_sent_only_when_typed() {
        let mut form = CreateForm::new(&[project("p1", "alpha")]);
        form.name = "w".into();
        form.branch = " feat/w ".into();
        form.base = "origin/main".into();
        let params = form.params().expect("params");
        assert_eq!(params.branch.as_deref(), Some("feat/w"));
        assert_eq!(params.base.as_deref(), Some("origin/main"));
        assert!(!params.existing_branch);
    }

    #[test]
    fn reusing_a_branch_needs_its_name() {
        let mut form = CreateForm::new(&[project("p1", "alpha")]);
        form.name = "w".into();
        form.existing_branch = true;
        assert_eq!(form.branch_error(), Some("Name the branch to reuse."));
        assert!(!form.can_submit());
        form.branch = "feat/w".into();
        assert!(form.can_submit());
        assert!(form.params().expect("params").existing_branch);
    }

    #[test]
    fn no_project_cannot_create_and_says_why() {
        let mut form = CreateForm::new(&two_projects());
        form.name = "w".into();
        form.project = Some(ProjectChoice::NoProject);
        assert!(!form.can_submit());
        assert_eq!(form.params(), None);
        assert_eq!(
            form.no_project_note(),
            Some("No project needs a newer service. Choose a project to create a workspace.")
        );
        assert_eq!(form.project_label(&two_projects()), "No project");
    }

    #[test]
    fn a_name_refusal_sits_under_the_name_and_others_above_the_footer() {
        let mut form = CreateForm::new(&[project("p1", "alpha")]);
        form.name = "w".into();
        form.begin();
        assert!(
            form.busy && !form.can_submit(),
            "busy blocks a second submit"
        );
        form.refused(&Refusal::NameProblem {
            message: "That name is taken.".into(),
        });
        assert!(!form.busy);
        assert_eq!(form.name_error.as_deref(), Some("That name is taken."));
        form.begin();
        assert_eq!(form.name_error, None, "a new attempt clears the old error");
        form.refused(&Refusal::unavailable("The service is not connected."));
        assert_eq!(form.error.as_deref(), Some("The service is not connected."));
    }

    fn flow() -> RemoveFlow {
        RemoveFlow::new("w1", "fix-login")
    }

    #[test]
    fn a_dirty_refusal_asks_to_discard_and_retries_with_force_only() {
        let mut flow = flow();
        assert!(flow.confirm());
        assert!(!flow.params().force, "first call carries no flag");
        assert!(!flow.answered(Err(Refusal::Dirty)));
        assert_eq!(flow.stage(), &RemoveStage::Dirty);
        assert!(flow.confirm());
        let params = flow.params();
        assert!(params.force && !params.discard_ignored && !params.forget_orphan);
        assert!(flow.answered(Ok(())));
    }

    #[test]
    fn ignored_files_have_their_own_waiver_after_the_dirty_one() {
        let mut flow = flow();
        flow.confirm();
        flow.answered(Err(Refusal::Dirty));
        flow.confirm();
        flow.answered(Err(Refusal::IgnoredFiles));
        assert_eq!(flow.stage(), &RemoveStage::IgnoredFiles);
        assert!(flow.confirm());
        let params = flow.params();
        assert!(params.force && params.discard_ignored && !params.forget_orphan);
    }

    #[test]
    fn an_orphan_refusal_offers_to_forget_with_one_flag() {
        let mut flow = flow();
        flow.confirm();
        flow.answered(Err(Refusal::Orphan));
        assert_eq!(flow.stage(), &RemoveStage::Orphan);
        assert_eq!(flow.copy().kind, DialogKind::Confirm);
        flow.confirm();
        let params = flow.params();
        assert!(params.forget_orphan && !params.force && !params.discard_ignored);
    }

    #[test]
    fn locked_and_main_worktree_offer_nothing_to_waive() {
        for refusal in [
            Refusal::Locked {
                message: "It is locked.".into(),
            },
            Refusal::MainWorktree {
                message: "It is the main checkout.".into(),
            },
        ] {
            let mut flow = flow();
            flow.confirm();
            flow.answered(Err(refusal.clone()));
            assert_eq!(
                flow.stage(),
                &RemoveStage::Refused {
                    message: refusal.message()
                }
            );
            assert!(!flow.confirm(), "a plain refusal runs nothing");
            assert_eq!(flow.copy().kind, DialogKind::Error);
        }
    }

    #[test]
    fn a_waiver_that_is_refused_again_does_not_loop() {
        let mut flow = flow();
        flow.confirm();
        flow.answered(Err(Refusal::Dirty));
        flow.confirm();
        flow.answered(Err(Refusal::Dirty));
        assert!(matches!(flow.stage(), RemoveStage::Refused { .. }));
    }

    #[test]
    fn a_failed_call_keeps_the_dialog_and_shows_why() {
        let mut flow = flow();
        flow.confirm();
        assert!(flow.busy());
        flow.answered(Err(Refusal::unavailable("The service is not connected.")));
        assert_eq!(flow.stage(), &RemoveStage::Confirm);
        assert!(!flow.busy());
        assert_eq!(flow.error(), Some("The service is not connected."));
        assert!(flow.confirm(), "the same call can be tried again");
        assert_eq!(flow.error(), None);
    }

    #[test]
    fn a_second_confirm_while_a_call_is_out_does_nothing() {
        let mut flow = flow();
        assert!(flow.confirm());
        assert!(!flow.confirm());
    }

    #[test]
    fn each_dialog_names_its_loss() {
        let mut flow = flow();
        assert_eq!(flow.copy().title, "Delete workspace fix-login?");
        flow.confirm();
        flow.answered(Err(Refusal::Dirty));
        let copy = flow.copy();
        assert_eq!(copy.kind, DialogKind::Destructive);
        assert_eq!(copy.loss.len(), 1);
        flow.confirm();
        flow.answered(Err(Refusal::IgnoredFiles));
        let copy = flow.copy();
        assert_eq!(copy.title, "Delete workspace fix-login?");
        assert_eq!(copy.loss.len(), 1);
    }

    #[test]
    fn every_destructive_stage_offers_delete_workspace() {
        let mut flow = flow();
        assert_eq!(flow.copy().action, "Delete workspace");
        flow.confirm();
        flow.answered(Err(Refusal::Dirty));
        assert_eq!(flow.copy().action, "Delete workspace");
        flow.confirm();
        flow.answered(Err(Refusal::IgnoredFiles));
        assert_eq!(flow.copy().action, "Delete workspace");
    }

    fn menu_workspace(kind: WorkspaceKind, branch: Option<&str>) -> Workspace {
        let mut record = workspace("w1", "p1", "fix-login", 1);
        record.kind = kind;
        record.branch = branch.map(str::to_owned);
        record
    }

    fn find_item(
        entries: &[WorkspaceMenuEntry],
        wanted: WorkspaceMenuId,
    ) -> Option<&WorkspaceMenuEntry> {
        entries.iter().find_map(|entry| match entry {
            WorkspaceMenuEntry::Item { id, .. } if *id == wanted => Some(entry),
            WorkspaceMenuEntry::Submenu { items, .. } => find_item(items, wanted),
            _ => None,
        })
    }

    fn off_reason(entries: &[WorkspaceMenuEntry], wanted: WorkspaceMenuId) -> Option<&'static str> {
        match find_item(entries, wanted) {
            Some(WorkspaceMenuEntry::Item { off, .. }) => *off,
            _ => panic!("the menu has no item {wanted:?}"),
        }
    }

    #[test]
    fn the_workspace_menu_lists_the_spec_rows_in_order() {
        let menu = workspace_menu(&menu_workspace(WorkspaceKind::Worktree, Some("feat/x")));
        let labels: Vec<Option<&str>> = menu
            .iter()
            .map(|entry| match entry {
                WorkspaceMenuEntry::Item { label, .. }
                | WorkspaceMenuEntry::Submenu { label, .. } => Some(*label),
                WorkspaceMenuEntry::Divider => None,
            })
            .collect();
        assert_eq!(
            labels,
            [
                Some("New session"),
                Some("Open in"),
                Some("Rename"),
                Some("Edit workspace"),
                Some("Open in Source control"),
                Some("Developer"),
                None,
                Some("Delete workspace"),
            ]
        );
    }

    #[test]
    fn rows_whose_layer_is_not_built_yet_are_off_with_their_reason() {
        let menu = workspace_menu(&menu_workspace(WorkspaceKind::Worktree, Some("feat/x")));
        assert_eq!(off_reason(&menu, WorkspaceMenuId::NewSession), None);
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::Rename),
            Some(RENAME_LATER)
        );
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::EditWorkspace),
            Some(EDIT_LATER)
        );
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::OpenEditor),
            Some(TOOLS_LATER)
        );
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::SourceControl),
            Some(GIT_LATER)
        );
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::RevealInWorkPanel),
            Some(WORK_PANEL_LATER)
        );
        assert_eq!(off_reason(&menu, WorkspaceMenuId::CopyPath), None);
        assert_eq!(off_reason(&menu, WorkspaceMenuId::CopyBranch), None);
        assert_eq!(off_reason(&menu, WorkspaceMenuId::Delete), None);
    }

    #[test]
    fn git_rows_say_a_folder_has_no_git() {
        let menu = workspace_menu(&menu_workspace(WorkspaceKind::Folder, None));
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::SourceControl),
            Some(NEEDS_GIT)
        );
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::CopyBranch),
            Some(NEEDS_GIT)
        );
        assert_eq!(off_reason(&menu, WorkspaceMenuId::CopyPath), None);
    }

    #[test]
    fn copy_branch_is_off_for_a_worktree_without_a_branch() {
        let menu = workspace_menu(&menu_workspace(WorkspaceKind::Worktree, None));
        assert_eq!(
            off_reason(&menu, WorkspaceMenuId::CopyBranch),
            Some(NO_BRANCH)
        );
    }

    #[test]
    fn a_chosen_row_maps_to_its_action_and_off_rows_choose_nothing() {
        let menu = workspace_menu(&menu_workspace(WorkspaceKind::Worktree, Some("feat/x")));
        assert_eq!(
            chosen_workspace_item(&menu, 7, None),
            Some(WorkspaceMenuId::Delete)
        );
        assert_eq!(
            chosen_workspace_item(&menu, 5, Some(0)),
            Some(WorkspaceMenuId::CopyPath)
        );
        assert_eq!(
            chosen_workspace_item(&menu, 5, Some(1)),
            Some(WorkspaceMenuId::CopyBranch)
        );
        // A submenu only opens, the divider is not a row, an off row cannot run, and a row past
        // the end, or a submenu child on a plain row, chooses nothing.
        assert_eq!(
            chosen_workspace_item(&menu, 0, None),
            Some(WorkspaceMenuId::NewSession)
        );
        assert_eq!(chosen_workspace_item(&menu, 1, None), None);
        assert_eq!(chosen_workspace_item(&menu, 2, None), None);
        assert_eq!(chosen_workspace_item(&menu, 5, Some(2)), None);
        assert_eq!(chosen_workspace_item(&menu, 6, None), None);
        assert_eq!(chosen_workspace_item(&menu, 9, None), None);
        assert_eq!(chosen_workspace_item(&menu, 7, Some(0)), None);
    }
}
