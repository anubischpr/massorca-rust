//! `usage.list` and `usage.refresh` (decision 0154) on a throwaway profile, from fake logs in the accounts' folders.

use std::fs;
use std::path::Path;

use massorca_client::{Client, Subscription};
use massorca_core_agents::paths::valid_account_home;
use massorca_core_store::{AgentAccountRow, Store};
use massorca_protocol::methods::config;
use massorca_protocol::methods::events::ChangeEvent;
use massorca_protocol::methods::usage::{
    self, Collection, SETTING_ENABLED, TOPIC_USAGE_CHANGED, UsageList, UsageParams, UsageState,
    WindowKind,
};
use serde_json::json;
use tokio::time::timeout;

use super::common::{REPLY_WAIT, connect, profile, start, stop, temp_home};

const CLAUDE: &str = "0195c0de-0000-7000-8000-0000000000c1";
const CODEX: &str = "0195c0de-0000-7000-8000-0000000000c2";
const OTHER: &str = "0195c0de-0000-7000-8000-0000000000c3";
const MINUTE_MS: i64 = 60_000;

fn insert_account(home: &Path, kind: &str, id: &str, created_at: i64) {
    let store = Store::open(profile(home, "default").store_path()).unwrap();
    store
        .insert_agent_account(&AgentAccountRow {
            id: id.to_owned(),
            kind: kind.to_owned(),
            label: format!("{kind} work"),
            created_at,
            identity: None,
            state: "signed_in".to_owned(),
            origin: "import".to_owned(),
        })
        .unwrap();
}

fn iso(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms).unwrap().to_string()
}

fn now() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

fn write_log(path: &Path, lines: &[serde_json::Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(path, text).unwrap();
}

/// Fake logs: one Claude reply of 150 tokens ten minutes ago, and one Codex record at 40% of its 5-hour limit.
fn write_fixtures(home: &Path) {
    let profile_dir = profile(home, "default").dir().to_path_buf();
    let claude = valid_account_home(&profile_dir, "claude", CLAUDE).unwrap();
    write_log(
        &claude.join("projects").join("-fake").join("s.jsonl"),
        &[json!({
            "type": "assistant", "timestamp": iso(now() - 10 * MINUTE_MS), "requestId": "req_1",
            "message": { "id": "msg_1", "content": [],
                         "usage": { "input_tokens": 100, "output_tokens": 50 } }
        })],
    );
    let codex = valid_account_home(&profile_dir, "codex", CODEX).unwrap();
    write_log(
        &codex
            .join("sessions")
            .join("2026")
            .join("11")
            .join("26")
            .join("rollout-a.jsonl"),
        &[json!({
            "timestamp": iso(now() - MINUTE_MS), "type": "event_msg",
            "payload": { "type": "token_count", "rate_limits": {
                "primary": { "used_percent": 40.0, "window_minutes": 300, "resets_in_seconds": 3600 }
            } }
        })],
    );
}

async fn set_enabled(client: &Client, on: bool) {
    let _: serde_json::Value = client
        .call(
            config::SET,
            &json!({ "owner": "core", "key": SETTING_ENABLED, "value": on }),
        )
        .await
        .unwrap();
}

async fn call(client: &Client, method: &str, params: &UsageParams) -> UsageList {
    client.call(method, params).await.unwrap()
}

/// Waits for a `usage.changed` event with `id`, skipping every other event.
async fn changed(events: &mut Subscription, id: &str) {
    timeout(REPLY_WAIT, async {
        loop {
            let event: ChangeEvent = events.next_as().await.unwrap().unwrap();
            if event.topic == TOPIC_USAGE_CHANGED && event.id == id {
                return;
            }
        }
    })
    .await
    .expect("the usage change is announced");
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_is_off_by_default_and_an_account_without_logs_has_no_data() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    insert_account(home.path(), "claude", CLAUDE, 1);

    for method in [usage::LIST, usage::REFRESH] {
        let answer = call(&client, method, &UsageParams::default()).await;
        assert_eq!(answer, UsageList::off(), "{method}");
    }
    set_enabled(&client, true).await;
    let answer = call(&client, usage::REFRESH, &UsageParams::default()).await;
    assert_eq!(answer.accounts[0].state, UsageState::NoData);
    assert!(answer.accounts[0].windows.is_empty());
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn readings_come_from_the_logs_are_cached_and_announced() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client.subscribe_events().await.unwrap();
    insert_account(home.path(), "claude", CLAUDE, 1);
    insert_account(home.path(), "codex", CODEX, 2);
    insert_account(home.path(), "gentle-shell", OTHER, 3);
    write_fixtures(home.path());
    set_enabled(&client, true).await;

    let refreshed = call(&client, usage::REFRESH, &UsageParams::default()).await;
    changed(&mut events, &format!("claude/{CLAUDE}")).await;
    assert_eq!(refreshed.collection, Collection::On);
    let kinds: Vec<_> = refreshed.accounts.iter().map(|row| &row.kind).collect();
    assert_eq!(kinds, ["claude", "codex"], "only kinds with a reader");

    let claude = &refreshed.accounts[0];
    assert_eq!(claude.state, UsageState::Ok, "{claude:?}");
    assert_eq!(claude.label, "claude work");
    assert_eq!(claude.windows[0].window, WindowKind::Session);
    assert_eq!(claude.windows[0].tokens, Some(150));
    let codex = &refreshed.accounts[1];
    assert_eq!(codex.state, UsageState::Ok, "{codex:?}");
    assert_eq!(codex.windows[0].used_percent, Some(40.0));
    let fetched = claude.fetched_at_ms.expect("a fetch time");
    assert!(claude.next_fetch_at_ms.is_some_and(|next| next > fetched));

    // A read answers the cache, and a refresh right after the last fetch does not read again.
    let only_claude = UsageParams {
        account: Some(CLAUDE.to_owned()),
        ..UsageParams::default()
    };
    let listed = call(&client, usage::LIST, &only_claude).await;
    assert_eq!(listed.accounts.len(), 1);
    assert_eq!(listed.accounts[0].fetched_at_ms, Some(fetched));
    let again = call(&client, usage::REFRESH, &only_claude).await;
    assert_eq!(again.accounts[0].fetched_at_ms, Some(fetched), "no refetch");

    let missing = json!({ "account": "no-such-account" });
    assert!(
        client
            .call::<_, UsageList>(usage::LIST, &missing)
            .await
            .is_err()
    );

    // Turning usage off drops what was kept and announces it.
    set_enabled(&client, false).await;
    changed(&mut events, &format!("codex/{CODEX}")).await;
    assert_eq!(
        call(&client, usage::LIST, &UsageParams::default()).await,
        UsageList::off()
    );
    stop(running).await;
}
