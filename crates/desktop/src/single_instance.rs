//! Raises the running window for a launch that handed its request over (PLAT-07).

use std::io;

use gpui::{App, WindowHandle};
use massorca_profile::DesktopOwner;

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
