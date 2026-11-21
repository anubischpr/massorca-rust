//! Element tests of the shell on a service store: the status item and the notice above the Canvas.
//! A scripted handle stands in for the service, so no window opens.

use super::*;
use crate::service::fixtures::{project, workspace};
use crate::service::{Command, ConnectionState, Script, StoreUpdate};
use gpui::{AppContext, TestAppContext, VisualTestContext};
use massorca_ui::{Appearance, Theme};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

struct Rig {
    shell: Entity<Shell>,
    updates: UnboundedSender<StoreUpdate>,
    script: Script,
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn connected() -> ConnectionState {
    ConnectionState::Connected {
        server: "massorca".into(),
        version: "0.1.0".into(),
    }
}

/// A shell on a service store with these projects and one workspace, connected.
fn rig<'a>(
    cx: &'a mut TestAppContext,
    projects: &[(&str, &str)],
) -> (Rig, &'a mut VisualTestContext) {
    setup(cx);
    let (updates, rx) = unbounded_channel();
    let (handle, script) = ServiceHandle::scripted();
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        Shell::with_store(store, Some(handle), None, window, cx)
    });
    updates
        .send(StoreUpdate::Connection(connected()))
        .expect("the store listens");
    updates
        .send(StoreUpdate::Resync {
            projects: projects
                .iter()
                .map(|(id, name)| project(id, name))
                .collect(),
            workspaces: vec![workspace("w1", projects[0].0, "fix-login", 1)],
        })
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    (
        Rig {
            shell,
            updates,
            script,
        },
        cx,
    )
}

#[gpui::test]
fn the_service_fills_the_sidebar_and_shows_its_status_item(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell.read_with(cx, |shell, _| {
        let projects = &shell.model().snapshot().projects;
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].hosts[0].workspaces[0].name, "fix-login");
        assert_eq!(shell.model().snapshot().service_notice, None);
    });
    assert!(cx.debug_bounds("status-service").is_some());
    assert!(
        cx.debug_bounds("service-banner").is_none(),
        "connected shows no banner"
    );
}

#[gpui::test]
fn a_service_that_cannot_be_reached_shows_the_banner_and_retry_now_reaches_the_bridge(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::Connection(ConnectionState::Unreachable {
            reason: "The service is not running.".into(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("service-banner").is_some());
    rig.shell.read_with(cx, |shell, _| {
        let notice = shell.model().snapshot().service_notice.clone();
        assert!(notice.is_some_and(|notice| notice.retry));
    });
    rig.shell.update(cx, |shell, _| shell.retry_service_now());
    assert!(matches!(rig.script.next(), Some(Command::RetryNow)));
}

#[gpui::test]
fn the_banner_goes_away_when_the_link_is_back(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::Connection(ConnectionState::Reconnecting {
            attempt: 2,
            retry_at: std::time::Instant::now(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("service-banner").is_some());
    rig.updates
        .send(StoreUpdate::Connection(connected()))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("service-banner").is_none());
}
