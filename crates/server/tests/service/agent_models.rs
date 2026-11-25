//! `agent.models` lists the models a kind's picker offers: the declared choices, then what the kind's vendor CLI lists
//! (decision 0166). No vendor CLI runs here: each test uses a kind that declares no listing, or one whose program is not
//! installed.

use massorca_protocol::methods::agents::{AgentModelsParams, ModelListingFailure, ModelSource};

use super::common::{connect, profile, start, stop, temp_home};

/// A profile kind that declares two models and a listing of a program that does not exist on any machine.
const GHOST_KIND: &str = r#"{
  "id": "ghost",
  "name": "Ghost",
  "icon": "bot",
  "program": ["massorca-no-such-agent-cli"],
  "prompt": { "mode": "argv" },
  "status": { "mechanism": "none" },
  "model": {
    "flag": "--model",
    "choices": [
      { "id": "fast", "label": "Fast", "default": true },
      { "id": "big", "label": "Big" }
    ],
    "discover": { "args": ["models"], "format": "lines" }
  }
}"#;

fn params(kind: &str) -> AgentModelsParams {
    AgentModelsParams {
        kind: kind.to_owned(),
        account: None,
        refresh: false,
    }
}

/// A profile kind with no `model` option, so it declares no models and no listing.
const PLAIN_KIND: &str = r#"{
  "id": "plain",
  "name": "Plain",
  "icon": "bot",
  "program": ["massorca-no-such-agent-cli"],
  "prompt": { "mode": "argv" },
  "status": { "mechanism": "none" }
}"#;

#[tokio::test]
async fn a_kind_that_declares_no_listing_answers_with_no_models_and_no_error() {
    let home = temp_home();
    let kinds = profile(home.path(), "default")
        .dir()
        .join("agents")
        .join("kinds");
    std::fs::create_dir_all(&kinds).expect("the kinds folder");
    std::fs::write(kinds.join("plain.json"), PLAIN_KIND).expect("a declared kind");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let models = client
        .agent_models(&params("plain"))
        .await
        .expect("the models");
    assert!(models.models.is_empty(), "{models:?}");
    assert_eq!((models.fetched_at_ms, models.error), (None, None));

    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_kind_that_is_not_enabled_is_refused_as_a_start_refuses_it() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    // A new profile lists the built-in kinds as available, not enabled.
    let refused = client.agent_models(&params("gentle-shell")).await;
    assert!(refused.is_err(), "{refused:?}");

    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_listing_whose_program_is_not_installed_keeps_the_declared_models_and_says_so() {
    let home = temp_home();
    let kinds = profile(home.path(), "default")
        .dir()
        .join("agents")
        .join("kinds");
    std::fs::create_dir_all(&kinds).expect("the kinds folder");
    std::fs::write(kinds.join("ghost.json"), GHOST_KIND).expect("a declared kind");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let models = client
        .agent_models(&params("ghost"))
        .await
        .expect("the models");
    let shape: Vec<(&str, ModelSource, bool)> = models
        .models
        .iter()
        .map(|model| (model.id.as_str(), model.source, model.default))
        .collect();
    assert_eq!(
        shape,
        [
            ("fast", ModelSource::Declared, true),
            ("big", ModelSource::Declared, false),
        ]
    );
    assert_eq!(
        models.error.map(|error| error.code),
        Some(ModelListingFailure::CliMissing)
    );
    assert_eq!(models.fetched_at_ms, None, "no listing has succeeded");

    drop(client);
    stop(running).await;
}
