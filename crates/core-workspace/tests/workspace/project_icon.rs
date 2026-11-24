use massorca_protocol::codes;
use massorca_protocol::methods::projects::ProjectIcon;

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
