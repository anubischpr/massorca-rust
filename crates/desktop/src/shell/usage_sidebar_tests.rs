//! Element tests of the usage sidebar on the rail: a ring per account with a reading, the windows and the switch in its
//! popover, the switch that turns usage on, the refresh that reads an account again, and the sentence for a service that
//! cannot report usage. A scripted handle stands in for the service, so no window opens.

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px};
use massorca_protocol::methods::config::ConfigSet;
use massorca_protocol::methods::usage::{
    AccountUsage, SETTING_ENABLED, UsageList, UsageParams, UsageState, UsageWindow, WindowKind,
};

use super::service_tests::{Rig, click, draw, rig};
use crate::model::usage::now_ms;
use crate::service::{Command, Script, StoreUpdate};

const HOUR: i64 = 3_600_000;

/// An account whose 5-hour window is at `percent`, read a minute ago.
fn reading(kind: &str, id: &str, label: &str, percent: f64) -> AccountUsage {
    let mut account = AccountUsage::pending(kind, id, label);
    account.state = UsageState::Ok;
    account.fetched_at_ms = Some(now_ms() - 60_000);
    account.windows = vec![UsageWindow {
        window_minutes: Some(300),
        used_percent: Some(percent),
        resets_at_ms: Some(now_ms() + 2 * HOUR),
        ..UsageWindow::new(WindowKind::Session)
    }];
    account
}

/// The service offers usage (or not), and holds `usage` for this computer when it is given.
fn offer_usage(rig: &Rig, cx: &mut VisualTestContext, offered: bool, usage: Option<UsageList>) {
    rig.updates
        .send(StoreUpdate::UsageOffered(offered))
        .expect("the store listens");
    if let Some(usage) = usage {
        rig.updates
            .send(StoreUpdate::UsageResync(usage))
            .expect("the store listens");
    }
    cx.run_until_parked();
    draw(cx);
}

/// Opens the usage popover of the first ring, and draws it so its rows are in the frame.
fn open_first_popover(cx: &mut VisualTestContext) {
    click(cx, "rail-usage-0");
    draw(cx);
}

/// Clicks the usage switch at the right end of its row, where the track is.
fn click_usage_switch(cx: &mut VisualTestContext) {
    draw(cx);
    let bounds = cx
        .debug_bounds("usage-switch")
        .expect("the usage switch is drawn");
    let track = point(
        bounds.origin.x + bounds.size.width - px(8.),
        bounds.center().y,
    );
    cx.simulate_click(track, Modifiers::default());
    cx.run_until_parked();
}

/// Takes the `config.set` call the switch sends.
fn expect_set_config(script: &mut Script) -> ConfigSet {
    loop {
        match script.next() {
            Some(Command::SetConfig { params, .. }) => return params,
            Some(_) => {}
            None => panic!("a config.set call was expected"),
        }
    }
}

/// Takes the `usage.refresh` call the popover sends.
fn expect_usage_refresh(script: &mut Script) -> UsageParams {
    loop {
        match script.next() {
            Some(Command::UsageRefresh { params, .. }) => return params,
            Some(_) => {}
            None => panic!("a usage.refresh call was expected"),
        }
    }
}

#[gpui::test]
fn the_rail_draws_one_ring_per_account_with_a_reading(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let usage = UsageList::on(vec![
        reading("claude", "a-1", "work", 62.0),
        reading("codex", "a-2", "home", 35.0),
    ]);
    offer_usage(&rig, cx, true, Some(usage));
    assert!(
        cx.debug_bounds("rail-usage-0").is_some(),
        "the first account has a ring"
    );
    assert!(
        cx.debug_bounds("rail-usage-1").is_some(),
        "the second account has a ring"
    );
    assert!(
        cx.debug_bounds("rail-usage-2").is_none(),
        "no ring without an account"
    );
}

#[gpui::test]
fn the_popover_lists_each_window_of_an_account_and_offers_the_switch(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    offer_usage(
        &rig,
        cx,
        true,
        Some(UsageList::on(vec![reading("claude", "a-1", "work", 62.0)])),
    );
    open_first_popover(cx);
    assert!(
        cx.debug_bounds("usage-window-0-0").is_some(),
        "the 5-hour window is listed"
    );
    assert!(
        cx.debug_bounds("usage-switch").is_some(),
        "the switch shows while usage is on"
    );
}

#[gpui::test]
fn off_offers_a_switch_that_turns_usage_on(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    offer_usage(&rig, cx, true, Some(UsageList::off()));
    assert!(cx.debug_bounds("rail-usage-1").is_none(), "off is one ring");
    open_first_popover(cx);
    assert!(
        cx.debug_bounds("usage-headline").is_some(),
        "the popover says why it is off"
    );
    click_usage_switch(cx);
    let setting = expect_set_config(&mut rig.script);
    assert_eq!(setting.key, SETTING_ENABLED);
    assert_eq!(
        setting.value.as_bool(),
        Some(true),
        "the switch turns usage on"
    );
}

#[gpui::test]
fn refresh_reads_the_account_again(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    offer_usage(
        &rig,
        cx,
        true,
        Some(UsageList::on(vec![reading("claude", "a-1", "work", 62.0)])),
    );
    open_first_popover(cx);
    click(cx, "button:usage-refresh-0");
    let params = expect_usage_refresh(&mut rig.script);
    assert_eq!(params.kind.as_deref(), Some("claude"));
    assert_eq!(
        params.account.as_deref(),
        Some("a-1"),
        "only this account is read again"
    );
}

#[gpui::test]
fn a_service_without_usage_says_it_needs_a_newer_build(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    offer_usage(&rig, cx, false, None);
    assert!(
        cx.debug_bounds("rail-usage-0").is_some(),
        "the ring says so"
    );
    open_first_popover(cx);
    assert!(
        cx.debug_bounds("usage-headline").is_some(),
        "the sentence shows in the popover"
    );
    assert!(
        cx.debug_bounds("usage-switch").is_none(),
        "an older service offers no switch"
    );
}
