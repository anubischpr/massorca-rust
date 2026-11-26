//! The first-prompt rename (decision 0118): the first prompt of a session names its generated workspace, once. The title
//! comes from the kind's one-shot form (decision 0167) and the rename from `workspace.rename` (decision 0111). The
//! decision is a table in [`Titles`]; the title run happens in the background, so no prompt waits for it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use massorca_core_agents::oneshot::{TitleRequest, title_timeout};
use massorca_core_store::{WorkspaceKind, WorkspaceState};
use massorca_protocol::methods::agents::AgentSession;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_WORKSPACE_UPDATED};
use massorca_protocol::methods::workspaces::WorkspaceRenameParams;
use massorca_protocol::{ErrorBody, codes};

use crate::context::{AppState, RequestCtx};
use crate::features::agents::on_agents;
use crate::features::workspace_domain::{core_setting, for_mutation_in};
use crate::store_blocking;

/// The setting that turns the rename off (decision 0118).
const SETTING: &str = "workspaces.rename_from_first_prompt";

/// Where a workspace stands in the first-prompt rename.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Stage {
    /// A title run is in flight; `retry` says it is the session's one retry.
    Pending { session: String, retry: bool },
    /// A run failed for `session`, whose next prompt may retry once.
    Failed { session: String },
    /// Decided: renamed, refused for good, or left alone. No later prompt renames it.
    Settled,
}

/// A session that may name its workspace, as the service last saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Known {
    workspace: String,
    kind: String,
    model: Option<String>,
    prompted: bool,
}

/// A title run that one prompt starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    session: String,
    workspace: String,
    kind: String,
    model: Option<String>,
    prompt: String,
    retry: bool,
}

/// What a prompt does to the title of its workspace.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// The session is not tracked: it is unknown, or a delegated child.
    Untracked,
    /// The prompt starts a title run.
    Start(Run),
    /// Nothing starts. `first` says the prompt was its session's first; `busy` says a run is still deciding the name.
    Held {
        first: bool,
        workspace: String,
        busy: bool,
    },
}

/// The decision table: which sessions may name a workspace, and where each workspace stands.
#[derive(Debug, Default)]
pub(crate) struct Titles {
    sessions: HashMap<String, Known>,
    stages: HashMap<String, Stage>,
}

impl Titles {
    /// Records a session that may name its workspace; a delegated child never does. `model` replaces the known one when
    /// given. `seen` says the session's prompts came before this run, so its next prompt is not a first one.
    fn learn(
        &mut self,
        id: &str,
        workspace: &str,
        kind: &str,
        child: bool,
        model: Option<String>,
        seen: bool,
    ) {
        if child {
            return;
        }
        let known = self.sessions.entry(id.to_owned()).or_insert_with(|| Known {
            workspace: workspace.to_owned(),
            kind: kind.to_owned(),
            model: None,
            prompted: seen,
        });
        if model.is_some() {
            known.model = model;
        }
    }

    /// Decides a prompt delivered to `session`: the first prompt of a workspace nobody decided yet starts its run, and
    /// the one retry after a failed run starts on the same session's next prompt.
    fn prompt(&mut self, session: &str, prompt: &str) -> Step {
        let Some(known) = self.sessions.get_mut(session) else {
            return Step::Untracked;
        };
        let first = !known.prompted;
        known.prompted = true;
        let workspace = known.workspace.clone();
        let start = |retry| {
            Step::Start(Run {
                session: session.to_owned(),
                workspace: workspace.clone(),
                kind: known.kind.clone(),
                model: known.model.clone(),
                prompt: prompt.to_owned(),
                retry,
            })
        };
        match self.stages.get(&workspace) {
            None if first => {
                self.stages.insert(
                    workspace.clone(),
                    Stage::Pending {
                        session: session.to_owned(),
                        retry: false,
                    },
                );
                start(false)
            }
            Some(Stage::Failed { session: failed }) if failed == session => {
                self.stages.insert(
                    workspace.clone(),
                    Stage::Pending {
                        session: session.to_owned(),
                        retry: true,
                    },
                );
                start(true)
            }
            stage => Step::Held {
                first,
                busy: matches!(stage, Some(Stage::Pending { .. })),
                workspace,
            },
        }
    }

    /// Records where `workspace` stands after a run.
    fn settle(&mut self, workspace: &str, stage: Stage) {
        self.stages.insert(workspace.to_owned(), stage);
    }
}

/// The title table behind a lock, held only while a prompt is decided.
#[derive(Debug, Default)]
pub(crate) struct TitleBook(Mutex<Titles>);

impl TitleBook {
    fn with<T>(&self, call: impl FnOnce(&mut Titles) -> T) -> T {
        let mut titles = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        call(&mut titles)
    }
}

/// Records a started session and decides its first prompt, when the start carries one.
pub(crate) fn note_started(
    state: &Arc<AppState>,
    session: &AgentSession,
    model: Option<String>,
    prompt: Option<&str>,
) {
    let step = state.first_prompt_titles.with(|titles| {
        titles.learn(
            &session.id,
            &session.workspace_id,
            &session.kind,
            session.parent_id.is_some(),
            model,
            false,
        );
        prompt.map(|text| titles.prompt(&session.id, text))
    });
    if let Some(step) = step {
        act(state, step);
    }
}

/// Decides a prompt the service just delivered to the session `selector` names.
pub(crate) async fn note_prompt(ctx: &RequestCtx, selector: String, prompt: String) {
    let Ok(session) = on_agents(ctx, move |agents| agents.show(&selector)).await else {
        return;
    };
    let step = ctx.state.first_prompt_titles.with(|titles| {
        // A session this run did not start predates it, so its next prompt is not a first one.
        titles.learn(
            &session.id,
            &session.workspace_id,
            &session.kind,
            session.parent_id.is_some(),
            None,
            true,
        );
        titles.prompt(&session.id, &prompt)
    });
    act(&ctx.state, step);
}

/// Logs a held prompt that is its session's first, and starts a title run in the background.
fn act(state: &Arc<AppState>, step: Step) {
    match step {
        Step::Untracked => {}
        Step::Held { first: false, .. } => {}
        Step::Held {
            first: true,
            workspace,
            busy,
        } => {
            if busy {
                tracing::info!(
                    "first-prompt rename of workspace {workspace} left its name: an earlier prompt is still deciding it"
                );
            } else {
                tracing::info!(
                    "first-prompt rename of workspace {workspace} left its name: an earlier prompt already decided it"
                );
            }
        }
        Step::Start(run) => {
            tokio::spawn(title_run(Arc::clone(state), run));
        }
    }
}

/// What one title run did to its workspace's name.
enum Outcome {
    Renamed(String),
    /// A definitive refusal or a setting that is off: no later prompt renames the workspace.
    Settled(String),
    /// A failure that the session's next prompt may retry once.
    Failed(String),
}

/// One title run: names the workspace or leaves it, logs the outcome and records where the workspace stands.
async fn title_run(state: Arc<AppState>, run: Run) {
    let outcome = name_from_prompt(&state, &run).await;
    let workspace = run.workspace.as_str();
    let stage = match outcome {
        Outcome::Renamed(name) => {
            tracing::info!("first-prompt rename of workspace {workspace} named it {name}");
            Stage::Settled
        }
        Outcome::Settled(reason) => {
            tracing::info!("first-prompt rename of workspace {workspace} left its name: {reason}");
            Stage::Settled
        }
        Outcome::Failed(code) if run.retry => {
            tracing::warn!(
                "first-prompt rename of workspace {workspace} failed again with {code}; it is not retried"
            );
            Stage::Settled
        }
        Outcome::Failed(code) => {
            tracing::warn!(
                "first-prompt rename of workspace {workspace} failed with {code}; the next prompt of session {} retries once",
                run.session
            );
            Stage::Failed {
                session: run.session.clone(),
            }
        }
    };
    state
        .first_prompt_titles
        .with(|titles| titles.settle(workspace, stage));
}

/// Asks the kind for a title and renames the workspace to it, as `workspace.rename` does (decision 0111).
async fn name_from_prompt(state: &Arc<AppState>, run: &Run) -> Outcome {
    match core_setting(state, SETTING).await {
        Ok(value) if value.as_bool() == Some(false) => {
            return Outcome::Settled("the setting is off".to_owned());
        }
        Ok(_) => {}
        Err(error) => return Outcome::Failed(error.code),
    }
    let id = run.workspace.clone();
    let row = match store_blocking::run(state, move |store| store.get_workspace(&id)).await {
        Ok(Some(row)) => row,
        Ok(None) => return Outcome::Settled("the workspace is gone".to_owned()),
        Err(error) => return Outcome::Failed(error.code),
    };
    if !row.name_generated {
        return Outcome::Settled("a name a person gave".to_owned());
    }
    if row.state == WorkspaceState::Removing {
        return Outcome::Settled("the workspace is being removed".to_owned());
    }
    let expected_branch = match row.kind {
        WorkspaceKind::Worktree => row.branch.clone(),
        WorkspaceKind::Folder => None,
    };
    let request = TitleRequest {
        kind: run.kind.clone(),
        account: None,
        model: run.model.clone(),
        prompt: run.prompt.clone(),
        reply: None,
    };
    let agents = state.agents.clone();
    let slug =
        match tokio::task::spawn_blocking(move || agents.generate_title(request, title_timeout()))
            .await
        {
            Ok(Ok(slug)) => slug,
            Ok(Err(error)) => return Outcome::Failed(ErrorBody::from(error).code),
            Err(_) => return Outcome::Failed(codes::INTERNAL.to_owned()),
        };
    let domain = match for_mutation_in(state).await {
        Ok(domain) => domain,
        Err(error) => return Outcome::Failed(error.code),
    };
    let params = WorkspaceRenameParams {
        workspace: run.workspace.clone(),
        name: slug,
        from_generated: true,
        expected_branch,
    };
    match tokio::task::spawn_blocking(move || domain.rename_workspace(params)).await {
        Ok(Ok(renamed)) => {
            state.events.publish(ChangeEvent {
                topic: TOPIC_WORKSPACE_UPDATED.to_owned(),
                id: renamed.id.clone(),
            });
            Outcome::Renamed(renamed.name)
        }
        Ok(Err(error)) => match error.code() {
            code @ (codes::WORKSPACE_NAME_NOT_GENERATED
            | codes::WORKSPACE_BRANCH_PUBLISHED
            | codes::WORKSPACE_BRANCH_CHANGED) => Outcome::Settled(code.to_owned()),
            code => Outcome::Failed(code.to_owned()),
        },
        Err(_) => Outcome::Failed(codes::INTERNAL.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learn(titles: &mut Titles, session: &str, workspace: &str) {
        titles.learn(session, workspace, "fake", false, None, false);
    }

    fn started(step: &Step) -> bool {
        matches!(step, Step::Start(_))
    }

    #[test]
    fn the_first_prompt_of_an_undecided_workspace_starts_its_run() {
        let mut titles = Titles::default();
        learn(&mut titles, "s1", "w1");
        assert!(started(&titles.prompt("s1", "Fix the bug")));
    }

    #[test]
    fn a_later_prompt_of_the_same_session_starts_nothing_and_is_not_first() {
        let mut titles = Titles::default();
        learn(&mut titles, "s1", "w1");
        titles.prompt("s1", "Fix the bug");
        assert_eq!(
            titles.prompt("s1", "again"),
            Step::Held {
                first: false,
                workspace: "w1".to_owned(),
                busy: true,
            }
        );
    }

    #[test]
    fn a_second_session_waits_for_a_run_in_flight() {
        let mut titles = Titles::default();
        learn(&mut titles, "s1", "w1");
        learn(&mut titles, "s2", "w1");
        titles.prompt("s1", "Fix the bug");
        assert_eq!(
            titles.prompt("s2", "Other"),
            Step::Held {
                first: true,
                workspace: "w1".to_owned(),
                busy: true,
            }
        );
    }

    #[test]
    fn a_settled_workspace_is_never_named_again() {
        let mut titles = Titles::default();
        learn(&mut titles, "s1", "w1");
        titles.prompt("s1", "Fix the bug");
        titles.settle("w1", Stage::Settled);
        learn(&mut titles, "s2", "w1");
        assert_eq!(
            titles.prompt("s2", "Other"),
            Step::Held {
                first: true,
                workspace: "w1".to_owned(),
                busy: false,
            }
        );
    }

    #[test]
    fn a_failed_run_retries_once_on_its_own_sessions_next_prompt_only() {
        let mut titles = Titles::default();
        learn(&mut titles, "s1", "w1");
        learn(&mut titles, "s2", "w1");
        titles.prompt("s1", "Fix the bug");
        titles.settle(
            "w1",
            Stage::Failed {
                session: "s1".to_owned(),
            },
        );
        assert!(matches!(
            titles.prompt("s2", "Other"),
            Step::Held { busy: false, .. }
        ));
        match titles.prompt("s1", "Retry") {
            Step::Start(run) => assert!(run.retry, "{run:?}"),
            other => panic!("the retry should start, got {other:?}"),
        }
        assert!(matches!(
            titles.prompt("s1", "Again"),
            Step::Held { busy: true, .. }
        ));
    }

    #[test]
    fn a_failed_retry_settles_the_workspace() {
        let mut titles = Titles::default();
        learn(&mut titles, "s1", "w1");
        titles.prompt("s1", "Fix the bug");
        titles.settle(
            "w1",
            Stage::Failed {
                session: "s1".to_owned(),
            },
        );
        titles.prompt("s1", "Retry");
        titles.settle("w1", Stage::Settled);
        assert!(matches!(
            titles.prompt("s1", "Third"),
            Step::Held { busy: false, .. }
        ));
    }

    #[test]
    fn a_session_from_before_this_run_never_starts_a_run() {
        let mut titles = Titles::default();
        titles.learn("old", "w1", "fake", false, None, true);
        assert_eq!(
            titles.prompt("old", "Fix the bug"),
            Step::Held {
                first: false,
                workspace: "w1".to_owned(),
                busy: false,
            }
        );
    }

    #[test]
    fn a_delegated_child_is_never_tracked() {
        let mut titles = Titles::default();
        titles.learn("child", "w1", "fake", true, None, false);
        assert_eq!(titles.prompt("child", "Do it"), Step::Untracked);
    }

    #[test]
    fn a_model_given_at_start_is_kept_when_a_later_session_read_has_none() {
        let mut titles = Titles::default();
        titles.learn("s1", "w1", "fake", false, Some("big".to_owned()), false);
        titles.learn("s1", "w1", "fake", false, None, true);
        match titles.prompt("s1", "Fix") {
            Step::Start(run) => assert_eq!(run.model.as_deref(), Some("big")),
            other => panic!("the first prompt should start, got {other:?}"),
        }
    }
}
