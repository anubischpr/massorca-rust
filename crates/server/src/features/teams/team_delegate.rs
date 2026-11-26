//! `team.delegate` and `team.delegation_tree` (capability `team-delegate/1`, decision 0128). A bot's run hands work to a
//! teammate as a child task of the task it works on. The child starts through the normal task start: in its own
//! workspace, or in its parent's when it only reads. Its outcome is posted to its room by its bot.

use std::collections::{BTreeSet, HashMap};

use massorca_core_agents::{
    Agents, DELEGATION_DEPTH_DEFAULT, DELEGATION_DEPTH_MAX, DELEGATION_DEPTH_MIN,
    DELEGATION_DEPTH_SETTING, SecretGrant,
};
use massorca_core_store::{BotRow, Store, TaskRow, TeamRow, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::agents::PermissionMode;
use massorca_protocol::methods::events::TOPIC_TASK_CHANGED;
use massorca_protocol::methods::tasks::{TaskCreateParams, TaskRef};
use massorca_protocol::methods::teams::{
    self, DelegationChild, DelegationLink, DelegationList, DelegationTree, TeamDelegateParams,
    TeamDelegated, TeamRef,
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
use super::task_start::{Changes, publish_changes, start_gate, start_task, team_mode};
use super::task_state;
use super::team_message::child_report;
use super::{blocking, non_blank, optional_text, selectors, store_error};

/// The longest title a child task takes from its text, in Unicode scalar values.
const MAX_TITLE_CHARS: usize = 80;
/// Words that name no job: they never decide which teammate takes a task.
const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "your", "make", "add", "use",
    "una", "los", "las", "para", "con", "por", "del",
];
/// The owner the core settings are stored under, as core-agents stores them.
const SETTINGS_OWNER: &str = "core";
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
        methods.register(
            Owner::Core,
            teams::DELEGATION_LIST,
            capabilities::TEAM_DELEGATE,
            Permission::Read,
            list,
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

async fn list(ctx: RequestCtx, params: TeamRef) -> Result<DelegationList, ErrorBody> {
    blocking(&ctx, move |store, agents| {
        delegation_list(store, agents, &params.team)
    })
    .await
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
    check_reads(params.read_only, &parent.team_id, &team.id)?;
    ensure_depth(store, &parent.id)?;
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
    // Stored with its link in one transaction. A repeated request from this parent returns the child it made first.
    let request_id = optional_text(params.request_id.clone());
    let repeated = {
        let _gate = start_gate();
        store
            .insert_delegated_task(
                &child,
                &parent.id,
                params.read_only,
                request_id.as_deref(),
                now_ms(),
            )
            .map_err(store_error)?
    };
    if let Some(first) = repeated {
        let row = selectors::task(store, &first)?;
        return Ok(TeamDelegated {
            task: view_of(&row, &task_state::load(agents, &row)?),
            parent: parent.id,
            picked,
        });
    }
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

/// A read-only child shares its parent's workspace, so it stays in the parent's team: another team's bot would share a
/// workspace it does not own (decision 0128).
fn check_reads(read_only: bool, parent_team: &str, team: &str) -> Result<(), ErrorBody> {
    if read_only && parent_team != team {
        return Err(ErrorBody::new(
            codes::TEAM_DELEGATE_READS_OTHER_TEAM,
            "a read-only child shares its parent's workspace, so it must stay in the parent's team",
        ));
    }
    Ok(())
}

/// The delegation depth limit the store holds under `agents.delegation_depth`, read on every call (decision 0070).
fn depth_limit(store: &Store) -> Result<i64, ErrorBody> {
    let stored = store
        .get_setting(SETTINGS_OWNER, DELEGATION_DEPTH_SETTING)
        .map_err(store_error)?;
    Ok(parse_depth_limit(stored.as_deref()))
}

/// The stored limit, clamped as core-agents clamps it; an unset or unreadable value is the default.
fn parse_depth_limit(stored: Option<&str>) -> i64 {
    stored
        .and_then(|text| serde_json::from_str::<i64>(text).ok())
        .map_or(DELEGATION_DEPTH_DEFAULT, |depth| {
            depth.clamp(DELEGATION_DEPTH_MIN, DELEGATION_DEPTH_MAX)
        })
}

/// Refuses a child that would sit deeper than the limit: a child of `parent` is one level below it, so a child cannot
/// delegate at the default depth (decision 0128).
fn ensure_depth(store: &Store, parent: &str) -> Result<(), ErrorBody> {
    let limit = depth_limit(store)?;
    let depth = i64::from(store.task_delegation_depth(parent).map_err(store_error)?) + 1;
    if depth <= limit {
        return Ok(());
    }
    let mut error = ErrorBody::new(
        codes::TEAM_DELEGATE_DEPTH_EXCEEDED,
        format!(
            "task {parent} cannot delegate: its child would be at depth {depth}, and {DELEGATION_DEPTH_SETTING} allows at most {limit}"
        ),
    );
    error.data = Some(serde_json::json!({
        "limit": limit,
        "setting": DELEGATION_DEPTH_SETTING,
        "depth": depth,
    }));
    Err(error)
}

/// The run that delegates: its session and token prove it is a live agent run, and the task it works on is the parent.
/// The caller is always that task's bot, so a bot can only delegate as itself.
fn caller(
    store: &Store,
    agents: &Agents,
    params: &TeamDelegateParams,
) -> Result<(TaskRow, BotRow), ErrorBody> {
    bot_run(
        store,
        agents,
        params.session.as_deref(),
        params.agent_token.as_deref(),
        || {
            ErrorBody::new(
                codes::TEAM_DELEGATE_NOT_A_BOT,
                "only a bot's agent run can delegate, with its own session and run token",
            )
        },
    )
}

/// The task and bot of a live bot run: `token` must be the run token of a live run of `session`
/// (`permission.denied` otherwise), and that session must be a task's with a bot (`not_a_bot` otherwise).
pub(super) fn bot_run(
    store: &Store,
    agents: &Agents,
    session: Option<&str>,
    token: Option<&str>,
    not_a_bot: impl Fn() -> ErrorBody,
) -> Result<(TaskRow, BotRow), ErrorBody> {
    let (Some(session), Some(token)) = (session, token) else {
        return Err(not_a_bot());
    };
    if !agents.run_token_matches(session, token) {
        return Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            "the agent token does not match a live run of that session",
        ));
    }
    let task = store
        .find_task_by_session(session)
        .map_err(store_error)?
        .ok_or_else(&not_a_bot)?;
    let bot_id = task.bot_id.clone().ok_or_else(&not_a_bot)?;
    let team = selectors::team(store, &task.team_id)?;
    let bot = selectors::bot(store, &team, &bot_id)?;
    Ok((task, bot))
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
    Ok(DelegationTree {
        children: children_of(store, agents, &parent.id)?,
        task: view_of(&parent, &task_state::load(agents, &parent)?),
    })
}

/// Every delegated child of the team, with the task that delegated it, the team's tasks in creation order.
fn delegation_list(
    store: &Store,
    agents: &Agents,
    selector: &str,
) -> Result<DelegationList, ErrorBody> {
    let team = selectors::team(store, selector)?;
    let mut links = Vec::new();
    for parent in store.list_tasks(Some(&team.id)).map_err(store_error)? {
        for child in children_of(store, agents, &parent.id)? {
            links.push(DelegationLink {
                parent: parent.id.clone(),
                read_only: child.read_only,
                child: child.task,
                report: child.report,
            });
        }
    }
    Ok(DelegationList { links })
}

/// The children `parent` delegated, oldest first, with their state.
fn children_of(
    store: &Store,
    agents: &Agents,
    parent: &str,
) -> Result<Vec<DelegationChild>, ErrorBody> {
    let mut children = Vec::new();
    for id in store
        .task_delegation_children(parent)
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
            report: child_report(store, &id, parent)?,
        });
    }
    Ok(children)
}

/// The plan mode of kind `kind`, the mode a read-only child starts in (decision 0128). `None` when the kind declares none,
/// and the child then keeps the vendor's default, which is never a bypass mode.
fn plan_mode_of(agents: &Agents, kind: &str) -> Option<String> {
    let modes = agents
        .kinds()
        .kinds
        .into_iter()
        .find(|declared| declared.id == kind)?
        .permission_modes;
    plan_mode_among(&modes)
}

/// The first mode that is a plan mode: not a bypass, and named `plan` by its id or its label, in any letter case.
fn plan_mode_among(modes: &[PermissionMode]) -> Option<String> {
    modes
        .iter()
        .find(|mode| {
            !mode.bypass
                && (mode.id.eq_ignore_ascii_case("plan") || mode.label.eq_ignore_ascii_case("plan"))
        })
        .map(|mode| mode.id.clone())
}

/// The mode a delegated child runs in (decision 0128). A read-only child starts in its kind's plan mode, so it cannot
/// edit, and keeps the vendor's default when the kind has none. A writing child gets its team's mode, but none when its
/// parent runs in the default or its parent's bot is gone.
pub(super) fn child_mode(
    store: &Store,
    agents: &Agents,
    team: &TeamRow,
    bot: &BotRow,
    task: &TaskRow,
) -> Result<Option<String>, ErrorBody> {
    let Some((parent_id, read_only)) = store
        .task_delegation_parent(&task.id)
        .map_err(store_error)?
    else {
        return Ok(team_mode(agents, team, bot));
    };
    if read_only {
        return Ok(plan_mode_of(agents, &bot.kind));
    }
    let parent = selectors::task(store, &parent_id)?;
    let parent_bot = match parent.bot_id.as_deref() {
        Some(id) => store.get_bot(id).map_err(store_error)?,
        None => None,
    };
    let Some(parent_bot) = parent_bot else {
        return Ok(None);
    };
    let parent_team = selectors::team(store, &parent.team_id)?;
    Ok(team_mode(agents, &parent_team, &parent_bot).and(team_mode(agents, team, bot)))
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
    use massorca_core_store::{TaskOrigin, new_id};

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

    fn store_with_team() -> (Store, String) {
        let store = Store::open_in_memory().expect("store");
        let team = TeamRow {
            id: "team".to_owned(),
            name: "Parser".to_owned(),
            purpose: String::new(),
            character: "triangle".to_owned(),
            access: "full".to_owned(),
            workspaces: false,
            created_at: 0,
        };
        store.insert_team(&team).expect("team");
        (store, team.id)
    }

    fn task_in(team: &str, title: &str) -> TaskRow {
        TaskRow {
            id: new_id(),
            team_id: team.to_owned(),
            project_id: None,
            bot_id: None,
            title: title.to_owned(),
            body: String::new(),
            origin: TaskOrigin::Cli,
            session_id: None,
            outcome: None,
            error: None,
            error_code: None,
            workspace_id: None,
            branch: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn a_child_cannot_delegate_at_the_default_depth() {
        let (store, team) = store_with_team();
        let root = task_in(&team, "Lead");
        let child = task_in(&team, "Write");
        store.insert_task(&root).expect("root");
        store
            .insert_delegated_task(&child, &root.id, false, None, 0)
            .expect("link");
        assert!(ensure_depth(&store, &root.id).is_ok());
        let refused = ensure_depth(&store, &child.id).expect_err("depth");
        assert_eq!(refused.code, codes::TEAM_DELEGATE_DEPTH_EXCEEDED);
    }

    #[test]
    fn a_raised_depth_limit_lets_a_child_delegate_one_level_deeper() {
        let (store, team) = store_with_team();
        store
            .set_setting("core", DELEGATION_DEPTH_SETTING, "2")
            .expect("setting");
        let root = task_in(&team, "Lead");
        let child = task_in(&team, "Write");
        let grandchild = task_in(&team, "Test");
        store.insert_task(&root).expect("root");
        store
            .insert_delegated_task(&child, &root.id, false, None, 0)
            .expect("child");
        store
            .insert_delegated_task(&grandchild, &child.id, false, None, 0)
            .expect("grandchild");
        assert!(ensure_depth(&store, &child.id).is_ok());
        let refused = ensure_depth(&store, &grandchild.id).expect_err("depth");
        assert_eq!(refused.code, codes::TEAM_DELEGATE_DEPTH_EXCEEDED);
    }

    #[test]
    fn the_depth_limit_is_the_stored_one_clamped_and_defaults_to_one() {
        assert_eq!(parse_depth_limit(None), 1);
        assert_eq!(parse_depth_limit(Some("3")), 3);
        assert_eq!(parse_depth_limit(Some("9")), 5);
        assert_eq!(parse_depth_limit(Some("0")), 1);
        assert_eq!(parse_depth_limit(Some("\"x\"")), 1);
    }

    fn mode(id: &str, label: &str, bypass: bool) -> PermissionMode {
        serde_json::from_value(serde_json::json!({ "id": id, "label": label, "bypass": bypass }))
            .expect("a permission mode")
    }

    #[test]
    fn a_read_only_child_takes_the_kinds_plan_mode_and_never_a_bypass() {
        let modes = vec![
            mode("bypassPermissions", "Bypass", true),
            mode("acceptEdits", "Accept edits", false),
            mode("plan", "Plan", false),
        ];
        assert_eq!(plan_mode_among(&modes).as_deref(), Some("plan"));
        let by_label = vec![mode("read-only", "PLAN", false)];
        assert_eq!(plan_mode_among(&by_label).as_deref(), Some("read-only"));
        let bypass_plan = vec![mode("plan", "Plan", true)];
        assert_eq!(plan_mode_among(&bypass_plan), None);
    }

    #[test]
    fn a_kind_without_a_plan_mode_gives_a_read_only_child_none() {
        let modes = vec![mode("acceptEdits", "Accept edits", false)];
        assert_eq!(plan_mode_among(&modes), None);
        assert_eq!(plan_mode_among(&[]), None);
    }

    #[test]
    fn a_read_only_child_stays_in_its_parents_team() {
        assert!(check_reads(true, "a", "b").is_err());
        assert!(check_reads(true, "a", "a").is_ok());
        assert!(check_reads(false, "a", "b").is_ok());
    }
}
