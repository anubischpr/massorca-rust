//! `massorca usage` (decision 0154): off until `usage.enabled` is on, then one row per account read from fake Codex logs
//! in a throwaway profile.

mod harness;

use std::fs;

use harness::{Sandbox, stdout_text};
use massorca_core_agents::paths::valid_account_home;
use massorca_core_store::{AgentAccountRow, Store};

const ACCOUNT: &str = "0195c0de-0000-7000-8000-0000000000d1";

fn insert_codex_account(sandbox: &Sandbox) {
    let store = Store::open(sandbox.profile_dir("p").join("store.sqlite3")).unwrap();
    store
        .insert_agent_account(&AgentAccountRow {
            id: ACCOUNT.to_owned(),
            kind: "codex".to_owned(),
            label: "work".to_owned(),
            created_at: 1,
            identity: None,
            state: "signed_in".to_owned(),
            origin: "import".to_owned(),
        })
        .unwrap();
    let home = valid_account_home(&sandbox.profile_dir("p"), "codex", ACCOUNT).unwrap();
    let log = home
        .join("sessions")
        .join("2026")
        .join("11")
        .join("26")
        .join("rollout-a.jsonl");
    fs::create_dir_all(log.parent().unwrap()).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // The record's time only orders records; the reset an hour from now keeps its percent current.
    let line = format!(
        r#"{{"timestamp":"2026-01-01T00:00:00Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":25.0,"window_minutes":300,"resets_at":{}}}}}}}}}"#,
        now + 3600
    );
    fs::write(log, format!("{line}\n")).unwrap();
}

#[test]
fn usage_is_off_until_the_setting_is_on() {
    let sandbox = Sandbox::new();
    let off = sandbox.json_ok("p", &["usage"]);
    assert_eq!(off["collection"], "off", "{off}");
    let text = stdout_text(&sandbox.run("p", &["usage"]).success());
    assert!(
        text.contains("massorca settings set usage.enabled true"),
        "{text}"
    );
}

#[test]
fn refresh_reads_each_account_from_its_logs() {
    let sandbox = Sandbox::new();
    sandbox
        .run("p", &["settings", "set", "usage.enabled", "true"])
        .success();
    insert_codex_account(&sandbox);

    let result = sandbox.json_ok("p", &["usage", "--refresh"]);
    assert_eq!(result["collection"], "on", "{result}");
    let account = &result["accounts"][0];
    assert_eq!(account["kind"], "codex", "{result}");
    assert_eq!(account["state"], "ok", "{result}");
    assert_eq!(account["windows"][0]["used_percent"], 25.0, "{result}");

    let text = stdout_text(&sandbox.run("p", &["usage", "--kind", "codex"]).success());
    assert!(text.contains("25%"), "{text}");
    assert!(text.contains("work"), "{text}");
}
