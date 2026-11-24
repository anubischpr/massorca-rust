//! What quit does with the link to the service: close the link, wait a short bound for it to finish, and quit either
//! way. The service is never asked to stop; it is detached and outlives the app.

use std::future::Future;
use std::time::Duration;

use gpui::App;

use super::LinkCloser;

/// How long quit waits for the link to finish. Past it, quit goes on and the process ends with the link still open.
pub const QUIT_BOUND: Duration = Duration::from_secs(2);

/// Resolves `true` once `finished` resolves, or `false` when `bound` passes first; `finished` is then left unfinished.
pub async fn finish_within(bound: Duration, finished: impl Future<Output = ()>) -> bool {
    tokio::select! {
        () = finished => true,
        () = tokio::time::sleep(bound) => false,
    }
}

/// Closes `link` and waits up to `bound` for it to finish. Blocks the calling thread, so call it from outside any
/// tokio runtime, as the quit observer does.
pub fn close_link_within(link: &LinkCloser, bound: Duration) -> bool {
    link.close();
    match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime.block_on(finish_within(bound, link.finished())),
        Err(_) => false,
    }
}

/// Closes `link` when the app quits. GPUI polls the futures quit handlers return for only 200 ms, so the bounded wait
/// runs in the handler itself, before any future is returned.
pub fn close_link_on_quit(cx: &mut App, link: LinkCloser) {
    cx.on_app_quit(move |_| {
        close_link_within(&link, QUIT_BOUND);
        std::future::ready(())
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ServiceHandle;

    #[tokio::test(start_paused = true)]
    async fn a_link_that_never_finishes_is_left_behind_at_the_bound() {
        let started = tokio::time::Instant::now();
        assert!(!finish_within(QUIT_BOUND, std::future::pending()).await);
        assert_eq!(started.elapsed(), QUIT_BOUND);
    }

    #[tokio::test(start_paused = true)]
    async fn a_link_that_finishes_in_time_is_waited_for() {
        let started = tokio::time::Instant::now();
        assert!(finish_within(QUIT_BOUND, tokio::time::sleep(QUIT_BOUND / 2)).await);
        assert_eq!(started.elapsed(), QUIT_BOUND / 2);
    }

    #[tokio::test(start_paused = true)]
    async fn closing_a_link_sends_the_service_no_command() {
        let (_handle, mut script) = ServiceHandle::scripted();
        let link = script.link();
        link.close();
        // Nothing serves the scripted link, so only the bound ends the wait.
        assert!(!finish_within(QUIT_BOUND, link.finished()).await);
        assert!(script.closing_requested());
        assert!(script.next().is_none(), "the service was sent no command");
    }
}
