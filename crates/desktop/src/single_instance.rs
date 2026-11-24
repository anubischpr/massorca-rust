//! Raises the running window for a launch that handed its request over (PLAT-07).

use std::io;

use gpui::{App, WindowHandle};
use massorca_profile::{DesktopClaim, DesktopOwner};

use crate::shell::Shell;

/// Answers later launches by raising `window` and the app. The request reaches the handler in the profile tests;
/// the raise itself is only run by the running app.
pub fn raise_on_handoff(
    owner: DesktopOwner,
    window: WindowHandle<Shell>,
    cx: &mut App,
) -> io::Result<()> {
    let (requests, mut received) = tokio::sync::mpsc::unbounded_channel::<()>();
    owner.serve(move || {
        // The receiver lives as long as the app, so a send only fails once the app has quit.
        let _ = requests.send(());
    })?;
    cx.spawn(async move |cx| {
        while received.recv().await.is_some() {
            cx.update(|cx| {
                let _ = window.update(cx, |_, window, _| window.activate_window());
                cx.activate(true);
            });
        }
    })
    .detach();
    Ok(())
}

/// What a launch does once it has asked to claim the profile.
#[derive(Debug)]
pub enum LaunchPlan {
    /// Open the window; `Some` owns the profile's lock, `None` runs without single-instance.
    Open(Option<DesktopOwner>),
    /// The running desktop was raised; this launch ends.
    HandedOff,
}

/// Decides a launch from its claim. A failed claim never blocks opening: the message to report is returned and
/// the window opens without owning the profile.
pub fn plan_launch(claim: Option<io::Result<DesktopClaim>>) -> (LaunchPlan, Option<String>) {
    match claim {
        None => (LaunchPlan::Open(None), None),
        Some(Ok(DesktopClaim::Owner(owner))) => (LaunchPlan::Open(Some(owner)), None),
        Some(Ok(DesktopClaim::HandedOff)) => (LaunchPlan::HandedOff, None),
        Some(Err(error)) => (
            LaunchPlan::Open(None),
            Some(format!(
                "cannot claim the profile for this window, opening without single-instance: {error}"
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_claim_still_opens_the_window_and_reports() {
        let (plan, message) = plan_launch(Some(Err(io::Error::other("no answer"))));
        assert!(matches!(plan, LaunchPlan::Open(None)));
        assert!(message.expect("reported").contains("no answer"));
    }

    #[test]
    fn a_launch_without_a_profile_opens_silently() {
        let (plan, message) = plan_launch(None);
        assert!(matches!(plan, LaunchPlan::Open(None)));
        assert_eq!(message, None);
    }

    #[test]
    fn a_handoff_ends_the_launch_and_an_owner_opens_it() {
        let (plan, message) = plan_launch(Some(Ok(DesktopClaim::HandedOff)));
        assert!(matches!(plan, LaunchPlan::HandedOff));
        assert_eq!(message, None);

        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = massorca_profile::Profile::resolve_with(
            Some(temp.path().to_path_buf()),
            Some("main".to_owned()),
            |_| None,
        )
        .expect("a profile");
        let claim = massorca_profile::claim_desktop(&profile);
        let (plan, message) = plan_launch(Some(claim));
        assert!(matches!(plan, LaunchPlan::Open(Some(_))));
        assert_eq!(message, None);
    }
}
