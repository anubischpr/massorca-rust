//! The state and actions behind the create-workspace and remove-workspace dialogs, and the menus
//! that open them (decision 0028). The service answers move the flows; the store changes only
//! through the events those calls cause.

use super::Shell;
use crate::model::workspace_flow::{
    CreateForm, ProjectChoice, ProjectIcon, RemoveFlow, RemoveStage, project_menu_rows,
};
use crate::service::Refusal;
use gpui::{AppContext, Context, Entity, Focusable, Pixels, Point, Subscription, Window};
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

/// What an open menu's rows stand for.
#[derive(Debug)]
pub(super) enum MenuTarget {
    Projects(Vec<ProjectChoice>),
    Workspace(String),
}

#[derive(Debug)]
pub(super) struct OpenMenu {
    pub menu: Entity<MenuView>,
    pub at: Point<Pixels>,
    pub target: MenuTarget,
    _subscription: Subscription,
}

/// Which workspace dialogs and menus are open. At most one create form and one remove flow.
#[derive(Debug, Default)]
pub(super) struct WorkspaceDialogs {
    pub create: Option<CreateDialog>,
    pub remove: Option<RemoveFlow>,
    pub menu: Option<OpenMenu>,
}

fn icon_of(icon: ProjectIcon) -> IconName {
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

    /// The project menu of the create form: every project, then "No project" (A95).
    pub(super) fn open_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let projects = self.store.read(cx).state().projects.clone();
        let Some(chosen) = self.dialogs.create.as_ref().map(|d| d.form.project.clone()) else {
            return;
        };
        let rows = project_menu_rows(&projects);
        let entries: Vec<MenuEntry> = rows
            .iter()
            .map(|row| {
                MenuItem::new(row.label.clone())
                    .icon(icon_of(row.icon))
                    .checked(chosen.as_ref() == Some(&row.choice))
                    .into()
            })
            .collect();
        let choices = rows.into_iter().map(|row| row.choice).collect();
        self.open_menu(
            entries,
            MenuTarget::Projects(choices),
            OverlayLayer::Dialog,
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
            window,
            cx,
        );
    }

    fn open_menu(
        &mut self,
        entries: Vec<MenuEntry>,
        target: MenuTarget,
        layer: OverlayLayer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = window.mouse_position();
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
            MenuTarget::Projects(choices) => {
                if let (Some(choice), Some(dialog)) =
                    (choices.get(row), self.dialogs.create.as_mut())
                {
                    dialog.form.project = Some(choice.clone());
                    let focus = dialog.name.read(cx).focus_handle(cx);
                    window.focus(&focus, cx);
                }
            }
            MenuTarget::Workspace(id) => self.open_remove(&id, cx),
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
