//! The start page with no workspace (D-AG-01, J3), where "+ Create" lands: the question, the placement card with
//! the host, project and branch, and the start composer with its agent pick and access chip. Send creates a
//! workspace in the project, named from the prompt, then starts the agent in it.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, Subscription, WeakEntity, Window, div, px};
use massorca_protocol::methods::projects::{Project, ProjectKind};
use massorca_ui::components::composer::Composer;
use massorca_ui::components::menu::{MenuEntry, MenuItem};
use massorca_ui::components::popover::{
    OverlayLayer, Popover, PopoverPlacement, PopoverVariant, PopoverWidth,
};
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::components::switch::Switch;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::components::tooltip::TooltipTrigger;
use massorca_ui::style::{StateFill, Surface};
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, RADIUS_MD, SIZE_CONTROL_DEFAULT, SPACE_2, SPACE_3, SPACE_6,
    TYPE_BODY, TYPE_CAPTION, TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, Icon, IconName, IconSize, TextStyled, Theme};

use super::Shell;
use super::start_page::{
    key_hints, profile_banner, project_title_link, refuse_send, starting_line,
};
use super::start_placement::{host_pill, off_pill, pill, pill_focus, placement_card};
use super::workspace_dialogs::{MenuTarget, ProjectMenuChoice, project_menu};
use crate::model::home_start::{
    BranchPill, CHOOSE_A_PROJECT, FOLDER_NO_GIT_REASON, HOME_START, NEW_WORKTREE_REASON,
    NO_PROJECT_NEEDS_SERVICE, NO_WORKTREE_REASON, WorkspaceDetails, branch_pill, create_params,
    folder_start_workspace, home_choice, home_start_params, name_is_derived,
    no_project_branch_pill, no_project_start_params,
};
use crate::model::service_view::LOCAL_HOST;
use crate::model::start_page::{StartMode, StartingSession, TERMINAL_NOTE};
use crate::model::workspace_flow::{NO_PROJECT_LABEL, ProjectChoice};
use crate::service::StoreUpdate;
use crate::service::create_for_start;

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
    /// Whether the project is a folder, which has no worktree or branch.
    pub folder: bool,
    /// The branches the popover lists: the project's base until the service lists the project's branches.
    pub branches: Vec<String>,
    /// The branch popover's selection and whether it is open.
    pub details: HomeDetails,
}

/// The branch popover of the start page with no workspace (D-AG-01): whether it is open, the base the person picked,
/// and the search over the project's branches. Kept on the shell so a search survives redraws.
#[derive(Clone, Debug)]
pub(super) struct HomeDetails {
    pub branch_menu: bool,
    /// The branch the new worktree starts from, once picked; `None` takes the project's base.
    pub base: Option<String>,
    /// The branches a workspace of the project listed, with the project they belong to.
    pub listed: Option<(String, Vec<String>)>,
    pub search: Entity<TextField>,
    /// Redraws the popover as the search changes.
    _search_events: Rc<Subscription>,
}

impl HomeDetails {
    /// The popover starts closed, with no base picked and an empty search.
    pub(super) fn new(cx: &mut Context<Shell>) -> Self {
        let search = cx.new(|cx| TextField::new(cx).placeholder("Search branches"));
        let events = cx.subscribe(&search, |_, _, event: &TextFieldEvent, cx| {
            if matches!(event, TextFieldEvent::Changed) {
                cx.notify();
            }
        });
        Self {
            branch_menu: false,
            base: None,
            listed: None,
            search,
            _search_events: Rc::new(events),
        }
    }

    /// What Send reads from the popover: the base picked. The name comes from the prompt, never from the popover.
    fn values(&self) -> WorkspaceDetails {
        WorkspaceDetails {
            name: String::new(),
            base: self.base.clone().unwrap_or_default(),
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
    } else if frame.folder {
        BranchPill {
            label: "No Git".to_owned(),
            detail: "folder".to_owned(),
        }
    } else {
        branch_pill(&frame.details.values(), frame.project_base.as_deref())
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
        let off = frame.no_project || frame.folder;
        let focus = pill_focus("home-start-branch", window, cx);
        let trigger = if off {
            // Off with "No project" or a folder: the pill names why on hover and on keyboard focus.
            let reason = if frame.folder {
                FOLDER_NO_GIT_REASON
            } else {
                NO_WORKTREE_REASON
            };
            off_pill(
                "home-start-branch",
                IconName::GitBranch,
                labels.label,
                Some(labels.detail),
                reason,
                &theme,
                focus,
            )
            .into_any_element()
        } else {
            pill(
                "home-start-branch",
                IconName::GitBranch,
                labels.label,
                Some(labels.detail),
                true,
                &theme,
                focus,
            )
            .on_click(move |_, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.toggle_branch_menu(cx));
            })
            .into_any_element()
        };
        let body = branch_popover(
            &frame.details,
            &frame.branches,
            frame.project_base.as_deref(),
            frame.shell.clone(),
            &theme,
            cx,
        );
        let dismiss = frame.shell.clone();
        Popover::new("home-start-branch-menu", trigger)
            .open(frame.details.branch_menu && !off)
            .variant(PopoverVariant::Form)
            .width(PopoverWidth::Medium)
            .placement(PopoverPlacement::Below)
            .layer(OverlayLayer::Dialog)
            .child(body)
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

/// The branch popover (D-AG-01): the "New worktree" switch, which stays on, and the branch row, a search over the
/// project's branches that picks the base the new worktree starts from. A blank base takes the project's.
fn branch_popover(
    details: &HomeDetails,
    branches: &[String],
    project_base: Option<&str>,
    shell: WeakEntity<Shell>,
    theme: &Theme,
    cx: &App,
) -> impl IntoElement {
    let p = &theme.palette;
    let query = details.search.read(cx).text().to_lowercase();
    let picked = details
        .base
        .clone()
        .or_else(|| project_base.map(str::to_owned));
    let current = picked
        .clone()
        .unwrap_or_else(|| "the default branch".to_owned());
    let switch = TooltipTrigger::new(
        "home-start-new-worktree-reason",
        NEW_WORKTREE_REASON,
        Switch::new("home-start-new-worktree", true)
            .label("New worktree")
            .disabled(true),
    );
    let rows = branches
        .iter()
        .filter(|branch| query.is_empty() || branch.to_lowercase().contains(&query))
        .enumerate()
        .map(|(index, branch)| {
            let shell = shell.clone();
            let value = branch.clone();
            let chosen = picked.as_deref() == Some(branch.as_str());
            let row = format!("home-start-branch-row-{index}");
            div()
                .id(row.clone())
                .debug_selector(move || row.clone())
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .h(px(SIZE_CONTROL_DEFAULT))
                .px(px(SPACE_2))
                .rounded(px(RADIUS_MD))
                .text_style(TYPE_BODY)
                .text_color(p.text.primary)
                .state_fills(theme, Surface::Overlay)
                .child(branch.clone())
                .when(chosen, |row| {
                    row.child(
                        Icon::new(IconName::Check)
                            .size(IconSize::Small)
                            .color(p.accent.fg),
                    )
                })
                .on_click(move |_, _, cx| {
                    let _ =
                        shell.update(cx, |shell, cx| shell.set_home_base(Some(value.clone()), cx));
                })
        });
    div()
        .debug_selector(|| "home-start-branch-popover".to_owned())
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(
            div()
                .debug_selector(|| "home-start-new-worktree".to_owned())
                .child(switch),
        )
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(p.text.secondary)
                .child(format!("Branch: {current}")),
        )
        .child(details.search.clone())
        .child(
            div()
                .debug_selector(|| "home-start-branch-list".to_owned())
                .flex()
                .flex_col()
                .children(rows),
        )
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
        if self.home_details.branch_menu {
            self.list_home_branches(cx);
        }
        cx.notify();
    }

    /// Closes the branch pill's menu on a click outside it or Escape.
    pub(super) fn close_branch_menu(&mut self, cx: &mut Context<Self>) {
        self.home_details.branch_menu = false;
        cx.notify();
    }

    /// Lists the project's branches for the popover through one of its workspaces: branches belong to the repository,
    /// so any workspace of the project lists them all. Without a workspace the popover keeps the project's base.
    fn list_home_branches(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(ProjectChoice::Project(project)) = home_choice(
            &self.store.read(cx).state().projects,
            self.home_project.as_ref(),
        ) else {
            return;
        };
        let workspace = self
            .store
            .read(cx)
            .state()
            .workspaces
            .iter()
            .find(|workspace| workspace.project_id == project)
            .map(|workspace| workspace.id.clone());
        let Some(workspace) = workspace else {
            return;
        };
        cx.spawn(async move |shell, cx| {
            if let Ok(names) = service.git_branches(workspace).await {
                let _ = shell.update(cx, |shell, cx| {
                    shell.home_details.listed = Some((project, names));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// The branches the popover lists: the project's base first, then the branches a workspace of the project listed.
    fn home_branch_list(&self, project_base: Option<&str>, cx: &App) -> Vec<String> {
        let chosen = match home_choice(
            &self.store.read(cx).state().projects,
            self.home_project.as_ref(),
        ) {
            Some(ProjectChoice::Project(id)) => Some(id),
            _ => None,
        };
        let listed: &[String] = match (&self.home_details.listed, chosen) {
            (Some((id, names)), Some(chosen)) if *id == chosen => names.as_slice(),
            _ => &[],
        };
        let mut branches: Vec<String> = project_base.map(str::to_owned).into_iter().collect();
        for name in listed {
            if !branches.contains(name) {
                branches.push(name.clone());
            }
        }
        branches
    }

    /// Picks the branch the new worktree starts from and closes the popover; `None` takes the project's base.
    pub(super) fn set_home_base(&mut self, base: Option<String>, cx: &mut Context<Self>) {
        self.home_details.base = base;
        self.home_details.branch_menu = false;
        cx.notify();
    }

    /// The page's frame while the Canvas shows it; sample data has no service to start with.
    pub(super) fn home_frame(&mut self, cx: &mut Context<Self>) -> Option<HomeFrame> {
        self.service.as_ref()?;
        let composer = self.start_composer(HOME_START, cx);
        let (project, project_base, no_project, folder) = {
            let projects = &self.store.read(cx).state().projects;
            match home_choice(projects, self.home_project.as_ref()) {
                Some(ProjectChoice::NoProject) => {
                    (Some(NO_PROJECT_LABEL.to_owned()), None, true, false)
                }
                Some(ProjectChoice::Project(id)) => match projects.iter().find(|p| p.id == id) {
                    Some(project) => (
                        Some(project.name.clone()),
                        project.base_ref.clone(),
                        false,
                        matches!(project.kind, ProjectKind::Folder),
                    ),
                    None => (None, None, false, false),
                },
                None => (None, None, false, false),
            }
        };
        let branches = self.home_branch_list(project_base.as_deref(), cx);
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
            mode: self.start_mode(HOME_START, cx),
            profile_unsafe: self.profile_unsafe,
            folder,
            branches,
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
        let details = self.home_details.values();
        let mode = self.start_mode(HOME_START, cx);
        self.start_pending = Some(StartingSession {
            workspace: HOME_START.to_owned(),
            kind_name: kind.name,
        });
        let options = self.chosen_options(&composer, cx);
        let grid = self.start_grid(cx);
        // A name derived from the prompt may be made unique by the service; a typed name is never changed.
        let derived = name_is_derived(&details);
        let unique_offered = self.store.read(cx).state().workspace_unique_name;
        let existing_folder = match &choice {
            ProjectChoice::Project(id) => {
                let state = self.store.read(cx).state();
                state
                    .projects
                    .iter()
                    .find(|project| &project.id == id)
                    .and_then(|project| folder_start_workspace(project, &state.workspaces))
                    .map(|workspace| workspace.id.clone())
            }
            ProjectChoice::NoProject => None,
        };
        let prompt = prompt.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = match choice {
                ProjectChoice::NoProject => {
                    let params =
                        no_project_start_params(&kind.id, &prompt, permission_mode, options, grid);
                    service.agent_start(params).await
                }
                ProjectChoice::Project(project) => {
                    let started = match existing_folder {
                        Some(workspace) => Ok(workspace),
                        None => create_for_start(
                            &service,
                            create_params(&project, &prompt, &details),
                            derived,
                            unique_offered,
                        )
                        .await
                        .map(|workspace| workspace.id),
                    };
                    match started {
                        Ok(workspace) => {
                            let _ = shell.update(cx, |shell, cx| {
                                shell.remember_chosen_kind(&workspace, &kind.id, cx);
                            });
                            let params = home_start_params(
                                &workspace,
                                &kind.id,
                                &prompt,
                                permission_mode,
                                options,
                                grid,
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
