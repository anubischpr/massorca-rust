//! A team project (D-WS-31, screens 4.4): its brief, its own Board, a link to a Workspaces project on
//! the team's host, and delete. A brief is edited in a dialog; the link is one choice on a control.

use super::super::Shell;
use super::dialogs::TeamConfirm;
use super::{board_columns, team_pill};
use crate::model::shell::HostReach;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{BoardOwner, LINK_NOT_SAVED, ProjectId, TeamId};
use gpui::{
    AnyElement, App, IntoElement, ParentElement, Styled, WeakEntity, Window, div, prelude::*, px,
};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::tokens::{SPACE_2, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_CAPTION, TYPE_HEADING};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

/// The Board area of the page is at least this tall, so an empty column keeps its height.
const BOARD_MIN_HEIGHT: f32 = 260.0;

/// The project page: brief, Board, link and delete, in the order of screen 4.4.
pub(in crate::shell) fn project_page(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    team: TeamId,
    project: ProjectId,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let Some(team_data) = model.teams().team(team) else {
        return note("This project is gone.", &theme).into_any_element();
    };
    let Some(project_data) = team_data.project(project) else {
        return note("This project is gone.", &theme).into_any_element();
    };
    let links = link_choices(model, &team_data.host);
    let (linkable, link_note) = link_state(team_data.reach, project_data.link_saved);
    let selected = project_data
        .link
        .as_ref()
        .and_then(|link| links.iter().position(|name| name == link))
        .map_or(0, |index| index + 1);
    let brief = if project_data.brief.is_empty() {
        "No brief yet.".to_owned()
    } else {
        project_data.brief.clone()
    };
    let link_control = link_control(shell, team, project, &links, selected, linkable);
    let edit_brief = {
        let shell = shell.clone();
        Button::new("project-edit-brief", "Edit brief")
            .variant(ButtonVariant::Secondary)
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.open_edit_brief(team, project, window, cx)
                });
            })
    };
    let delete = {
        let shell = shell.clone();
        let name = project_data.name.clone();
        Button::new("project-delete", "Delete project")
            .variant(ButtonVariant::Danger)
            .on_click(move |_, cx| {
                let confirm = TeamConfirm::DeleteProject {
                    team,
                    project,
                    name: name.clone(),
                    error: None,
                };
                let _ = shell.update(cx, |shell, cx| shell.open_team_confirm(confirm, cx));
            })
    };
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child(project_data.name.clone()),
        )
        .child(team_pill(&team_data.name, team, &theme));
    let brief_section = section("Brief", &theme)
        .child(body(brief, &theme))
        .child(edit_brief);
    let board_section =
        section("Board", &theme).child(div().min_h(px(BOARD_MIN_HEIGHT)).child(board_columns(
            model,
            shell,
            BoardOwner::Project(team, project),
            0.0,
            window,
            cx,
        )));
    let link_section = section("Link to a Workspaces project", &theme)
        .child(link_control)
        .child(note(link_note, &theme));
    div()
        .id("project-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(header)
        .child(brief_section)
        .child(board_section)
        .child(link_section)
        .child(div().flex().justify_end().child(delete))
        .into_any_element()
}

/// The Workspaces projects on the team's host, which a project can link to (4.4).
fn link_choices(model: &ShellModel, host: &str) -> Vec<String> {
    let mut names: Vec<String> = model
        .snapshot()
        .projects
        .iter()
        .filter(|project| project.hosts.iter().any(|item| item.name == host))
        .map(|project| project.name.clone())
        .collect();
    names.dedup();
    names
}

/// "None" first, then each linkable project. A host that cannot link disables the control.
fn link_control(
    shell: &WeakEntity<Shell>,
    team: TeamId,
    project: ProjectId,
    links: &[String],
    selected: usize,
    linkable: bool,
) -> impl IntoElement {
    let segments = std::iter::once("None")
        .chain(links.iter().map(String::as_str))
        .map(Segment::text)
        .collect();
    let choices = links.to_vec();
    let shell = shell.clone();
    SegmentedControl::new("project-link", "Link to a Workspaces project", segments)
        .selected(selected)
        .disabled(!linkable)
        .on_change(move |index, _, cx| {
            let link = match index {
                0 => None,
                index => choices.get(index - 1).cloned(),
            };
            let _ = shell.update(cx, |shell, cx| {
                shell.set_project_link(team, project, link, cx)
            });
        })
}

/// Whether a project's link control can be used, and the sentence under it. A project the service reads keeps
/// no link, so its control stays off on any host.
fn link_state(reach: HostReach, link_saved: bool) -> (bool, &'static str) {
    if !link_saved {
        return (false, LINK_NOT_SAVED);
    }
    if matches!(reach, HostReach::Connected) {
        (
            true,
            "Links the Board to a Workspaces project's tasks. Optional.",
        )
    } else {
        (
            false,
            "This host can't link projects. Connect it, or leave the link off.",
        )
    }
}

fn section(title: &'static str, theme: &Theme) -> gpui::Div {
    div().flex().flex_col().gap(px(SPACE_2)).child(
        div()
            .text_style(TYPE_CAPTION)
            .text_color(theme.palette.text.secondary)
            .child(title),
    )
}

fn body(text: String, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.primary)
        .child(text)
}

fn note(text: &'static str, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::navigation::Page;
    use crate::model::teams::{SampleTeamsSource, TeamPage};
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use massorca_ui::{Appearance, Theme as UiTheme};

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| {
            UiTheme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        })
    }

    /// Opens the sample project's page and draws it.
    fn open_project(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell
                .model
                .open_team_page(TeamPage::Project(TeamId(1), ProjectId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[gpui::test]
    fn the_project_page_links_its_board_to_a_workspaces_project(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        open_project(&shell, cx);
        shell.update(cx, |shell, cx| {
            shell.set_project_link(TeamId(1), ProjectId(1), Some("massorca-rust".into()), cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.read_with(cx, |shell, _| {
            let project = shell
                .model()
                .teams()
                .team(TeamId(1))
                .and_then(|team| team.project(ProjectId(1)))
                .unwrap();
            assert_eq!(project.link.as_deref(), Some("massorca-rust"));
        });
    }

    #[gpui::test]
    fn the_brief_saves_from_the_edit_dialog_and_closes_it(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        open_project(&shell, cx);
        shell.update_in(cx, |shell, window, cx| {
            shell.open_edit_brief(TeamId(1), ProjectId(1), window, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.update(cx, |shell, cx| shell.submit_team_dialog(cx));
        shell.read_with(cx, |shell, _| assert!(shell.team_dialog.is_none()));
    }

    #[test]
    fn a_project_the_service_reads_has_its_link_off_and_says_why() {
        assert_eq!(
            link_state(HostReach::Connected, false),
            (
                false,
                "Linking to Workspaces is not saved by the service yet."
            )
        );
        assert!(link_state(HostReach::Connected, true).0);
    }

    #[gpui::test]
    fn deleting_the_project_returns_to_its_team_board(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        open_project(&shell, cx);
        shell.update(cx, |shell, cx| {
            shell.open_team_confirm(
                TeamConfirm::DeleteProject {
                    team: TeamId(1),
                    project: ProjectId(1),
                    name: "Desarrollo onboarding".into(),
                    error: None,
                },
                cx,
            );
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.update(cx, |shell, cx| shell.confirm_team_delete(cx));
        shell.read_with(cx, |shell, _| {
            let team = shell.model().teams().team(TeamId(1)).unwrap();
            assert!(team.project(ProjectId(1)).is_none());
            assert_eq!(
                shell.model().navigation().page,
                Page::Team(TeamPage::Board(TeamId(1)))
            );
        });
    }
}
