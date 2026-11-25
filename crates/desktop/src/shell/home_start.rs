//! The start page with no workspace (D-AG-01, J3), where "+ Create" lands: the question, the placement card with
//! the host, project and branch, and the start composer with its agent pick and access chip. Send creates a
//! workspace in the project, named from the prompt, then starts the agent in it.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, WeakEntity, Window, div, px};
use massorca_protocol::methods::projects::Project;
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::composer::Composer;
use massorca_ui::components::menu::{MenuEntry, MenuItem};
use massorca_ui::components::popover::{
    OverlayLayer, Popover, PopoverPlacement, PopoverVariant, PopoverWidth,
};
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, SPACE_2, SPACE_3, SPACE_6, TYPE_CAPTION, TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

use super::Shell;
use super::start_page::{
    key_hints, profile_banner, project_title_link, refuse_send, starting_line,
};
use super::start_placement::{host_pill, pill, pill_focus, placement_card};
use super::workspace_dialogs::{MenuTarget, ProjectMenuChoice, project_menu};
use crate::model::home_start::{
    CHOOSE_A_PROJECT, HOME_START, NO_PROJECT_NEEDS_SERVICE, WorkspaceDetails, branch_pill,
    create_params, home_choice, home_start_params, no_project_branch_pill, no_project_start_params,
};
use crate::model::service_view::LOCAL_HOST;
use crate::model::start_page::{StartMode, StartingSession, TERMINAL_NOTE};
use crate::model::workspace_flow::{NO_PROJECT_LABEL, ProjectChoice};
use crate::service::StoreUpdate;

/// What the page draws, gathered by the shell for one frame.
pub(super) struct HomeFrame {
    pub composer: Entity<Composer>,
    pub shell: WeakEntity<Shell>,
    /// The project's name, "No project" when the person chose it, or `None` while there is none to create in.
    pub project: Option<String>,
    /// Whether "No project" is chosen: the title names no project, and the branch pill names no worktree.
    pub no_project: bool,
    /// The project's base branch, which a new worktree starts from when the branch menu names none.
    pub project_base: Option<String>,
    /// The start in flight from this page, shown under the composer.
    pub starting: Option<String>,
    pub mode: StartMode,
    /// The service refused because the data folder is not secured.
    pub profile_unsafe: bool,
    /// The branch menu's fields and whether it is open.
    pub details: HomeDetails,
}

/// The branch menu of the start page with no workspace (D-AG-01, A89): whether it is open, whether Send reuses a
/// branch, and its text fields. Kept on the shell so a draft survives redraws.
#[derive(Clone, Debug)]
pub(super) struct HomeDetails {
    pub branch_menu: bool,
    pub existing_branch: bool,
    pub name: Entity<TextField>,
    pub branch: Entity<TextField>,
    pub base: Entity<TextField>,
}

impl HomeDetails {
    /// The menu starts closed, with empty fields.
    pub(super) fn new(cx: &mut Context<Shell>) -> Self {
        Self {
            branch_menu: false,
            existing_branch: false,
            name: cx.new(|cx| {
                TextField::new(cx)
                    .label("Name")
                    .placeholder("From the prompt")
            }),
            branch: cx.new(|cx| {
                TextField::new(cx)
                    .label("Branch")
                    .placeholder("Same as the name")
            }),
            base: cx.new(|cx| {
                TextField::new(cx)
                    .label("Base")
                    .placeholder("The default branch")
            }),
        }
    }

    /// What Send reads from the menu: the fields' text and the reuse choice.
    fn values(&self, cx: &App) -> WorkspaceDetails {
        WorkspaceDetails {
            name: self.name.read(cx).text().to_owned(),
            branch: self.branch.read(cx).text().to_owned(),
            base: self.base.read(cx).text().to_owned(),
            existing_branch: self.existing_branch,
        }
    }
}

/// The project menu of this page: the shared rows. "No project" is chosen only once the service can start an agent
/// without a workspace (A95); until then it is off with its reason, and its row stands for nothing.
pub(super) fn home_project_menu(
    projects: &[Project],
    chosen: Option<&ProjectChoice>,
    no_project_offered: bool,
) -> (Vec<MenuEntry>, Vec<Option<ProjectMenuChoice>>) {
    let (mut entries, mut choices) = project_menu(projects, chosen);
    if no_project_offered {
        return (entries, choices);
    }
    for (entry, choice) in entries.iter_mut().zip(choices.iter_mut()) {
        if *choice == Some(ProjectMenuChoice::Project(ProjectChoice::NoProject)) {
            *entry = MenuItem::new(NO_PROJECT_LABEL)
                .icon(IconName::FolderX)
                .disabled(NO_PROJECT_NEEDS_SERVICE)
                .into();
            *choice = None;
        }
    }
    (entries, choices)
}

/// The page, centred in the Canvas.
pub(super) fn home_page(frame: HomeFrame, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let title = {
        let shell = frame.shell.clone();
        let heading = div()
            .debug_selector(|| "home-start-title".to_owned())
            .flex()
            .flex_row()
            .flex_wrap()
            .justify_center()
            .text_style(TYPE_HEADING)
            .text_color(p.text.primary);
        match frame.project.clone() {
            Some(name) if !frame.no_project => heading
                .child("What should we work on in ")
                .child(project_title_link(
                    "home-start-project-link",
                    name,
                    &theme,
                    move |window, cx| {
                        let _ =
                            shell.update(cx, |shell, cx| shell.open_home_project_menu(window, cx));
                    },
                ))
                .child("?"),
            _ => heading.child("What should we work on?"),
        }
    };
    let labels = if frame.no_project {
        no_project_branch_pill()
    } else {
        branch_pill(&frame.details.values(cx), frame.project_base.as_deref())
    };
    let host = host_pill(
        "home-start-host",
        LOCAL_HOST,
        &theme,
        pill_focus("home-start-host", window, cx),
    );
    let project = {
        let shell = frame.shell.clone();
        pill(
            "home-start-project",
            IconName::Folder,
            frame
                .project
                .clone()
                .unwrap_or_else(|| CHOOSE_A_PROJECT.to_owned()),
            None,
            true,
            &theme,
            pill_focus("home-start-project", window, cx),
        )
        .on_click(move |_, window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_home_project_menu(window, cx));
        })
    };
    let branch = {
        let shell = frame.shell.clone();
        let no_project = frame.no_project;
        let trigger = pill(
            "home-start-branch",
            IconName::GitBranch,
            labels.label,
            Some(labels.detail),
            !no_project,
            &theme,
            pill_focus("home-start-branch", window, cx),
        )
        .when(!no_project, |pill| {
            pill.on_click(move |_, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.toggle_branch_menu(cx));
            })
        });
        let dismiss = frame.shell.clone();
        Popover::new("home-start-branch-menu", trigger)
            .open(frame.details.branch_menu && !no_project)
            .variant(PopoverVariant::Form)
            .width(PopoverWidth::Medium)
            .placement(PopoverPlacement::Below)
            .layer(OverlayLayer::Dialog)
            .child(branch_fields(&frame.details, frame.shell.clone()))
            .on_dismiss(move |_, cx| {
                let _ = dismiss.update(cx, |shell, cx| shell.close_branch_menu(cx));
            })
    };
    let placement = placement_card(host, project, branch, &theme);
    let stack = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .child(placement)
                .child(frame.composer.clone()),
        )
        .when(frame.mode == StartMode::Terminal, |column| {
            column.child(
                div()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.secondary)
                    .child(TERMINAL_NOTE),
            )
        })
        .child(key_hints(&theme))
        .when_some(frame.starting.clone(), |column, status| {
            column.child(starting_line(status, &theme))
        });
    div()
        .debug_selector(|| "home-start-column".to_owned())
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .child(div().flex().justify_center().child(rose_logo()))
        .child(title)
        .when(frame.profile_unsafe, |column| {
            column.child(profile_banner())
        })
        .child(stack)
        .into_any_element()
}

/// The branch menu's fields, as Send reads them for `workspace.create`: the name, the branch, the reuse choice and the
/// base. Host lives in its own pill.
fn branch_fields(details: &HomeDetails, shell: WeakEntity<Shell>) -> impl IntoElement {
    let existing = {
        let shell = shell.clone();
        Checkbox::new("home-start-existing", details.existing_branch)
            .label("Use an existing branch")
            .on_change(move |checked, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.set_home_existing_branch(checked, cx));
            })
    };
    div()
        .debug_selector(|| "home-start-branch-fields".to_owned())
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(details.name.clone())
        .child(details.branch.clone())
        .child(existing)
        .child(details.base.clone())
}

impl Shell {
    /// "+ Create": the start page with no workspace, its composer focused (D-AG-01).
    pub(super) fn open_create_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model.close_session();
        self.model.select_workspace(None);
        let composer = self.start_composer(HOME_START, cx);
        composer.update(cx, |composer, cx| composer.focus(window, cx));
        cx.notify();
    }

    /// Opens or closes the branch pill's menu. It starts closed.
    pub(super) fn toggle_branch_menu(&mut self, cx: &mut Context<Self>) {
        self.home_details.branch_menu = !self.home_details.branch_menu;
        cx.notify();
    }

    /// Closes the branch pill's menu on a click outside it or Escape.
    pub(super) fn close_branch_menu(&mut self, cx: &mut Context<Self>) {
        self.home_details.branch_menu = false;
        cx.notify();
    }

    /// "Use an existing branch": Send reuses the branch the field names instead of creating one.
    pub(super) fn set_home_existing_branch(&mut self, checked: bool, cx: &mut Context<Self>) {
        self.home_details.existing_branch = checked;
        cx.notify();
    }

    /// The page's frame while the Canvas shows it; sample data has no service to start with.
    pub(super) fn home_frame(&mut self, cx: &mut Context<Self>) -> Option<HomeFrame> {
        self.service.as_ref()?;
        let composer = self.start_composer(HOME_START, cx);
        let (project, project_base, no_project) = {
            let projects = &self.store.read(cx).state().projects;
            match home_choice(projects, self.home_project.as_ref()) {
                Some(ProjectChoice::NoProject) => (Some(NO_PROJECT_LABEL.to_owned()), None, true),
                Some(ProjectChoice::Project(id)) => match projects.iter().find(|p| p.id == id) {
                    Some(project) => (Some(project.name.clone()), project.base_ref.clone(), false),
                    None => (None, None, false),
                },
                None => (None, None, false),
            }
        };
        let starting = self
            .start_pending
            .as_ref()
            .filter(|pending| pending.workspace == HOME_START)
            .map(StartingSession::status);
        Some(HomeFrame {
            shell: cx.entity().downgrade(),
            project,
            no_project,
            project_base,
            starting,
            mode: self.start_mode(HOME_START),
            profile_unsafe: self.profile_unsafe,
            details: self.home_details.clone(),
            composer,
        })
    }

    /// The project pill's menu, with the page's choice checked. It opens where the pointer is.
    pub(super) fn open_home_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (projects, offered) = {
            let state = self.store.read(cx).state();
            (state.projects.clone(), state.agent_no_project)
        };
        let chosen = home_choice(&projects, self.home_project.as_ref());
        let (entries, choices) = home_project_menu(&projects, chosen.as_ref(), offered);
        self.open_menu(
            entries,
            MenuTarget::HomeProject(choices),
            OverlayLayer::Dialog,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// Send: with a project, `workspace.create` in it and then `agent.start` in the new workspace; with "No project",
    /// `agent.start` alone. The chosen kind and mode go with it. A refusal gives the prompt back with the service's
    /// sentence.
    pub(super) fn start_from_home(
        &mut self,
        prompt: &str,
        composer: Entity<Composer>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(choice) = home_choice(
            &self.store.read(cx).state().projects,
            self.home_project.as_ref(),
        ) else {
            return;
        };
        let (kind, permission_mode) = match self.chosen_start(&composer, cx) {
            Ok(chosen) => chosen,
            Err(refusal) => return refuse_send(prompt, &composer, refusal, cx),
        };
        let details = self.home_details.values(cx);
        // The branch menu names a worktree, so "No project" never waits on it.
        if let (ProjectChoice::Project(_), Some(reason)) = (&choice, details.send_block()) {
            return refuse_send(prompt, &composer, reason.to_owned(), cx);
        }
        let mode = self.start_mode(HOME_START);
        self.start_pending = Some(StartingSession {
            workspace: HOME_START.to_owned(),
            kind_name: kind.name,
        });
        let options = self.chosen_options(&composer, &kind.id, cx);
        let prompt = prompt.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = match choice {
                ProjectChoice::NoProject => {
                    let params =
                        no_project_start_params(&kind.id, &prompt, permission_mode, options);
                    service.agent_start(params).await
                }
                ProjectChoice::Project(project) => {
                    match service
                        .create_workspace(create_params(&project, &prompt, &details))
                        .await
                    {
                        Ok(workspace) => {
                            let _ = shell.update(cx, |shell, cx| {
                                shell.remember_chosen_kind(&workspace.id, &kind.id, cx);
                            });
                            let params = home_start_params(
                                &workspace.id,
                                &kind.id,
                                &prompt,
                                permission_mode,
                                options,
                            );
                            service.agent_start(params).await
                        }
                        Err(refusal) => Err(refusal),
                    }
                }
            };
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_start(&prompt, composer, mode, answer, cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// Keeps the agent kind chosen for a workspace this app created, until its first session replaces its row (A97).
    pub(super) fn remember_chosen_kind(
        &mut self,
        workspace: &str,
        kind: &str,
        cx: &mut Context<Self>,
    ) {
        let update = StoreUpdate::WorkspaceKindChosen {
            workspace: workspace.to_owned(),
            kind: kind.to_owned(),
        };
        self.store
            .update(cx, |store, cx| store.apply(vec![update], cx));
    }
}
