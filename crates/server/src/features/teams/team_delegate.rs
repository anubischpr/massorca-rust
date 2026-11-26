//! `team.delegate` and `team.delegation_tree` (capability `team-delegate/1`, decision 0128). A bot's run hands work to a
//! teammate as a child task of the task it works on. The child starts through the normal task start: in its own
//! workspace, or in its parent's when it only reads. Its outcome is posted to its room by its bot.

use std::collections::{BTreeSet, HashMap};

use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_store::{BotRow, Store, TaskRow, TeamRow, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::events::TOPIC_TASK_CHANGED;
use massorca_protocol::methods::tasks::{TaskCreateParams, TaskRef};
use massorca_protocol::methods::teams::{
    self, DelegationChild, DelegationTree, TeamDelegateParams, TeamDelegated,
};
use massorca_protocol::methods::workspaces::Workspace;
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{for_mutation, publish};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::task::{new_task_row, view_of};
use super::task_start::{Changes, publish_changes, start_task, team_mode};
use super::task_state;
use super::{blocking, non_blank, optional_text, selectors, store_error};

/// The longest title a child task takes from its text, in Unicode scalar values.
const MAX_TITLE_CHARS: usize = 80;
/// Words that name no job: they never decide which teammate takes a task.
const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "your", "make", "add", "use",
    "una", "los", "las", "para", "con", "por", "del",
];
/// The reason a child's run gets no vault variables: a delegated child never takes more scope than its parent.
const WITHHELD_REASON: &str = "a delegated child starts without the vault";

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::TEAM_DELEGATE, Owner::Core)?,
        methods.register(
            Owner::Core,
            teams::DELEGATE,
            capabilities::TEAM_DELEGATE,
            Permission::Workspace,
            delegate,
        )?,
        methods.register(
            Owner::Core,
            teams::DELEGATION_TREE,
            capabilities::TEAM_DELEGATE,
            Permission::Read,
            tree,
        )?,
    ])
}

async fn delegate(ctx: RequestCtx, params: TeamDelegateParams) -> Result<TeamDelegated, ErrorBody> {
    let workspaces = for_mutation(&ctx).await?;
    let (changes, result) = blocking(&ctx, move |store, agents| {
        let mut changes = Changes::default();
        let result = delegate_task(store, agents, &workspaces, params, &mut changes);
        Ok((changes, result))
    })
    .await?;
    publish_changes(&ctx.state, &changes);
    let delegated = result?;
    publish(&ctx, TOPIC_TASK_CHANGED, &delegated.task.id);
    Ok(delegated)
}

async fn tree(ctx: RequestCtx, params: TaskRef) -> Result<DelegationTree, ErrorBody> {
    blocking(&ctx, move |store, agents| {
        delegation_tree(store, agents, &params.task)
    })
    .await
}

fn delegate_task(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    params: TeamDelegateParams,
    changes: &mut Changes,
) -> Result<TeamDelegated, ErrorBody> {
    let (parent, caller) = caller(store, agents, &params)?;
    let text = non_blank(&params.text, "the task text")?;
    let parent_team = selectors::team(store, &parent.team_id)?;
    let team = match optional_text(params.team.clone()) {
        Some(selector) => selectors::team(store, &selector)?,
        None => parent_team,
    };
    let (bot, picked) = match optional_text(params.bot.clone()) {
        Some(selector) => (selectors::bot(store, &team, &selector)?, false),
        None => (pick_teammate(store, &team, &caller, &text)?, true),
    };
    if bot.id == caller.id && team.id == caller.team_id {
        return Err(ErrorBody::new(
            codes::TEAM_DELEGATE_SELF,
            "a bot never delegates to itself; name a teammate with bot",
        ));
    }
    // A writing child keeps the parent's project, so it gets that project's own worktree. A read-only child shares
    // the parent's workspace instead. A project of another team is never linked.
    let project = (!params.read_only && team.id == parent.team_id)
        .then(|| parent.project_id.clone())
        .flatten();
    let child = new_task_row(
        store,
        TaskCreateParams {
            team: team.id.clone(),
            title: title_of(&text)?,
            body: Some(text),
            bot: Some(bot.id.clone()),
            project,
            origin: None,
        },
    )?;
    store.insert_task(&child).map_err(store_error)?;
    store
        .insert_task_delegation(&child.id, &parent.id, params.read_only, now_ms())
        .map_err(store_error)?;
    // A child that cannot start stays Open with the reason in its `error`, as a task does; the delegation stands.
    let started = start_task(
        store,
        agents,
        workspaces,
        &child.id,
        SecretGrant::Withhold(WITHHELD_REASON),
        changes,
    );
    let task = match started {
        Ok(task) => task,
        Err(_) => {
            let row = selectors::task(store, &child.id)?;
            view_of(&row, &task_state::load(agents, &row)?)
        }
    };
    Ok(TeamDelegated {
        task,
        parent: parent.id,
        picked,
    })
}

/// The run that delegates: its session and token prove it is a live agent run, and the task it works on is the parent.
/// The caller is always that task's bot, so a bot can only delegate as itself.
fn caller(
    store: &Store,
    agents: &Agents,
    params: &TeamDelegateParams,
) -> Result<(TaskRow, BotRow), ErrorBody> {
    let not_a_bot = || {
        ErrorBody::new(
            codes::TEAM_DELEGATE_NOT_A_BOT,
            "only a bot's agent run can delegate, with its own session and run token",
        )
    };
    let (Some(session), Some(token)) = (params.session.as_deref(), params.agent_token.as_deref())
    else {
        return Err(not_a_bot());
    };
    if !agents.run_token_matches(session, token) {
        return Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            "the agent token does not match a live run of that session",
        ));
    }
    let parent = store
        .find_task_by_session(session)
        .map_err(store_error)?
        .ok_or_else(not_a_bot)?;
    let bot_id = parent.bot_id.clone().ok_or_else(not_a_bot)?;
    let team = selectors::team(store, &parent.team_id)?;
    let caller = selectors::bot(store, &team, &bot_id)?;
    Ok((parent, caller))
}

/// The bot competence picks for `text` when none is named: the teammate whose job or skills share the most words with
/// it. Ties go to the name that sorts first, so the same text always picks the same bot. The caller is not a candidate
/// in its own team. A text that shares no word with any teammate is refused rather than given to a bot by chance.
fn pick_teammate(
    store: &Store,
    team: &TeamRow,
    caller: &BotRow,
    text: &str,
) -> Result<BotRow, ErrorBody> {
    let skills = store.team_bot_skills(&team.id).map_err(store_error)?;
    let candidates = store
        .list_bots(&team.id)
        .map_err(store_error)?
        .into_iter()
        .filter(|bot| !(bot.id == caller.id && team.id == caller.team_id))
        .map(|bot| {
            let profile = profile_words(&bot, &skills);
            (bot, profile)
        })
        .collect::<Vec<_>>();
    pick_by_words(&candidates, text).cloned().ok_or_else(|| {
        ErrorBody::new(
            codes::TEAM_DELEGATE_NO_BOT,
            "no teammate's job or skills match this task; name the bot with bot",
        )
    })
}

fn profile_words(bot: &BotRow, skills: &HashMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut words = words_of(&bot.job);
    for skill in skills.get(&bot.id).into_iter().flatten() {
        words.extend(words_of(skill));
    }
    words
}

/// The candidate that shares the most words with `text`, or `None` when none shares one. Ties go to the lowest name.
fn pick_by_words<'a>(
    candidates: &'a [(BotRow, BTreeSet<String>)],
    text: &str,
) -> Option<&'a BotRow> {
    let wanted = words_of(text);
    candidates
        .iter()
        .map(|(bot, profile)| (wanted.intersection(profile).count(), bot))
        .filter(|(shared, _)| *shared > 0)
        .min_by(|(left, a), (right, b)| right.cmp(left).then_with(|| a.name.cmp(&b.name)))
        .map(|(_, bot)| bot)
}

/// The distinct words of `text` that can name a job: lowercase, at least three letters, and not a stop word.
fn words_of(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| word.chars().count() >= 3 && !STOP_WORDS.contains(&word.as_str()))
        .collect()
}

/// The child's title: the first line of its text, cut to the title length.
fn title_of(text: &str) -> Result<String, ErrorBody> {
    let first = text.lines().next().unwrap_or_default();
    let title: String = first.chars().take(MAX_TITLE_CHARS).collect();
    non_blank(&title, "the task text")
}

fn delegation_tree(
    store: &Store,
    agents: &Agents,
    selector: &str,
) -> Result<DelegationTree, ErrorBody> {
    let parent = selectors::task(store, selector)?;
    let mut children = Vec::new();
    for id in store
        .task_delegation_children(&parent.id)
        .map_err(store_error)?
    {
        let row = selectors::task(store, &id)?;
        let read_only = store
            .task_delegation_parent(&id)
            .map_err(store_error)?
            .is_some_and(|(_, read_only)| read_only);
        children.push(DelegationChild {
            task: view_of(&row, &task_state::load(agents, &row)?),
            read_only,
        });
    }
    Ok(DelegationTree {
        task: view_of(&parent, &task_state::load(agents, &parent)?),
        children,
    })
}

/// The mode a delegated child runs in (decision 0128): the team's own mode, but never a bypass mode when the parent runs
/// in the vendor's default. An undelegated task keeps its team's mode.
pub(super) fn child_mode(
    store: &Store,
    agents: &Agents,
    team: &TeamRow,
    bot: &BotRow,
    task: &TaskRow,
) -> Result<Option<String>, ErrorBody> {
    let own = team_mode(agents, team, bot);
    let Some((parent_id, _)) = store
        .task_delegation_parent(&task.id)
        .map_err(store_error)?
    else {
        return Ok(own);
    };
    let parent = selectors::task(store, &parent_id)?;
    let parent_team = selectors::team(store, &parent.team_id)?;
    let Some(parent_bot) = parent.bot_id.as_deref() else {
        return Ok(None);
    };
    let parent_bot = selectors::bot(store, &parent_team, parent_bot)?;
    Ok(team_mode(agents, &parent_team, &parent_bot).and(own))
}

/// The parent's workspace a read-only child runs in. `None` for a task that is not a read-only child, or whose parent
/// has no workspace yet; the child then takes the team's folder as any task does.
pub(super) fn read_only_workspace(
    store: &Store,
    workspaces: &Workspaces,
    task: &TaskRow,
) -> Result<Option<Workspace>, ErrorBody> {
    let Some((parent_id, true)) = store
        .task_delegation_parent(&task.id)
        .map_err(store_error)?
    else {
        return Ok(None);
    };
    let parent = selectors::task(store, &parent_id)?;
    let Some(workspace_id) = parent.workspace_id else {
        return Ok(None);
    };
    // A removed parent workspace leaves the child the team's folder, the same as a task with none.
    Ok(workspaces.show_workspace(&workspace_id).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bot(name: &str, job: &str) -> BotRow {
        BotRow {
            id: format!("id-{name}"),
            team_id: "team".to_owned(),
            name: name.to_owned(),
            job: job.to_owned(),
            kind: "claude".to_owned(),
            model: None,
            effort: None,
            lead: false,
            created_at: 0,
        }
    }

    fn candidates() -> Vec<(BotRow, BTreeSet<String>)> {
        [
            bot("Mara", "Writes the parser and the lexer"),
            bot("Jun", "Reviews pull requests for security"),
            bot("Ada", "Writes the parser tests"),
        ]
        .into_iter()
        .map(|bot| {
            let profile = words_of(&bot.job);
            (bot, profile)
        })
        .collect()
    }

    #[test]
    fn competence_picks_the_teammate_whose_job_shares_the_most_words() {
        let pool = candidates();
        let picked =
            pick_by_words(&pool, "Parse the lexer and the parser").map(|bot| bot.name.clone());
        assert_eq!(picked.as_deref(), Some("Mara"));
    }

    #[test]
    fn competence_breaks_a_tie_by_the_name_that_sorts_first() {
        let pool = candidates();
        let tied = pick_by_words(&pool, "write the parser").map(|bot| bot.name.clone());
        assert_eq!(tied.as_deref(), Some("Ada"));
    }

    #[test]
    fn competence_refuses_when_no_job_shares_a_word_with_the_task() {
        let pool = candidates();
        assert!(pick_by_words(&pool, "Refactor the billing page").is_none());
        assert!(pick_by_words(&pool, "the and for").is_none());
    }

    #[test]
    fn a_title_is_the_first_line_cut_to_its_length() {
        assert_eq!(title_of("First line\nsecond").unwrap(), "First line");
        let long = "x".repeat(200);
        assert_eq!(title_of(&long).unwrap().chars().count(), MAX_TITLE_CHARS);
        assert!(title_of("   ").is_err());
    }
}
