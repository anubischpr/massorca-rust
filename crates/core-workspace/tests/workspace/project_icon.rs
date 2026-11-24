use massorca_protocol::codes;
use massorca_protocol::methods::projects::{ProjectIcon, ProjectUpdateParams};

use crate::fixture::{Fixture, expect_code, project_add};

#[test]
fn a_project_added_with_an_icon_keeps_it_in_every_read() {
    let fx = Fixture::new();
    let mut params = project_add(&fx.repo());
    params.icon = Some(ProjectIcon::Emoji {
        emoji: " 🚀 ".to_owned(),
    });

    let added = fx.workspaces.add_project(params).expect("add project");

    let icon = Some(ProjectIcon::Emoji {
        emoji: "🚀".to_owned(),
    });
    assert_eq!(added.icon, icon);
    assert_eq!(fx.workspaces.show_project("repo").expect("show").icon, icon);
    assert_eq!(fx.workspaces.list_projects().expect("list")[0].icon, icon);
}

#[test]
fn an_invalid_icon_refuses_the_add_and_registers_nothing() {
    let fx = Fixture::new();
    let mut params = project_add(&fx.repo());
    params.icon = Some(ProjectIcon::Emoji {
        emoji: "   ".to_owned(),
    });

    expect_code(
        fx.workspaces.add_project(params),
        codes::REQUEST_INVALID_PARAMS,
    );
    assert!(fx.workspaces.list_projects().expect("list").is_empty());
}

fn update(project: &str, icon: Option<Option<ProjectIcon>>) -> ProjectUpdateParams {
    ProjectUpdateParams {
        project: project.to_owned(),
        icon,
    }
}

fn emoji(text: &str) -> ProjectIcon {
    ProjectIcon::Emoji {
        emoji: text.to_owned(),
    }
}

#[test]
fn an_update_sets_clears_or_keeps_the_icon() {
    let fx = Fixture::new();
    fx.workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    let set = fx
        .workspaces
        .update_project(update("repo", Some(Some(emoji("🚀")))))
        .expect("set the icon");
    assert_eq!(set.icon, Some(emoji("🚀")));
    assert_eq!(
        fx.workspaces.show_project("repo").expect("show").icon,
        Some(emoji("🚀"))
    );

    let kept = fx
        .workspaces
        .update_project(update("repo", None))
        .expect("an absent icon keeps it");
    assert_eq!(kept.icon, Some(emoji("🚀")));

    let cleared = fx
        .workspaces
        .update_project(update("repo", Some(None)))
        .expect("a null icon clears it");
    assert_eq!(cleared.icon, None);
    assert_eq!(fx.workspaces.list_projects().expect("list")[0].icon, None);
}

#[test]
fn an_invalid_icon_refuses_the_update_and_keeps_the_stored_one() {
    let fx = Fixture::new();
    let mut params = project_add(&fx.repo());
    params.icon = Some(emoji("🚀"));
    fx.workspaces.add_project(params).expect("add project");

    expect_code(
        fx.workspaces
            .update_project(update("repo", Some(Some(emoji("   "))))),
        codes::REQUEST_INVALID_PARAMS,
    );
    assert_eq!(
        fx.workspaces.show_project("repo").expect("show").icon,
        Some(emoji("🚀"))
    );
}

#[test]
fn updating_an_unknown_project_is_refused() {
    let fx = Fixture::new();

    expect_code(
        fx.workspaces.update_project(update("missing", Some(None))),
        codes::PROJECT_NOT_FOUND,
    );
}
