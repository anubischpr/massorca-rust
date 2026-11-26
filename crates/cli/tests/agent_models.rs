//! `massorca agent models <KIND>`: the models a kind's picker offers for an account, the declared choices first and then what
//! the kind's vendor CLI lists (decision 0166). The fake agent answers each listing from the token of the account home, so
//! every account has its own list and no vendor CLI runs.

mod harness;

use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use harness::{ChildGuard, Sandbox, massorca_in, stdout_text};
use serde_json::{Value, json};

/// A fake token whose account lists two models. Each account has its own identity, so an import is never taken for the
/// same login as another account's (decision 0098).
const ALPHA_TOKEN: &str = r#"{"fake":true,"email":"alpha@example.com","refresh":"fake-secret-value","listing":["alpha:Alpha","beta:Beta"]}"#;
/// A fake token of a second account, which lists another model.
const GAMMA_TOKEN: &str = r#"{"fake":true,"email":"gamma@example.com","refresh":"fake-secret-value","listing":["gamma:Gamma"]}"#;
/// A fake token whose account lists the declared model with the model id the vendor says it runs now.
const RESOLVED_TOKEN: &str = r#"{"fake":true,"email":"resolved@example.com","refresh":"fake-secret-value","listing":["declared:Opus:claude-opus-5-5","other:Other"]}"#;
/// A fake token whose account lists the declared model with no resolved model id.
const UNRESOLVED_TOKEN: &str = r#"{"fake":true,"email":"unresolved@example.com","refresh":"fake-secret-value","listing":["declared:Opus"]}"#;
/// A fake token with no listing, so its account's listing names no model.
const BARE_TOKEN: &str = r#"{"fake":true,"refresh":"fake-secret-value"}"#;
/// The stream-json control request a Claude-style listing writes to its stdin (decision 0166).
const CLAUDE_REQUEST: &str = "{\"type\":\"control_request\",\"request_id\":\"massorca-model-discovery\",\"request\":{\"subtype\":\"list_models\"}}\n";

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// The kind a declaration names: its fake program, its account home, and one declared model, `declared`.
fn kind_json(id: &str, discover: Value) -> String {
    json!({
        "id": id,
        "name": id,
        "icon": "bot",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "status": { "mechanism": "none" },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
        "accounts": {
            "login": ["login"],
            "logout": ["logout"],
            "check": {
                "args": ["whoami"],
                "signed_in_exit": 0,
                "identity_pattern": "signed in as (\\S+)"
            }
        },
        "model": {
            "flag": "--model",
            "choices": [{ "id": "declared", "label": "Declared", "default": true }],
            "discover": discover
        }
    })
    .to_string()
}

/// Declares `listed`, whose listing is one line per model, and `claude-like`, whose listing is a stream-json control
/// request answered on stdin.
fn declare_kinds(sandbox: &Sandbox) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let lines = json!({ "args": ["models"], "format": "lines" });
    fs::write(dir.join("listed.json"), kind_json("listed", lines)).expect("write the listed kind");
    let stream = json!({
        "args": ["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"],
        "stdin": CLAUDE_REQUEST,
        "format": "claude-control"
    });
    fs::write(
        dir.join("claude-like.json"),
        kind_json("claude-like", stream),
    )
    .expect("write the claude-like kind");
}

/// Starts the service with `env` set on it, and waits until it runs.
fn start_service(sandbox: &Sandbox, env: &[(&str, &str)]) -> ChildGuard {
    sandbox.track("default");
    let mut command = massorca_in(sandbox.home());
    command
        .args(["--profile", "default", "service", "run"])
        .env_remove("FAKE_AGENT_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    let guard = ChildGuard::new(command.spawn().expect("the service starts"));
    sandbox.wait_for_running("default", true);
    guard
}

/// Imports a source holding `token` as a new account of `kind`, labelled `label`.
fn import(sandbox: &Sandbox, kind: &str, label: &str, token: &str) {
    let source = sandbox.home().join(format!("source-{kind}-{label}"));
    fs::create_dir_all(&source).expect("the import source");
    fs::write(source.join("token.json"), token).expect("the fake token");
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "import",
            kind,
            "--from",
            path_text(&source),
            "--label",
            label,
        ],
    );
}

/// The id of the account of `kind` labelled `label`.
fn account_id(sandbox: &Sandbox, kind: &str, label: &str) -> String {
    let listed = sandbox.json_ok("default", &["agent", "accounts", "list", kind]);
    listed["accounts"]
        .as_array()
        .expect("an accounts array")
        .iter()
        .find(|account| account["label"] == label)
        .unwrap_or_else(|| panic!("no account labelled {label}"))["id"]
        .as_str()
        .expect("an account id")
        .to_owned()
}

/// The models of an `agent models` document, by id, in order.
fn ids(models: &Value) -> Vec<&str> {
    models["models"]
        .as_array()
        .expect("a models array")
        .iter()
        .map(|model| model["id"].as_str().expect("a model id"))
        .collect()
}

/// `agent models <kind> --account <id>` as a document, with `extra` arguments appended.
fn models_of(sandbox: &Sandbox, kind: &str, account: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "models", kind, "--account", account];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

/// How many times the fake agent's listing ran, as the counter file records it.
fn calls(counter: &Path) -> usize {
    fs::read_to_string(counter).map_or(0, |text| text.lines().count())
}

#[test]
fn the_declared_models_come_first_and_the_listed_ones_follow_for_the_account() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[]);
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");

    let models = models_of(&sandbox, "listed", &alpha, &[]);
    assert_eq!(ids(&models), ["declared", "alpha", "beta"], "{models}");
    assert_eq!(models["models"][0]["source"], "declared");
    assert_eq!(models["models"][0]["default"], true);
    assert_eq!(models["models"][1]["source"], "discovered");
    assert!(models["fetched_at_ms"].as_u64().is_some(), "{models}");
    assert!(models.get("error").is_none(), "{models}");
}

#[test]
fn a_listing_is_answered_from_the_cache_until_refresh_asks_again() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let counter = sandbox.home().join("listing-calls.txt");
    let _service = start_service(
        &sandbox,
        &[("FAKE_AGENT_MODELS_COUNT", path_text(&counter))],
    );
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");

    models_of(&sandbox, "listed", &alpha, &[]);
    models_of(&sandbox, "listed", &alpha, &[]);
    assert_eq!(
        calls(&counter),
        1,
        "the second call is served from the cache"
    );

    models_of(&sandbox, "listed", &alpha, &["--refresh"]);
    assert_eq!(calls(&counter), 2, "--refresh asks the vendor CLI again");
}

#[test]
fn each_account_lists_its_own_models() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[]);
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    import(&sandbox, "listed", "gamma", GAMMA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");
    let gamma = account_id(&sandbox, "listed", "gamma");

    assert_eq!(
        ids(&models_of(&sandbox, "listed", &alpha, &[])),
        ["declared", "alpha", "beta"]
    );
    assert_eq!(
        ids(&models_of(&sandbox, "listed", &gamma, &[])),
        ["declared", "gamma"],
        "one account's list never answers for another"
    );
}

#[test]
fn a_stream_json_listing_gets_its_request_on_stdin_and_lists_its_rows() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[]);
    import(&sandbox, "claude-like", "alpha", ALPHA_TOKEN);
    let alpha = account_id(&sandbox, "claude-like", "alpha");

    let models = models_of(&sandbox, "claude-like", &alpha, &[]);
    assert_eq!(ids(&models), ["declared", "alpha", "beta"], "{models}");
    assert_eq!(
        models["models"][1]["efforts"],
        json!([]),
        "a row that takes no effort"
    );
}

/// The label `agent kinds` gives `kind`'s model choice `id`.
fn kinds_label(sandbox: &Sandbox, kind: &str, id: &str) -> String {
    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let kind = kinds["kinds"]
        .as_array()
        .expect("a kinds array")
        .iter()
        .find(|entry| entry["id"] == kind)
        .unwrap_or_else(|| panic!("no kind {kind}: {kinds}"));
    kind["model"]["choices"]
        .as_array()
        .expect("a choices array")
        .iter()
        .find(|choice| choice["id"] == id)
        .unwrap_or_else(|| panic!("no choice {id}: {kind}"))["label"]
        .as_str()
        .expect("a label")
        .to_owned()
}

#[test]
fn a_model_the_vendor_resolves_reads_as_its_version_in_the_models_and_the_kinds() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[]);
    import(&sandbox, "claude-like", "resolved", RESOLVED_TOKEN);
    let resolved = account_id(&sandbox, "claude-like", "resolved");
    assert_eq!(
        kinds_label(&sandbox, "claude-like", "declared"),
        "Declared",
        "nothing is listed yet, so the declared label stands"
    );

    let models = models_of(&sandbox, "claude-like", &resolved, &[]);
    assert_eq!(models["models"][0]["label"], "Opus 5.5", "{models}");
    assert_eq!(models["models"][1]["label"], "Other", "{models}");
    assert_eq!(
        kinds_label(&sandbox, "claude-like", "declared"),
        "Opus 5.5",
        "agent kinds reads the catalog's last listing for the active account"
    );
}

#[test]
fn a_model_the_vendor_does_not_resolve_keeps_its_declared_label() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[]);
    import(&sandbox, "claude-like", "unresolved", UNRESOLVED_TOKEN);
    let unresolved = account_id(&sandbox, "claude-like", "unresolved");

    let models = models_of(&sandbox, "claude-like", &unresolved, &[]);
    assert_eq!(models["models"][0]["label"], "Declared", "{models}");
    assert_eq!(kinds_label(&sandbox, "claude-like", "declared"), "Declared");
}

#[test]
fn a_listing_that_names_no_model_reports_it_and_keeps_the_declared_models() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[]);
    import(&sandbox, "listed", "bare", BARE_TOKEN);
    let bare = account_id(&sandbox, "listed", "bare");

    let models = models_of(&sandbox, "listed", &bare, &[]);
    assert_eq!(ids(&models), ["declared"], "{models}");
    assert_eq!(models["error"]["code"], "no_models", "{models}");
    assert!(models.get("fetched_at_ms").is_none(), "{models}");
}

#[test]
fn a_vendor_cli_that_fails_reports_failed_and_keeps_the_declared_models() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[("FAKE_AGENT_MODELS_FAIL", "1")]);
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");

    let models = models_of(&sandbox, "listed", &alpha, &[]);
    assert_eq!(ids(&models), ["declared"], "{models}");
    assert_eq!(models["error"]["code"], "failed", "{models}");
}

#[test]
fn a_vendor_cli_that_hangs_is_stopped_at_the_time_limit() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(
        &sandbox,
        &[
            ("FAKE_AGENT_MODELS_HANG", "1"),
            ("MASSORCA_MODEL_LISTING_TIMEOUT_MS", "500"),
        ],
    );
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");

    let started = Instant::now();
    let models = models_of(&sandbox, "listed", &alpha, &[]);
    assert_eq!(models["error"]["code"], "timed_out", "{models}");
    assert_eq!(ids(&models), ["declared"], "{models}");
    assert!(
        started.elapsed().as_secs() < 60,
        "the hung CLI is stopped at the limit, not waited for"
    );
}

#[test]
fn the_text_output_lists_each_model_with_its_default_and_its_source() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let _service = start_service(&sandbox, &[]);
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");

    let outcome = sandbox.run(
        "default",
        &["agent", "models", "listed", "--account", alpha.as_str()],
    );
    let text = stdout_text(&outcome);
    assert!(
        text.contains("declared  Declared  default  declared"),
        "{text}"
    );
    assert!(text.contains("alpha  Alpha  listed"), "{text}");
    assert!(text.contains("beta  Beta  listed"), "{text}");
}

/// The same login as `ALPHA_TOKEN` (the same identity), now listing another model. Importing it updates the account
/// rather than adding one (decision 0098).
const ALPHA_UPDATED_TOKEN: &str = r#"{"fake":true,"email":"alpha@example.com","refresh":"fake-secret-value","listing":["delta:Delta"]}"#;

/// `agent models <KIND>` without an account: the kind's active account, as a document.
fn default_models(sandbox: &Sandbox, kind: &str) -> Value {
    sandbox.json_ok("default", &["agent", "models", kind])
}

#[test]
fn an_import_that_updates_an_account_drops_its_listing() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let counter = sandbox.home().join("update-calls.txt");
    let _service = start_service(
        &sandbox,
        &[("FAKE_AGENT_MODELS_COUNT", path_text(&counter))],
    );
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");
    assert_eq!(
        ids(&models_of(&sandbox, "listed", &alpha, &[])),
        ["declared", "alpha", "beta"]
    );
    assert_eq!(calls(&counter), 1);

    import(&sandbox, "listed", "alpha", ALPHA_UPDATED_TOKEN);
    assert_eq!(
        account_id(&sandbox, "listed", "alpha"),
        alpha,
        "the updated account keeps its id"
    );
    assert_eq!(
        ids(&models_of(&sandbox, "listed", &alpha, &[])),
        ["declared", "delta"]
    );
    assert_eq!(calls(&counter), 2, "the updated login is listed again");
}

#[test]
fn a_change_of_the_active_account_drops_the_default_listing() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox);
    let counter = sandbox.home().join("default-calls.txt");
    let _service = start_service(
        &sandbox,
        &[("FAKE_AGENT_MODELS_COUNT", path_text(&counter))],
    );
    import(&sandbox, "listed", "alpha", ALPHA_TOKEN);
    import(&sandbox, "listed", "gamma", GAMMA_TOKEN);
    let alpha = account_id(&sandbox, "listed", "alpha");
    let gamma = account_id(&sandbox, "listed", "gamma");

    // The first imported account is the active one, so a request without an account lists it.
    assert_eq!(
        ids(&default_models(&sandbox, "listed")),
        ["declared", "alpha", "beta"]
    );
    assert_eq!(calls(&counter), 1);

    // Activating gamma and then alpha again changes the active account twice; the default listing runs again.
    sandbox.json_ok(
        "default",
        &["agent", "accounts", "activate", "listed", gamma.as_str()],
    );
    sandbox.json_ok(
        "default",
        &["agent", "accounts", "activate", "listed", alpha.as_str()],
    );
    assert_eq!(
        ids(&default_models(&sandbox, "listed")),
        ["declared", "alpha", "beta"]
    );
    assert_eq!(
        calls(&counter),
        2,
        "the active account changed, so the default listing ran again"
    );
}
