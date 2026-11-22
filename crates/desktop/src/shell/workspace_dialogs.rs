//! The state and actions behind the create-workspace and remove-workspace dialogs, and the menus
//! that open them (decision 0028). The service answers move the flows; the store changes only
//! through the events those calls cause.

use super::Shell;
use crate::model::project_add::{ADD_PROJECT_LABEL, AddProjectForm, NO_PROJECTS_LABEL};
use crate::model::status_items::{StatusItem, visibility_menu_items};
use crate::model::workspace_flow::{
    CreateForm, ProjectChoice, ProjectIcon, RemoveFlow, RemoveStage, project_menu_rows,
};
use crate::service::Refusal;
use gpui::{AppContext, Context, Entity, Focusable, Pixels, Point, Subscription, Window};
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::workspaces::Workspace;
use massorca_ui::IconName;
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};

/// The create form with the text fields it owns.
#[derive(Debug)]
pub(super) struct CreateDialog {
    pub form: CreateForm,
    pub name: Entity<TextField>,
    pub branch: Entity<TextField>,
    pub base: Entity<TextField>,
    _subscriptions: Vec<Subscription>,
}

/// The "Add a project" dialog: its path field, and the form the field feeds.
#[derive(Debug)]
pub(super) struct AddProjectDialog {
    pub form: AddProjectForm,
    pub path: Entity<TextField>,
    _subscriptions: Vec<Subscription>,
}

/// The rows of a project menu: every project with `chosen` checked, a caption when there are no
/// projects, then a separator and "Add a project…" last (A38). Shared by the create form and by
/// the start page, so both menus list the same rows.
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
        entries.push(
            MenuItem::new(row.label.clone())
                .icon(icon_of(row.icon))
                .checked(chosen == Some(&row.choice))
                .into(),
        );
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
    /// One entry per menu row, with `None` for captions and separators.
    Projects(Vec<Option<ProjectMenuChoice>>),
    Workspace(String),
    /// One entry per menu row, with `None` for the heading.
    StatusItems(Vec<Option<StatusItem>>),
    /// The start page's project menu: the rows of the project menu, chosen to open a start page.
    StartProject(Vec<Option<ProjectMenuChoice>>),
}

#[derive(Debug)]
pub(super) struct OpenMenu {
    pub menu: Entity<MenuView>,
    pub at: Point<Pixels>,
    pub target: MenuTarget,
    _subscription: Subscription,
}

/// Which workspace dialogs and menus are open: at most one create form, one add-project dialog and
/// one remove flow.
#[derive(Debug, Default)]
pub(super) struct WorkspaceDialogs {
    pub create: Option<CreateDialog>,
    pub add_project: Option<AddProjectDialog>,
    pub remove: Option<RemoveFlow>,
    pub menu: Option<OpenMenu>,
}

pub(super) fn icon_of(icon: ProjectIcon) -> IconName {
    match icon {
        ProjectIcon::Folder => IconName::Folder,
        ProjectIcon::FolderStruck => IconName::FolderX,
    }
}

impl Shell {
    /// Opens "New workspace". Sample data has no service to ask, so nothing opens.
    pub(super) fn open_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.service.is_none() || self.dialogs.create.is_some() {
            return;
        }
        let projects = self.store.read(cx).state().projects.clone();
        let name = cx.new(|cx| TextField::new(cx).label("Name").placeholder("fix-login"));
        let branch = cx.new(|cx| {
            TextField::new(cx)
                .label("Branch")
                .placeholder("Same as the name")
        });
        let base = cx.new(|cx| {
            TextField::new(cx)
                .label("Base")
                .placeholder("The default branch")
        });
        let mut subscriptions = Vec::new();
        for (field, submits) in [(&name, true), (&branch, false), (&base, false)] {
            subscriptions.push(cx.subscribe_in(
                field,
                window,
                move |shell, _, event: &TextFieldEvent, _, cx| match event {
                    TextFieldEvent::Changed => shell.read_create_fields(cx),
                    TextFieldEvent::Submitted if submits => shell.submit_create(cx),
                    TextFieldEvent::Submitted => {}
                },
            ));
        }
        self.dialogs.create = Some(CreateDialog {
            form: CreateForm::new(&projects),
            name,
            branch,
            base,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    /// Copies what is typed into the form, and clears a name error once the name changes.
    fn read_create_fields(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialogs.create.as_mut() else {
            return;
        };
        dialog.form.name = dialog.name.read(cx).text().to_owned();
        dialog.form.branch = dialog.branch.read(cx).text().to_owned();
        dialog.form.base = dialog.base.read(cx).text().to_owned();
        if dialog.form.name_error.take().is_some() {
            dialog
                .name
                .update(cx, |field, cx| field.set_error(None, cx));
        }
        cx.notify();
    }

    pub(super) fn set_existing_branch(&mut self, existing: bool, cx: &mut Context<Self>) {
        if let Some(dialog) = self.dialogs.create.as_mut() {
            dialog.form.existing_branch = existing;
            cx.notify();
        }
    }

    pub(super) fn close_create(&mut self, cx: &mut Context<Self>) {
        if self.dialogs.create.as_ref().is_some_and(|d| !d.form.busy) {
            self.dialogs.create = None;
            cx.notify();
        }
    }

    /// Sends the form. It stays open and busy until the service answers.
    pub(super) fn submit_create(&mut self, cx: &mut Context<Self>) {
        let (Some(handle), Some(dialog)) = (self.service.clone(), self.dialogs.create.as_mut())
        else {
            return;
        };
        let Some(params) = dialog.form.params() else {
            return;
        };
        dialog.form.begin();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = handle.create_workspace(params).await;
            this.update(cx, |shell, cx| shell.finish_create(result, cx))
                .ok();
        })
        .detach();
    }

    fn finish_create(&mut self, result: Result<Workspace, Refusal>, cx: &mut Context<Self>) {
        match result {
            // The new row arrives as an event, not from this answer.
            Ok(_) => self.dialogs.create = None,
            Err(refusal) => {
                if let Some(dialog) = self.dialogs.create.as_mut() {
                    dialog.form.refused(&refusal);
                    if let Refusal::NameProblem { message } = &refusal {
                        let message = message.clone();
                        dialog
                            .name
                            .update(cx, |field, cx| field.set_error(Some(message.into()), cx));
                    }
                }
            }
        }
        cx.notify();
    }

    /// The project menu of the create form: every project, then "No project" (A95), a separator
    /// and "Add a project…" last (A38). With no project, a caption says so.
    pub(super) fn open_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let projects = self.store.read(cx).state().projects.clone();
        let Some(chosen) = self.dialogs.create.as_ref().map(|d| d.form.project.clone()) else {
            return;
        };
        let (entries, choices) = project_menu(&projects, chosen.as_ref());
        self.open_menu(
            entries,
            MenuTarget::Projects(choices),
            OverlayLayer::Dialog,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// The context menu of a workspace row.
    pub(super) fn open_workspace_menu(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.service.is_none() {
            return;
        }
        let entries = vec![
            MenuItem::new("Remove workspace...")
                .icon(IconName::Trash2)
                .destructive()
                .into(),
        ];
        self.open_menu(
            entries,
            MenuTarget::Workspace(id),
            OverlayLayer::Page,
            window.mouse_position(),
            window,
            cx,
        );
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

    /// Sends the path. The dialog stays open and busy until the service answers.
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
            let result = handle.add_project(params).await;
            this.update(cx, |shell, cx| shell.finish_add_project(result, cx))
                .ok();
        })
        .detach();
    }

    /// A new project closes the dialog and is chosen in the create form that asked for it. A
    /// refusal keeps the dialog open with its reason shown.
    fn finish_add_project(&mut self, result: Result<Project, Refusal>, cx: &mut Context<Self>) {
        match result {
            Ok(project) => {
                self.dialogs.add_project = None;
                if let Some(create) = self.dialogs.create.as_mut() {
                    create.form.project = Some(ProjectChoice::Project(project.id));
                }
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
                    shell.menu_chosen(menu.target, choice.row, window, cx);
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
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match target {
            MenuTarget::Projects(choices) => match choices.get(row).cloned().flatten() {
                Some(ProjectMenuChoice::Project(choice)) => {
                    if let Some(dialog) = self.dialogs.create.as_mut() {
                        dialog.form.project = Some(choice);
                        let focus = dialog.name.read(cx).focus_handle(cx);
                        window.focus(&focus, cx);
                    }
                }
                Some(ProjectMenuChoice::AddProject) => self.open_add_project(window, cx),
                None => {}
            },
            MenuTarget::StatusItems(rows) => {
                if let Some(item) = rows.get(row).copied().flatten() {
                    self.model.toggle_status_item(item);
                }
            }
            MenuTarget::Workspace(id) => self.open_remove(&id, cx),
            MenuTarget::StartProject(choices) => match choices.get(row).cloned().flatten() {
                Some(ProjectMenuChoice::Project(choice)) => self.open_start_project(choice, cx),
                Some(ProjectMenuChoice::AddProject) => self.open_add_project(window, cx),
                None => {}
            },
        }
    }

    /// Opens the remove flow for a workspace the store knows.
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
