//! The state and actions behind the add-project and remove-workspace dialogs, and the menus
//! that open them (decision 0028). The service answers move the flows; the store changes only
//! through the events those calls cause.

use super::path_field::PickKind;
use super::project_icon::read_icon_file;
use super::project_mark::project_mark;
use std::path::Path;

use super::Shell;
use crate::model::project_add::{ADD_PROJECT_LABEL, AddProjectForm, NO_PROJECTS_LABEL};
use crate::model::regions::Region;
use crate::model::session_delete::{SessionDeleteFlow, SessionDeleteStage};
use crate::model::session_menu::{
    SessionFacts, SessionMenuFacts, SessionMenuId, SessionMenuRow, session_menu,
};
use crate::model::status_items::{StatusItem, visibility_menu_items};
use crate::model::workspace_flow::{
    ProjectChoice, ProjectIcon, RemoveFlow, RemoveStage, WorkspaceMenuEntry, WorkspaceMenuId,
    chosen_workspace_item, project_menu_rows, workspace_menu,
};
use crate::service::Refusal;
use gpui::{
    App, AppContext, ClipboardItem, Context, Entity, Focusable, Pixels, Point, Subscription, Window,
};
use massorca_protocol::methods::projects::{Project, ProjectKind};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::WorkspaceKind;
use massorca_ui::IconName;
use massorca_ui::components::menu::{MenuChoice, MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::project_icon_popover::ProjectIconChoice;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};

/// The "Add a project" dialog: its path field, and the form the field feeds.
#[derive(Debug)]
pub(super) struct AddProjectDialog {
    pub form: AddProjectForm,
    pub path: Entity<TextField>,
    _subscriptions: Vec<Subscription>,
}

/// The rows of a project menu: every project with `chosen` checked, a caption when there are no
/// projects, then a separator and "Add a project…" last (A38). Both start pages use it, so
/// their menus list the same rows.
pub(super) fn project_menu(
    projects: &[Project],
    chosen: Option<&ProjectChoice>,
) -> (Vec<MenuEntry>, Vec<Option<ProjectMenuChoice>>) {
    let mut entries: Vec<MenuEntry> = Vec::new();
    let mut choices: Vec<Option<ProjectMenuChoice>> = Vec::new();
    if projects.is_empty() {
        entries.push(MenuEntry::Group(NO_PROJECTS_LABEL.into()));
        choices.push(None);
    }
    for row in project_menu_rows(projects) {
        let item = MenuItem::new(row.label.clone()).checked(chosen == Some(&row.choice));
        let item = match row.glyph {
            Some(glyph) => item.leading_element(move || project_mark(&glyph)),
            None => item.icon(icon_of(row.icon)),
        };
        entries.push(item.into());
        choices.push(Some(ProjectMenuChoice::Project(row.choice)));
    }
    entries.push(MenuEntry::Separator);
    choices.push(None);
    entries.push(
        MenuItem::new(ADD_PROJECT_LABEL)
            .icon(IconName::FolderPlus)
            .into(),
    );
    choices.push(Some(ProjectMenuChoice::AddProject));
    (entries, choices)
}

/// What a row of the project menu does when chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProjectMenuChoice {
    Project(ProjectChoice),
    AddProject,
}

/// What an open menu's rows stand for.
#[derive(Debug)]
pub(super) enum MenuTarget {
    /// A workspace's menu: its rows, and the workspace they act on.
    Workspace {
        id: String,
        rows: Vec<WorkspaceMenuEntry>,
    },
    /// A session row's menu: the session it acts on (None while its workspace has none), the workspace whose
    /// folder it names, and its rows.
    Session {
        session: Option<String>,
        workspace: String,
        rows: Vec<SessionMenuRow>,
    },
    /// A session a paired host runs: the host it runs on, the session, its rows, and its folder on that host.
    RemoteSession {
        host: String,
        session: String,
        rows: Vec<SessionMenuRow>,
        path: Option<String>,
    },
    /// One entry per menu row, with `None` for the heading.
    StatusItems(Vec<Option<StatusItem>>),
    /// The start page's project menu: the rows of the project menu, chosen to open a start page.
    StartProject(Vec<Option<ProjectMenuChoice>>),
    /// The project menu of the start page with no workspace: chosen to create the workspace in.
    HomeProject(Vec<Option<ProjectMenuChoice>>),
    /// A Marketplace item card's More menu: the item and its rows.
    MarketplaceItem {
        item: crate::model::marketplace::MarketplaceItemId,
        rows: Vec<crate::model::marketplace::ItemMenuId>,
    },
}

#[derive(Debug)]
pub(super) struct OpenMenu {
    pub menu: Entity<MenuView>,
    pub at: Point<Pixels>,
    pub target: MenuTarget,
    _subscription: Subscription,
}

/// Which workspace dialogs and menus are open: at most one add-project dialog and one remove
/// flow.
#[derive(Debug, Default)]
pub(super) struct WorkspaceDialogs {
    pub add_project: Option<AddProjectDialog>,
    pub remove: Option<RemoveFlow>,
    pub menu: Option<OpenMenu>,
    pub delete_session: Option<SessionDeleteFlow>,
}

pub(super) fn icon_of(icon: ProjectIcon) -> IconName {
    match icon {
        ProjectIcon::Folder => IconName::Folder,
        ProjectIcon::FolderStruck => IconName::FolderX,
    }
}

/// The rows as the ui menu draws them: an off row shows its reason, a submenu lists its items and
/// Delete is destructive (D-WS-02).
fn menu_entries(rows: &[WorkspaceMenuEntry]) -> Vec<MenuEntry> {
    rows.iter()
        .map(|row| match row {
            WorkspaceMenuEntry::Item { id, label, off } => item_entry(*id, label, *off),
            WorkspaceMenuEntry::Submenu { id, label, items } => MenuItem::new(*label)
                .icon(menu_icon(*id))
                .submenu(menu_entries(items))
                .into(),
            WorkspaceMenuEntry::Divider => MenuEntry::Separator,
        })
        .collect()
}

fn item_entry(id: WorkspaceMenuId, label: &'static str, off: Option<&'static str>) -> MenuEntry {
    let mut item = MenuItem::new(label).icon(menu_icon(id));
    if let Some(reason) = off {
        item = item.disabled(reason);
    }
    if id == WorkspaceMenuId::Delete {
        item = item.destructive();
    }
    item.into()
}

/// The session menu's rows as the ui menu draws them: each action with its icon and words, and the divider
/// before Delete session (A97).
fn session_menu_entries(rows: &[SessionMenuRow]) -> Vec<MenuEntry> {
    rows.iter()
        .map(|row| match row {
            SessionMenuRow::Item(id) => MenuEntry::from(session_item(*id)),
            SessionMenuRow::Divider => MenuEntry::Separator,
        })
        .collect()
}

fn session_item(id: SessionMenuId) -> MenuItem {
    let (label, icon) = match id {
        SessionMenuId::Stop => ("Stop", IconName::Square),
        SessionMenuId::CopyPath => ("Copy path", IconName::Copy),
        SessionMenuId::OpenFolder => ("Open folder", IconName::Folder),
        SessionMenuId::RevealInWorkPanel => ("Reveal in Work panel", IconName::PanelRight),
        SessionMenuId::Rename => ("Rename", IconName::Pencil),
        SessionMenuId::Archive => ("Archive", IconName::EyeOff),
        SessionMenuId::Unarchive => ("Unarchive", IconName::Eye),
        SessionMenuId::Delete => ("Delete session…", IconName::Trash2),
    };
    let item = MenuItem::new(label).icon(icon);
    if id == SessionMenuId::Delete {
        item.destructive()
    } else {
        item
    }
}

fn menu_icon(id: WorkspaceMenuId) -> IconName {
    match id {
        WorkspaceMenuId::NewSession => IconName::MessageSquare,
        WorkspaceMenuId::OpenIn => IconName::ExternalLink,
        WorkspaceMenuId::OpenFileManager => IconName::Folder,
        WorkspaceMenuId::OpenTerminal => IconName::SquareTerminal,
        WorkspaceMenuId::OpenEditor | WorkspaceMenuId::Developer => IconName::Code,
        WorkspaceMenuId::Rename | WorkspaceMenuId::EditWorkspace => IconName::Pencil,
        WorkspaceMenuId::SourceControl | WorkspaceMenuId::CopyBranch => IconName::GitBranch,
        WorkspaceMenuId::CopyPath => IconName::Copy,
        WorkspaceMenuId::RevealInWorkPanel => IconName::PanelRight,
        WorkspaceMenuId::Delete => IconName::Trash2,
    }
}

impl Shell {
    /// The context menu of a workspace (D-WS-02). No tree row opens it now (A97); the project row menu will.
    #[expect(
        dead_code,
        reason = "workspace menu waits for the project row menu (A97)"
    )]
    pub(super) fn open_workspace_menu(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.service.is_none() {
            return;
        }
        let Some(rows) = self
            .store
            .read(cx)
            .state()
            .workspace(&id)
            .map(workspace_menu)
        else {
            return;
        };
        self.open_menu(
            menu_entries(&rows),
            MenuTarget::Workspace { id, rows },
            OverlayLayer::Page,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// The context menu of a session row (A97), or of a workspace's row while it has no session.
    pub(super) fn open_session_menu(
        &mut self,
        session: Option<String>,
        workspace: String,
        local: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.service.is_none() {
            return;
        }
        let rows = {
            let state = self.store.read(cx).state();
            if state.workspace(&workspace).is_none() {
                return;
            }
            let facts = match session.as_deref() {
                Some(id) => {
                    let Some(agent) = state.agents.iter().find(|agent| agent.id == id) else {
                        return;
                    };
                    Some(SessionFacts {
                        running: agent.execution.state != TerminalState::Exited,
                        has_terminal: agent.terminal_id.is_some(),
                        archived: agent.archived,
                    })
                }
                None => None,
            };
            session_menu(SessionMenuFacts {
                session: facts,
                local,
                delete_offered: state.agent_archive,
                archive_offered: state.agent_archive,
            })
        };
        self.open_menu(
            session_menu_entries(&rows),
            MenuTarget::Session {
                session,
                workspace,
                rows,
            },
            OverlayLayer::Page,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// The context menu of a session row that a paired host runs (STAT-16). Its rows are decided from the host's own
    /// report: Stop is offered only while the host is reached and the run has not ended.
    pub(super) fn open_remote_session_menu(
        &mut self,
        host: String,
        session: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.service.is_none() {
            return;
        }
        let (rows, path) = {
            let state = self.store.read(cx).state();
            let Some(held) = state.host_sessions.get(&host) else {
                return;
            };
            let Some(agent) = held.sessions.iter().find(|agent| agent.id == session) else {
                return;
            };
            let facts = SessionFacts {
                running: held.reached && agent.execution.state != TerminalState::Exited,
                has_terminal: false,
                archived: false,
            };
            let path = held
                .workspaces
                .iter()
                .find(|workspace| workspace.id == agent.workspace_id)
                .map(|workspace| workspace.path.clone());
            let rows = session_menu(SessionMenuFacts {
                session: Some(facts),
                local: false,
                delete_offered: false,
                archive_offered: false,
            });
            (rows, path)
        };
        self.open_menu(
            session_menu_entries(&rows),
            MenuTarget::RemoteSession {
                host,
                session,
                rows,
                path,
            },
            OverlayLayer::Page,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// Runs a chosen row of a session a paired host runs. Stop reaches that host's own connection, never This PC's
    /// service. The rows that act on This PC's folders or terminals are not offered for these sessions.
    pub(super) fn run_remote_session_action(
        &mut self,
        host: &str,
        session: &str,
        path: Option<&str>,
        action: SessionMenuId,
        cx: &mut Context<Self>,
    ) {
        match action {
            SessionMenuId::Stop => {
                let Some(service) = self.service.clone() else {
                    return;
                };
                let (host, session) = (host.to_owned(), session.to_owned());
                cx.spawn(async move |_, _| {
                    // The host's follower reports the change; a refusal leaves the session as it was.
                    let _ = service.host_agent_stop(&host, &session).await;
                })
                .detach();
            }
            SessionMenuId::CopyPath => {
                if let Some(path) = path {
                    cx.write_to_clipboard(ClipboardItem::new_string(path.to_owned()));
                }
            }
            SessionMenuId::OpenFolder
            | SessionMenuId::RevealInWorkPanel
            | SessionMenuId::Rename
            | SessionMenuId::Archive
            | SessionMenuId::Unarchive
            | SessionMenuId::Delete => {}
        }
    }

    /// Runs a chosen workspace row. Rows that are off never reach here.
    fn run_workspace_action(&mut self, id: &str, action: WorkspaceMenuId, cx: &mut Context<Self>) {
        match action {
            WorkspaceMenuId::Delete => self.open_remove(id, cx),
            WorkspaceMenuId::CopyPath | WorkspaceMenuId::CopyBranch => {
                let text = self
                    .store
                    .read(cx)
                    .state()
                    .workspace(id)
                    .and_then(|workspace| {
                        if action == WorkspaceMenuId::CopyBranch {
                            workspace.branch.clone()
                        } else {
                            Some(workspace.path.clone())
                        }
                    });
                if let Some(text) = text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            WorkspaceMenuId::NewSession => self.open_new_session(id, cx),
            WorkspaceMenuId::OpenIn
            | WorkspaceMenuId::OpenFileManager
            | WorkspaceMenuId::OpenTerminal
            | WorkspaceMenuId::OpenEditor
            | WorkspaceMenuId::Rename
            | WorkspaceMenuId::EditWorkspace
            | WorkspaceMenuId::SourceControl
            | WorkspaceMenuId::Developer
            | WorkspaceMenuId::RevealInWorkPanel => {}
        }
    }

    /// Runs a chosen session row. Stop reaches the service; the other rows act on this PC.
    pub(super) fn run_session_action(
        &mut self,
        session: Option<&str>,
        workspace: &str,
        action: SessionMenuId,
        cx: &mut Context<Self>,
    ) {
        match action {
            SessionMenuId::Stop => {
                let (Some(session), Some(service)) = (session, self.service.clone()) else {
                    return;
                };
                let session = session.to_owned();
                cx.spawn(async move |_, _| {
                    // The store follows the service's own event; a refusal leaves the session as it was.
                    let _ = service.agent_stop(&session).await;
                })
                .detach();
            }
            SessionMenuId::CopyPath => {
                if let Some(path) = self.workspace_path(workspace, cx) {
                    cx.write_to_clipboard(ClipboardItem::new_string(path));
                }
            }
            SessionMenuId::OpenFolder => {
                if let Some(path) = self.workspace_path(workspace, cx) {
                    cx.open_with_system(Path::new(&path));
                }
            }
            SessionMenuId::RevealInWorkPanel => {
                let terminal = session.and_then(|id| {
                    self.store
                        .read(cx)
                        .state()
                        .agents
                        .iter()
                        .find(|agent| agent.id == id)
                        .and_then(|agent| agent.terminal_id.clone())
                });
                if let Some(terminal) = terminal {
                    self.model.show_region(Region::WorkPanel);
                    self.reveal_terminal(workspace, &terminal, cx);
                }
            }
            SessionMenuId::Delete => {
                if let Some(session) = session {
                    self.open_delete_session(session, cx);
                }
            }
            SessionMenuId::Rename => {
                if let Some(session) = session {
                    self.start_session_rename(session, cx);
                }
            }
            SessionMenuId::Archive | SessionMenuId::Unarchive => {
                let (Some(session), Some(service)) = (session, self.service.clone()) else {
                    return;
                };
                let (session, archived) = (session.to_owned(), action == SessionMenuId::Archive);
                cx.spawn(async move |_, _| {
                    // The store follows the service's own answer; a refusal leaves the session as it was.
                    let _ = service.agent_archive(&session, archived).await;
                })
                .detach();
            }
        }
    }

    fn workspace_path(&self, workspace: &str, cx: &App) -> Option<String> {
        self.store
            .read(cx)
            .state()
            .workspace(workspace)
            .map(|workspace| workspace.path.clone())
    }

    /// "Add a project…": a dialog for the path of a folder or repository (A38). Needs a service.
    pub(super) fn open_add_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.service.is_none() || self.dialogs.add_project.is_some() {
            return;
        }
        let path = cx.new(|cx| {
            TextField::new(cx)
                .label("Folder path")
                .placeholder("Path to a folder or Git repository")
        });
        let focus = path.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        let subscription = cx.subscribe_in(
            &path,
            window,
            |shell, _, event: &TextFieldEvent, _, cx| match event {
                TextFieldEvent::Changed => shell.read_add_project_path(cx),
                TextFieldEvent::Submitted => shell.submit_add_project(cx),
            },
        );
        self.dialogs.add_project = Some(AddProjectDialog {
            form: AddProjectForm::default(),
            path,
            _subscriptions: vec![subscription],
        });
        cx.notify();
    }

    /// Copies the typed path into the form, and clears its refusal once the path changes.
    fn read_add_project_path(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialogs.add_project.as_mut() else {
            return;
        };
        dialog.form.path = dialog.path.read(cx).text().to_owned();
        if dialog.form.path_error.take().is_some() {
            dialog
                .path
                .update(cx, |field, cx| field.set_error(None, cx));
        }
        cx.notify();
    }

    /// Sends the path. The service says first whether it is a Git repository: a plain folder waits for the
    /// warning (WS-31), anything else goes to `project.add`. The dialog stays open and busy until then.
    pub(super) fn submit_add_project(&mut self, cx: &mut Context<Self>) {
        let (Some(handle), Some(dialog)) =
            (self.service.clone(), self.dialogs.add_project.as_mut())
        else {
            return;
        };
        let Some(params) = dialog.form.params() else {
            return;
        };
        dialog.form.begin();
        cx.notify();
        cx.spawn(async move |this, cx| {
            match handle.inspect_project(params.path.clone()).await {
                Ok(found) if found.kind == ProjectKind::Folder && found.registered.is_none() => {
                    this.update(cx, |shell, cx| {
                        shell.warn_about_plain_folder(params.path, cx)
                    })
                    .ok();
                }
                // A repository, a folder already added, or an answer this build cannot read: `project.add` decides.
                _ => {
                    let result = handle.add_project(params).await;
                    this.update(cx, |shell, cx| shell.finish_add_project(result, cx))
                        .ok();
                }
            }
        })
        .detach();
    }

    /// The folder is not a Git repository: the add waits on the warning, and nothing is added yet (WS-31).
    fn warn_about_plain_folder(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(dialog) = self.dialogs.add_project.as_mut() {
            dialog.form.hold_plain_folder(path);
        }
        cx.notify();
    }

    /// Continue without Git: the held folder is added as a plain folder, the one add the warning allows (WS-31).
    pub(super) fn continue_add_project_without_git(&mut self, cx: &mut Context<Self>) {
        let (Some(handle), Some(dialog)) =
            (self.service.clone(), self.dialogs.add_project.as_mut())
        else {
            return;
        };
        let Some(params) = dialog.form.continue_without_git() else {
            return;
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.add_project(params).await;
            this.update(cx, |shell, cx| shell.finish_add_project(result, cx))
                .ok();
        })
        .detach();
    }

    /// Cancel on the warning: nothing is added, and the path form comes back with the path still typed (WS-31).
    pub(super) fn cancel_plain_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.dialogs.add_project.as_mut() {
            dialog.form.cancel_plain_folder();
        }
        cx.notify();
    }

    /// A new project closes the dialog and becomes the project of the page with no workspace. A
    /// refusal keeps the dialog open with its reason shown.
    fn finish_add_project(&mut self, result: Result<Project, Refusal>, cx: &mut Context<Self>) {
        self.note_refusal(result.as_ref().err());
        match result {
            Ok(project) => {
                self.dialogs.add_project = None;
                self.home_project = Some(ProjectChoice::Project(project.id));
            }
            Err(refusal) => {
                if let Some(dialog) = self.dialogs.add_project.as_mut() {
                    dialog.form.refused(&refusal);
                    if let Some(message) = dialog.form.path_error.clone() {
                        dialog
                            .path
                            .update(cx, |field, cx| field.set_error(Some(message.into()), cx));
                    }
                }
            }
        }
        cx.notify();
    }

    /// Opens or closes the Add a project dialog's icon grid.
    pub(super) fn set_add_project_icon_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if let Some(dialog) = self.dialogs.add_project.as_mut() {
            dialog.form.set_icon_open(open);
            cx.notify();
        }
    }

    /// Chooses the dialog's icon: a bundled one, Reset for the service's own detection, or a PNG the person picks.
    pub(super) fn choose_add_project_icon(
        &mut self,
        choice: ProjectIconChoice,
        cx: &mut Context<Self>,
    ) {
        match choice {
            ProjectIconChoice::Lucide(name) => self.set_add_project_icon(
                Some(massorca_protocol::methods::projects::ProjectIcon::Lucide {
                    name: name.to_owned(),
                    color: None,
                }),
                cx,
            ),
            ProjectIconChoice::Reset => self.set_add_project_icon(None, cx),
            ProjectIconChoice::Upload => self.upload_add_project_icon(cx),
        }
    }

    fn set_add_project_icon(
        &mut self,
        icon: Option<massorca_protocol::methods::projects::ProjectIcon>,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.dialogs.add_project.as_mut() {
            dialog.form.choose_icon(icon);
            cx.notify();
        }
    }

    /// Reads the PNG the person picks as the dialog's icon; a file that is not one keeps the grid open with its reason.
    fn upload_add_project_icon(&self, cx: &mut Context<Self>) {
        let picked = self.path_picker.pick(PickKind::File, cx);
        cx.spawn(async move |this, cx| {
            let Some(path) = picked.await else {
                return;
            };
            let read = read_icon_file(&path);
            let _ = this.update(cx, |shell, cx| match read {
                Ok(icon) => shell.set_add_project_icon(Some(icon), cx),
                Err(reason) => {
                    if let Some(dialog) = shell.dialogs.add_project.as_mut() {
                        dialog.form.refuse_icon(reason);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Closes the add-project dialog unless its call is out.
    pub(super) fn close_add_project(&mut self, cx: &mut Context<Self>) {
        if self
            .dialogs
            .add_project
            .as_ref()
            .is_some_and(|dialog| !dialog.form.busy)
        {
            self.dialogs.add_project = None;
            cx.notify();
        }
    }

    /// The Status bar's visibility menu: a checkable row per item under a heading (StatusBar
    /// spec). Choosing a row shows or hides that item for this window.
    pub(super) fn open_status_menu(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = visibility_menu_items(self.model.snapshot().service_status.is_some());
        let mut entries = vec![MenuEntry::Group("Status bar items".into())];
        let mut rows = vec![None];
        for item in items {
            let shown = self.model.status_items().is_shown(item);
            entries.push(MenuItem::new(item.label()).checked(shown).into());
            rows.push(Some(item));
        }
        self.open_menu(
            entries,
            MenuTarget::StatusItems(rows),
            OverlayLayer::Page,
            at,
            window,
            cx,
        );
    }

    pub(super) fn open_menu(
        &mut self,
        entries: Vec<MenuEntry>,
        target: MenuTarget,
        layer: OverlayLayer,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(layer));
        let subscription =
            cx.subscribe_in(&menu, window, |shell, _, event: &MenuEvent, window, cx| {
                let menu = shell.dialogs.menu.take();
                if let (MenuEvent::Chosen(choice), Some(menu)) = (event, menu) {
                    shell.menu_chosen(menu.target, *choice, window, cx);
                }
                cx.notify();
            });
        self.dialogs.menu = Some(OpenMenu {
            menu,
            at,
            target,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn menu_chosen(
        &mut self,
        target: MenuTarget,
        choice: MenuChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let row = choice.row;
        match target {
            MenuTarget::StatusItems(rows) => {
                if let Some(item) = rows.get(row).copied().flatten() {
                    self.model.toggle_status_item(item);
                }
            }
            MenuTarget::Workspace { id, rows } => {
                if let Some(action) = chosen_workspace_item(&rows, row, choice.child) {
                    self.run_workspace_action(&id, action, cx);
                }
            }
            MenuTarget::Session {
                session,
                workspace,
                rows,
            } => {
                if let Some(SessionMenuRow::Item(action)) = rows.get(row).copied() {
                    self.run_session_action(session.as_deref(), &workspace, action, cx);
                }
            }
            MenuTarget::RemoteSession {
                host,
                session,
                rows,
                path,
            } => {
                if let Some(SessionMenuRow::Item(action)) = rows.get(row).copied() {
                    self.run_remote_session_action(&host, &session, path.as_deref(), action, cx);
                }
            }
            MenuTarget::StartProject(choices) => match choices.get(row).cloned().flatten() {
                Some(ProjectMenuChoice::Project(choice)) => self.open_start_project(choice, cx),
                Some(ProjectMenuChoice::AddProject) => self.open_add_project(window, cx),
                None => {}
            },
            MenuTarget::HomeProject(choices) => match choices.get(row).cloned().flatten() {
                Some(ProjectMenuChoice::Project(choice)) => self.home_project = Some(choice),
                Some(ProjectMenuChoice::AddProject) => self.open_add_project(window, cx),
                // A row that is off, such as "No project" before the service offers it, stands for nothing.
                None => {}
            },
            MenuTarget::MarketplaceItem { item, rows } => {
                if let Some(action) = rows.get(row).copied() {
                    self.run_item_menu(item, action, cx);
                }
            }
        }
    }

    /// Opens the remove flow for a workspace the store knows.
    /// Opens the confirm that deletes a session the store knows (Delete session…, A97).
    fn open_delete_session(&mut self, session: &str, cx: &mut Context<Self>) {
        let flow = {
            let state = self.store.read(cx).state();
            let Some(agent) = state.agents.iter().find(|agent| agent.id == session) else {
                return;
            };
            let Some(workspace) = state.workspace(&agent.workspace_id) else {
                return;
            };
            let title = agent
                .title
                .clone()
                .or_else(|| workspace.branch.clone())
                .unwrap_or_else(|| workspace.name.clone());
            SessionDeleteFlow::new(session, title, workspace.kind != WorkspaceKind::Folder)
        };
        if self.dialogs.delete_session.is_none() {
            self.dialogs.delete_session = Some(flow);
            cx.notify();
        }
    }

    /// Closes the delete confirm unless a call is out.
    pub(super) fn close_session_delete(&mut self, cx: &mut Context<Self>) {
        if self
            .dialogs
            .delete_session
            .as_ref()
            .is_some_and(|flow| !flow.busy())
        {
            self.dialogs.delete_session = None;
            cx.notify();
        }
    }

    /// The worktree option of the delete confirm. The flow ignores it once the call is out.
    pub(super) fn set_session_delete_remove_workspace(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(flow) = self.dialogs.delete_session.as_mut() {
            flow.set_remove_workspace(on);
            cx.notify();
        }
    }

    /// The delete confirm's action: sends the forget its stage asks for. A plain refusal closes the confirm.
    pub(super) fn confirm_session_delete(&mut self, cx: &mut Context<Self>) {
        let (Some(handle), Some(flow)) =
            (self.service.clone(), self.dialogs.delete_session.as_mut())
        else {
            return;
        };
        if !flow.confirm() {
            if matches!(flow.stage(), SessionDeleteStage::Refused { .. }) {
                self.close_session_delete(cx);
            }
            return;
        }
        let params = flow.params();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.agent_forget(params).await.map(|_| ());
            this.update(cx, |shell, cx| shell.finish_session_delete(result, cx))
                .ok();
        })
        .detach();
    }

    fn finish_session_delete(&mut self, result: Result<(), Refusal>, cx: &mut Context<Self>) {
        if let Some(flow) = self.dialogs.delete_session.as_mut()
            && flow.answered(result)
        {
            // The row leaves the Sidebar once the service's forget reaches the store.
            self.dialogs.delete_session = None;
        }
        cx.notify();
    }

    pub(super) fn open_remove(&mut self, id: &str, cx: &mut Context<Self>) {
        let name = self
            .store
            .read(cx)
            .state()
            .workspace(id)
            .map(|workspace| workspace.name.clone());
        if let (Some(name), None) = (name, &self.dialogs.remove) {
            self.dialogs.remove = Some(RemoveFlow::new(id, name));
            cx.notify();
        }
    }

    pub(super) fn close_remove(&mut self, cx: &mut Context<Self>) {
        if self
            .dialogs
            .remove
            .as_ref()
            .is_some_and(|flow| !flow.busy())
        {
            self.dialogs.remove = None;
            cx.notify();
        }
    }

    /// The remove dialog's action: retries with the waiver its stage asks for, or closes a plain
    /// refusal.
    pub(super) fn confirm_remove(&mut self, cx: &mut Context<Self>) {
        let (Some(handle), Some(flow)) = (self.service.clone(), self.dialogs.remove.as_mut())
        else {
            return;
        };
        if !flow.confirm() {
            if matches!(flow.stage(), RemoveStage::Refused { .. }) {
                self.close_remove(cx);
            }
            return;
        }
        let params = flow.params();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.remove_workspace(params).await.map(|_| ());
            this.update(cx, |shell, cx| shell.finish_remove(result, cx))
                .ok();
        })
        .detach();
    }

    fn finish_remove(&mut self, result: Result<(), Refusal>, cx: &mut Context<Self>) {
        if let Some(flow) = self.dialogs.remove.as_mut()
            && flow.answered(result)
        {
            // The row leaves the Sidebar when the service's removal event arrives.
            self.dialogs.remove = None;
        }
        cx.notify();
    }
}
