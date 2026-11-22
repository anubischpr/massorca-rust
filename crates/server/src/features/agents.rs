//! The agent methods (capability `agents/1`, `docs/architecture/agents.md`). The rules live in `core-agents`;
//! handlers run its blocking calls on the blocking pool. `core-agents` announces every status change; the service
//! turns each into `agent.changed` on the event bus and an item for `agent.watch` (0052).

use std::path::Path;

use massorca_core_agents::{AgentError, Agents, Change, MergeResult, RepoOps};
use massorca_core_git::{Git, GitError, MergeOutcome};
use massorca_core_workspace::DomainError;
use massorca_protocol::methods::agents::{
    self, AccountSummary, AccountsActivateParams, AccountsAddParams, AccountsAdded,
    AccountsCheckParams, AccountsImportParams, AccountsImported, AccountsList, AccountsListParams,
    AccountsRemoveParams, AccountsRemoved, AgentBringBackParams, AgentDelegateParams,
    AgentDelegated, AgentHookParams, AgentKinds, AgentListParams, AgentPromptParams, AgentSession,
    AgentSessionList, AgentSessionRef, AgentStartParams, AgentStopParams, AgentTree,
    AgentWatchEvent, AgentWatchParams, BringBackResult, CredentialsImportParams,
    CredentialsImported, CredentialsKindRef, CredentialsRemoved, CredentialsShown, PromptReceipt,
};
use massorca_protocol::methods::git::status::GitOperation;
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::{ErrorBody, capabilities, codes};
use tokio::sync::{broadcast, mpsc};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::method::{StreamSource, encode};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let agents_cap = capabilities::AGENTS;
    Ok(vec![
        registries.capabilities.register(agents_cap, Owner::Core)?,
        methods.register(
            Owner::Core,
            agents::KINDS,
            agents_cap,
            Permission::Read,
            kinds,
        )?,
        methods.register(
            Owner::Core,
            agents::START,
            agents_cap,
            Permission::Workspace,
            start,
        )?,
        methods.register(
            Owner::Core,
            agents::LIST,
            agents_cap,
            Permission::Read,
            list,
        )?,
        methods.register(
            Owner::Core,
            agents::SHOW,
            agents_cap,
            Permission::Read,
            show,
        )?,
        methods.register(
            Owner::Core,
            agents::STOP,
            agents_cap,
            Permission::Workspace,
            stop,
        )?,
        methods.register(
            Owner::Core,
            agents::PROMPT,
            agents_cap,
            Permission::Workspace,
            prompt,
        )?,
        methods.register(
            Owner::Core,
            agents::RESUME,
            agents_cap,
            Permission::Workspace,
            resume,
        )?,
        methods.register(
            Owner::Core,
            agents::FORGET,
            agents_cap,
            Permission::Workspace,
            forget,
        )?,
        methods.register(
            Owner::Core,
            agents::HOOK,
            agents_cap,
            Permission::Workspace,
            hook,
        )?,
        methods.register(
            Owner::Core,
            agents::DELEGATE,
            agents_cap,
            Permission::Workspace,
            delegate,
        )?,
        methods.register(
            Owner::Core,
            agents::TREE,
            agents_cap,
            Permission::Read,
            tree,
        )?,
        methods.register(
            Owner::Core,
            agents::BRING_BACK,
            agents_cap,
            Permission::Workspace,
            bring_back,
        )?,
        methods.register_stream(
            Owner::Core,
            agents::WATCH,
            agents_cap,
            Permission::Read,
            watch,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_IMPORT,
            agents_cap,
            Permission::Workspace,
            credentials_import,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_SHOW,
            agents_cap,
            Permission::Read,
            credentials_show,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_REMOVE,
            agents_cap,
            Permission::Workspace,
            credentials_remove,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_LIST,
            agents_cap,
            Permission::Read,
            accounts_list,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_ADD,
            agents_cap,
            Permission::Workspace,
            accounts_add,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_IMPORT,
            agents_cap,
            Permission::Workspace,
            accounts_import,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_ACTIVATE,
            agents_cap,
            Permission::Workspace,
            accounts_activate,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_REMOVE,
            agents_cap,
            Permission::Workspace,
            accounts_remove,
        )?,
        methods.register(
            Owner::Core,
            agents::ACCOUNTS_CHECK,
            agents_cap,
            Permission::Workspace,
            accounts_check,
        )?,
    ])
}

/// Items queued for one watcher before its forwarder waits for the connection to catch up.
const WATCH_QUEUE_CAPACITY: usize = 64;

/// Every declared kind, read again from the profile on each call, with programs resolved on the service's `PATH`.
async fn kinds(ctx: RequestCtx, _params: NoParams) -> Result<AgentKinds, ErrorBody> {
    on_agents(&ctx, |agents| Ok(agents.kinds())).await
}

async fn start(ctx: RequestCtx, params: AgentStartParams) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.start(params)).await
}

async fn list(ctx: RequestCtx, params: AgentListParams) -> Result<AgentSessionList, ErrorBody> {
    let sessions = on_agents(&ctx, move |agents| agents.list(params.workspace.as_deref())).await?;
    Ok(AgentSessionList { sessions })
}

async fn show(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.show(&params.session)).await
}

async fn stop(ctx: RequestCtx, params: AgentStopParams) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.stop(&params.session, params.force)
    })
    .await
}

async fn forget(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.forget(&params.session)).await
}

/// Starts a new run of an exited session, which keeps its id (0054).
async fn resume(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.resume(&params.session)).await
}

/// Types a prompt into a run's terminal (0054). The text is never logged.
async fn prompt(ctx: RequestCtx, params: AgentPromptParams) -> Result<PromptReceipt, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.prompt(&params.session, &params.text, params.force)
    })
    .await
}

/// A report from a run's hook (0053). The payload, the result and the token are never logged.
async fn hook(ctx: RequestCtx, params: AgentHookParams) -> Result<(), ErrorBody> {
    on_agents(&ctx, move |agents| agents.hook(&params)).await
}

/// Starts a child session of a parent session (0055). The prompt is never logged.
async fn delegate(
    ctx: RequestCtx,
    params: AgentDelegateParams,
) -> Result<AgentDelegated, ErrorBody> {
    on_agents(&ctx, move |agents| agents.delegate(params)).await
}

/// A session's delegation tree: its root and the root's children (0055).
async fn tree(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentTree, ErrorBody> {
    on_agents(&ctx, move |agents| agents.tree(&params.session)).await
}

/// Brings a finished child back into its parent (0055). The parent workspace's git mutation lock is held for the whole
/// call, so no git command of a client interleaves with the clean check and the merge.
async fn bring_back(
    ctx: RequestCtx,
    params: AgentBringBackParams,
) -> Result<BringBackResult, ErrorBody> {
    let child = params.child.clone();
    let merge = params.merge;
    // The workspace a merge would go into: only a child with a workspace of its own is merged (0055).
    let merge_into = on_agents(&ctx, move |agents| {
        let child = agents.show(&child)?;
        let Some(parent) = child.parent_id else {
            return Ok(None);
        };
        let parent = agents.show(&parent)?;
        Ok((merge && parent.workspace_id != child.workspace_id).then_some(parent.workspace_id))
    })
    .await?;
    // A merge never runs unlocked: a workspace that is not a git worktree, is missing or is being removed refuses the
    // bring-back here.
    let _guard = match merge_into {
        Some(workspace) => Some(super::git::repository(&ctx, workspace).await?.lock().await),
        None => None,
    };
    let git = WorktreeGit(ctx.state.workspaces.git().clone());
    on_agents(&ctx, move |agents| agents.bring_back(&params, &git)).await
}

/// The bring-back's git operations, through `core-git`.
struct WorktreeGit(Git);

impl RepoOps for WorktreeGit {
    fn is_dirty(&self, dir: &Path) -> Result<bool, AgentError> {
        self.0.status_dirty(dir).map_err(agent_git_error)
    }

    fn operation_in_progress(&self, dir: &Path) -> Result<Option<String>, AgentError> {
        let operation = self.0.operation_in_progress(dir).map_err(agent_git_error)?;
        Ok(operation.map(|operation| {
            match operation {
                GitOperation::Merge => "a merge",
                GitOperation::Rebase => "a rebase",
                GitOperation::CherryPick => "a cherry-pick",
                GitOperation::Revert => "a revert",
                GitOperation::ApplyMailbox => "a mailbox apply",
                GitOperation::Unknown => "a git operation",
            }
            .to_owned()
        }))
    }

    fn merge_no_ff(&self, dir: &Path, branch: &str) -> Result<MergeResult, AgentError> {
        Ok(
            match self.0.merge_no_ff(dir, branch).map_err(agent_git_error)? {
                MergeOutcome::Merged { commit } => MergeResult::Merged { commit },
                MergeOutcome::Conflict { paths } => MergeResult::Conflict { paths },
            },
        )
    }
}

/// A git failure as the protocol reports it, as `git_error` maps it for the git methods.
fn agent_git_error(error: GitError) -> AgentError {
    match massorca_core_git::failure_code(&error) {
        Some(code) => AgentError::new(code, error.to_string()),
        None => DomainError::from(error).into(),
    }
}

/// The sessions a watcher selected, as ids resolved once when the stream opens.
#[derive(Debug, Clone)]
struct Selection {
    workspace_id: Option<String>,
    session_id: Option<String>,
}

impl Selection {
    fn resolve(agents: &Agents, params: &AgentWatchParams) -> Result<Self, AgentError> {
        let workspace_id = match &params.workspace {
            Some(selector) => Some(agents.workspace_id(selector)?),
            None => None,
        };
        let session_id = match &params.session {
            Some(selector) => Some(agents.show(selector)?.id),
            None => None,
        };
        Ok(Self {
            workspace_id,
            session_id,
        })
    }

    fn selects(&self, session: &AgentSession) -> bool {
        self.workspace_id
            .as_ref()
            .is_none_or(|id| *id == session.workspace_id)
            && self.session_id.as_ref().is_none_or(|id| *id == session.id)
    }

    /// Every selected session now, newest first.
    fn snapshot(&self, agents: &Agents) -> Result<AgentWatchEvent, AgentError> {
        let sessions = agents
            .list(self.workspace_id.as_deref())?
            .into_iter()
            .filter(|session| self.selects(session))
            .collect();
        Ok(AgentWatchEvent::Snapshot { sessions })
    }
}

/// Streams the selected sessions: a `snapshot` first, then a `changed` item after every change, in the order the store
/// applied them. The subscription is taken before the snapshot, so no change falls between; a watcher that falls
/// behind is given a fresh `snapshot` (0052).
async fn watch(ctx: RequestCtx, params: AgentWatchParams) -> Result<StreamSource, ErrorBody> {
    let mut changes = ctx.state.agent_changes.subscribe();
    let (selection, first) = on_agents(&ctx, move |agents| {
        let selection = Selection::resolve(agents, &params)?;
        let first = selection.snapshot(agents)?;
        Ok((selection, first))
    })
    .await?;
    let (items, source) = mpsc::channel(WATCH_QUEUE_CAPACITY);
    let agents = ctx.state.agents.clone();
    tokio::spawn(async move {
        let mut next = Some(first);
        loop {
            if let Some(item) = next.take() {
                let Ok(value) = encode(&item) else { break };
                if items.send(value).await.is_err() {
                    break;
                }
            }
            let change = tokio::select! {
                () = items.closed() => break,
                change = changes.recv() => change,
            };
            next = match change {
                Ok(Change::Updated(session)) if selection.selects(&session) => {
                    Some(AgentWatchEvent::Changed { session })
                }
                Ok(_) => None,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let agents = agents.clone();
                    let selection = selection.clone();
                    match tokio::task::spawn_blocking(move || selection.snapshot(&agents)).await {
                        Ok(Ok(snapshot)) => Some(snapshot),
                        _ => break,
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            };
        }
    });
    Ok(source)
}

/// Copies a kind's declared credential files into the profile's home for it (0056). Only names and sizes come back;
/// nothing about the files is logged beyond that.
/// Deprecated (0056, decision 0059): imports into the kind's active account. Refused while a run of the kind is live
/// when it replaces the account.
async fn credentials_import(
    ctx: RequestCtx,
    params: CredentialsImportParams,
) -> Result<CredentialsImported, ErrorBody> {
    on_agents(&ctx, move |agents| {
        if params.replace {
            refuse_live_runs(agents, &params.kind)?;
        }
        agents.credentials_import(&params.kind, &params.from, params.replace)
    })
    .await
}

/// Deprecated (0056): the kind's active account's files, names and sizes only.
async fn credentials_show(
    ctx: RequestCtx,
    params: CredentialsKindRef,
) -> Result<CredentialsShown, ErrorBody> {
    on_agents(&ctx, move |agents| agents.credentials_show(&params.kind)).await
}

/// Deprecated (0056): removes the kind's active account. Refused while a run of the kind is live.
async fn credentials_remove(
    ctx: RequestCtx,
    params: CredentialsKindRef,
) -> Result<CredentialsRemoved, ErrorBody> {
    on_agents(&ctx, move |agents| {
        refuse_live_runs(agents, &params.kind)?;
        agents.credentials_remove(&params.kind)
    })
    .await
}

/// The accounts of a kind (decision 0059).
async fn accounts_list(
    ctx: RequestCtx,
    params: AccountsListParams,
) -> Result<AccountsList, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents
            .accounts_list(&params.kind)
            .map(|accounts| AccountsList { accounts })
    })
    .await
}

/// Sign-in: a new account and the terminal that runs the kind's login (decision 0059).
async fn accounts_add(
    ctx: RequestCtx,
    params: AccountsAddParams,
) -> Result<AccountsAdded, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.accounts_add(&params.kind, params.label)
    })
    .await
}

/// Import into a new account from a directory the person names (decision 0059).
async fn accounts_import(
    ctx: RequestCtx,
    params: AccountsImportParams,
) -> Result<AccountsImported, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.accounts_import(&params.kind, &params.from, params.label)
    })
    .await
}

async fn accounts_activate(
    ctx: RequestCtx,
    params: AccountsActivateParams,
) -> Result<AccountSummary, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.accounts_activate(&params.kind, &params.account, params.project.as_deref())
    })
    .await
}

async fn accounts_remove(
    ctx: RequestCtx,
    params: AccountsRemoveParams,
) -> Result<AccountsRemoved, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.accounts_remove(&params.kind, &params.account)
    })
    .await
}

async fn accounts_check(
    ctx: RequestCtx,
    params: AccountsCheckParams,
) -> Result<AccountSummary, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.accounts_check(&params.kind, params.account.as_deref())
    })
    .await
}

/// Refuses with `agent.busy` while any session of `kind` has a run that has not exited: replacing or removing its home
/// would pull its login away under it.
fn refuse_live_runs(agents: &Agents, kind: &str) -> Result<(), AgentError> {
    let live: Vec<String> = agents
        .list(None)?
        .into_iter()
        .filter(|session| session.kind == kind && session.execution.state != TerminalState::Exited)
        .map(|session| session.id)
        .collect();
    if live.is_empty() {
        return Ok(());
    }
    let mut error = AgentError::new(
        codes::AGENT_BUSY,
        format!(
            "a run of agent kind {kind} is still live (session {}); stop it before replacing or removing its credentials",
            live.join(", ")
        ),
    );
    error.data = Some(serde_json::json!({ "sessions": live }));
    Err(error)
}

/// Runs one agents call on the blocking pool and maps its refusal to the protocol error.
async fn on_agents<T, F>(ctx: &RequestCtx, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Agents) -> Result<T, AgentError> + Send + 'static,
{
    let agents = ctx.state.agents.clone();
    match tokio::task::spawn_blocking(move || call(&agents)).await {
        Ok(result) => result.map_err(ErrorBody::from),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the agents task failed: {error}"),
        )),
    }
}
