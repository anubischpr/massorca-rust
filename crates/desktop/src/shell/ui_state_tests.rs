//! Element tests of the remembered UI state (SET-02): the rail mode, the folded projects and the window's size are
//! saved as they change, and the folds come back at start. The scripted shell of app_theme_tests stands in for the
//! service.

use std::collections::BTreeSet;
use std::time::Duration;

use gpui::{TestAppContext, px, size};
use massorca_profile::Profile;

use super::app_theme_tests::{click, rig};
use crate::model::navigation::Mode;
use crate::ui_state::{self, UiState};

#[gpui::test]
fn a_mode_switch_is_saved_as_it_happens(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = Profile::resolve_with(
        Some(temp.path().to_path_buf()),
        Some("main".to_owned()),
        |_| None,
    )
    .expect("a profile");
    let (_rig, cx) = rig(cx);
    cx.update(|_, cx| ui_state::install(cx, profile.clone(), UiState::default()));

    click(cx, "rail-mode");
    cx.run_until_parked();

    assert_eq!(ui_state::load(&profile).mode, Mode::BotTeams);
}

fn temp_profile(temp: &tempfile::TempDir) -> Profile {
    Profile::resolve_with(
        Some(temp.path().to_path_buf()),
        Some("main".to_owned()),
        |_| None,
    )
    .expect("a profile")
}

#[gpui::test]
fn a_folded_project_is_saved_as_it_happens_and_opens_folded_again(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = temp_profile(&temp);
    let (rig, cx) = rig(cx);
    cx.update(|_, cx| ui_state::install(cx, profile.clone(), UiState::default()));

    rig.shell.update(cx, |shell, cx| {
        shell.toggle_project("p1");
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        ui_state::load(&profile).collapsed_projects,
        BTreeSet::from(["p1".to_owned()])
    );

    rig.shell.update(cx, |shell, cx| {
        shell.toggle_project("p1");
        cx.notify();
    });
    cx.run_until_parked();
    assert!(ui_state::load(&profile).collapsed_projects.is_empty());
}

#[gpui::test]
fn the_saved_folds_are_restored_at_start(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);

    rig.shell.update(cx, |shell, _| {
        shell.restore_collapsed_projects(BTreeSet::from(["p1".to_owned()]));
    });

    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.collapsed_projects, BTreeSet::from(["p1".to_owned()]));
    });
}

#[gpui::test]
fn a_window_resize_is_saved_once_the_window_is_still(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = temp_profile(&temp);
    let (_rig, cx) = rig(cx);
    cx.update(|_, cx| ui_state::install(cx, profile.clone(), UiState::default()));

    cx.simulate_resize(size(px(1300.), px(800.)));
    cx.simulate_resize(size(px(1320.), px(820.)));
    cx.run_until_parked();
    assert_eq!(ui_state::load(&profile).window, None, "not saved mid-drag");

    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();

    let saved = ui_state::load(&profile)
        .window
        .expect("the window layout is saved");
    assert_eq!((saved.width, saved.height), (1320, 820));
    assert!(!saved.maximized);
}
