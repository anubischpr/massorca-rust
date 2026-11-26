//! Element tests of the app theme on a scripted service: the rail toggle asks the service for the
//! other appearance, the theme follows what the store holds, and a refused write keeps the theme and
//! shows the service's sentence. A scripted handle stands in for the service and no window opens.

use super::*;
use crate::model::app_theme::ThemeChoice;
use crate::model::marketplace::ServiceMarketplace;
use crate::model::teams::{TeamsSource, UnavailableTeams};
use crate::service::{Command, ConnectionState, Refusal, Script, StoreUpdate};
use gpui::{AppContext, Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::methods::config::{ConfigSet, ConfigValue};
use massorca_ui::{ActiveTheme, Appearance, Theme};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

type ConfigSetCall = (ConfigSet, oneshot::Sender<Result<ConfigValue, Refusal>>);

/// A connected shell on a scripted service, shared with the other element tests that need one.
pub(super) struct Rig {
    pub(super) shell: Entity<Shell>,
    pub(super) updates: UnboundedSender<StoreUpdate>,
    pub(super) script: Script,
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

pub(super) fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Clicks the centre of the element with this debug selector, after a fresh draw.
pub(super) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    draw(cx);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
}

fn theme(cx: &mut VisualTestContext) -> Appearance {
    cx.update(|_, cx| cx.theme().appearance)
}

/// A connected shell on a scripted service. Its theme starts dark, and no setting is listed yet.
pub(super) fn rig(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    setup(cx);
    let (updates, rx) = unbounded_channel();
    let (handle, script) = ServiceHandle::scripted();
    script.set_connection(ConnectionState::Connected {
        server: "massorca".into(),
        version: "0.1.0".into(),
    });
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        Shell::with_store(
            store,
            Some(handle),
            None,
            UnavailableTeams.teams(),
            Box::new(ServiceMarketplace::default()),
            window,
            cx,
        )
    });
    updates
        .send(StoreUpdate::Connection(ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        }))
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

/// The `appearance.theme` value the service reports, as a change event's read carries it.
fn stored_theme(word: &str) -> ConfigValue {
    ConfigValue {
        owner: "core".into(),
        key: "appearance.theme".into(),
        value: word.into(),
        ..Default::default()
    }
}

/// The next `config.set` the shell sends once its tasks have run, with the reply that answers it.
fn config_set_call(cx: &mut VisualTestContext, script: &mut Script) -> Option<ConfigSetCall> {
    cx.run_until_parked();
    while let Some(command) = script.next() {
        if let Command::SetConfig { params, reply } = command {
            return Some((params, reply));
        }
    }
    None
}

#[gpui::test]
fn the_rail_toggle_sends_config_set_for_the_other_appearance(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx);
    click(cx, "rail-theme");
    let (params, _reply) =
        config_set_call(cx, &mut rig.script).expect("the toggle asks the service for a value");
    assert_eq!(params.key, "appearance.theme");
    assert_eq!(params.value.as_str(), Some("light"));
    assert_eq!(
        theme(cx),
        Appearance::Dark,
        "the theme waits for the service"
    );
}

#[gpui::test]
fn a_changed_setting_flips_the_theme_and_back(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    rig.updates
        .send(StoreUpdate::ConfigChanged(stored_theme("light")))
        .expect("the store listens");
    cx.run_until_parked();
    assert_eq!(theme(cx), Appearance::Light);
    rig.updates
        .send(StoreUpdate::ConfigChanged(stored_theme("dark")))
        .expect("the store listens");
    cx.run_until_parked();
    assert_eq!(theme(cx), Appearance::Dark);
}

#[gpui::test]
fn a_refused_write_keeps_the_theme_and_shows_the_services_sentence(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx);
    click(cx, "rail-theme");
    let (_params, reply) =
        config_set_call(cx, &mut rig.script).expect("the toggle asks the service for a value");
    let refusal = Refusal::Unavailable {
        message: "The service is not connected.".into(),
    };
    let sentence = refusal.sentence();
    let _ = reply.send(Err(refusal));
    cx.run_until_parked();
    draw(cx);
    assert_eq!(theme(cx), Appearance::Dark);
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.theme_notice.clone());
    assert_eq!(notice, Some(sentence));
}

#[gpui::test]
fn the_settings_row_writes_the_chosen_word(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.save_app_theme(ThemeChoice::System, cx)
    });
    let (params, _reply) =
        config_set_call(cx, &mut rig.script).expect("the row asks the service for a value");
    assert_eq!(params.key, "appearance.theme");
    assert_eq!(params.value.as_str(), Some("system"));
}
