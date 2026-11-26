//! Element test of the remembered rail mode (SET-02): a press of the mode button is saved as it happens, so the next
//! start opens in the same mode. The scripted shell of app_theme_tests stands in for the service.

use gpui::TestAppContext;
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
