//! The start page with no workspace (D-AG-01, J3), where "+ Create" lands: the question, a
//! placement row with the project, and the start composer with its agent pick and access chip.
//! Send creates a workspace in the project, named from the prompt, then starts the agent in it.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, WeakEntity, Window, div, px};
use massorca_protocol::methods::projects::Project;
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::composer::Composer;
use massorca_ui::components::menu::{MenuEntry, MenuItem};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, SPACE_2, SPACE_6, TYPE_CAPTION, TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

use super::Shell;
use super::start_page::{
    KindNotice, key_hints, kind_notice_row, profile_banner, refuse_send, starting_line,
};
use super::workspace_dialogs::{MenuTarget, ProjectMenuChoice, project_menu};
use crate::model::home_start::{
    CHOOSE_A_PROJECT, HOME_START, NO_PROJECT_NEEDS_SERVICE, create_params, home_project,
    home_start_params,
};
use crate::model::start_page::{StartMode, StartingSession, TERMINAL_NOTE};
use crate::model::workspace_flow::{NO_PROJECT_LABEL, ProjectChoice};

/// What the page draws, gathered by the shell for one frame.
pub(super) struct HomeFrame {
    pub composer: Entity<Composer>,
    pub shell: WeakEntity<Shell>,
    /// The project's name, or `None` while there is no project.
    pub project: Option<String>,
    /// The start in flight from this page, shown under the composer.
    pub starting: Option<String>,
    pub mode: StartMode,
    /// Why the chosen agent cannot start, when it cannot.
    pub kind_notice: Option<KindNotice>,
    /// The service refused because the data folder is not secured.
    pub profile_unsafe: bool,
}

/// The project menu of this page: the shared rows, with "No project" off until the service can
/// start an agent without a workspace (A95). Its choice is `None`, so it can never be chosen.
pub(super) fn home_project_menu(
    projects: &[Project],
    chosen: Option<&str>,
) -> (Vec<MenuEntry>, Vec<Option<ProjectMenuChoice>>) {
    let chosen = chosen.map(|id| ProjectChoice::Project(id.to_owned()));
    let (mut entries, mut choices) = project_menu(projects, chosen.as_ref());
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
pub(super) fn home_page(frame: HomeFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let project = {
        let shell = frame.shell.clone();
        Button::new(
            "home-start-project",
            frame
                .project
                .clone()
                .unwrap_or_else(|| CHOOSE_A_PROJECT.to_owned()),
        )
        .variant(ButtonVariant::Ghost)
        .leading_icon(IconName::Folder)
        .trailing_icon(IconName::ChevronDown)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_home_project_menu(window, cx));
        })
    };
    let composer = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(frame.composer.clone())
        .when(frame.mode == StartMode::Terminal, |column| {
            column.child(
                div()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.secondary)
                    .child(TERMINAL_NOTE),
            )
        })
        .when_some(frame.kind_notice.clone(), |column, notice| {
            column.child(kind_notice_row(notice, frame.shell.clone(), &theme))
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
        .child(
            div()
                .flex()
                .justify_center()
                .text_style(TYPE_HEADING)
                .text_color(p.text.primary)
                .child("What should we work on?"),
        )
        .child(div().flex().flex_row().justify_center().child(project))
        .when(frame.profile_unsafe, |column| {
            column.child(profile_banner())
        })
        .child(composer)
        .into_any_element()
}

impl Shell {
    /// "+ Create": the start page with no workspace, its composer focused (D-AG-01). The New
    /// workspace form stays for reference only (A88).
    pub(super) fn open_create_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model.close_session();
        self.model.select_workspace(None);
        let composer = self.start_composer(HOME_START, cx);
        composer.update(cx, |composer, cx| composer.focus(window, cx));
        cx.notify();
    }

    /// The page's frame while the Canvas shows it; sample data has no service to start with.
    pub(super) fn home_frame(&mut self, cx: &mut Context<Self>) -> Option<HomeFrame> {
        self.service.as_ref()?;
        let composer = self.start_composer(HOME_START, cx);
        let project = {
            let projects = &self.store.read(cx).state().projects;
            home_project(projects, self.home_project.as_deref()).map(|p| p.name.clone())
        };
        let starting = self
            .start_pending
            .as_ref()
            .filter(|pending| pending.workspace == HOME_START)
            .map(StartingSession::status);
        Some(HomeFrame {
            shell: cx.entity().downgrade(),
            project,
            starting,
            mode: self.start_mode(HOME_START),
            kind_notice: self.kind_notice(&composer, cx),
            profile_unsafe: self.profile_unsafe,
            composer,
        })
    }

    /// The project button's menu, with the page's project checked. It opens where the pointer is.
    pub(super) fn open_home_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let projects = self.store.read(cx).state().projects.clone();
        let chosen = home_project(&projects, self.home_project.as_deref()).map(|p| p.id.clone());
        let (entries, choices) = home_project_menu(&projects, chosen.as_deref());
        self.open_menu(
            entries,
            MenuTarget::HomeProject(choices),
            OverlayLayer::Dialog,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// Send: `workspace.create` in the page's project, then `agent.start` in the new workspace with
    /// the chosen kind and mode. Either refusal gives the prompt back with the service's sentence.
    pub(super) fn start_from_home(
        &mut self,
        prompt: &str,
        composer: Entity<Composer>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(project) = home_project(
            &self.store.read(cx).state().projects,
            self.home_project.as_deref(),
        )
        .map(|project| project.id.clone()) else {
            return;
        };
        let (kind, permission_mode) = match self.chosen_start(&composer, cx) {
            Ok(chosen) => chosen,
            Err(refusal) => return refuse_send(prompt, &composer, refusal, cx),
        };
        let mode = self.start_mode(HOME_START);
        self.start_pending = Some(StartingSession {
            workspace: HOME_START.to_owned(),
            kind_name: kind.name,
        });
        let create = create_params(&project, prompt);
        let prompt = prompt.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = match service.create_workspace(create).await {
                Ok(workspace) => {
                    let params =
                        home_start_params(&workspace.id, &kind.id, &prompt, permission_mode);
                    service.agent_start(params).await
                }
                Err(refusal) => Err(refusal),
            };
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_start(&prompt, composer, mode, answer, cx);
            });
        })
        .detach();
        cx.notify();
    }
}
