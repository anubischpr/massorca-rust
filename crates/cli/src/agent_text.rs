//! Human-readable lines for agent kinds and sessions.

use massorca_protocol::methods::agents::{
    Activity, ActivitySource, ActivityState, AgentKind, AgentModeSet, AgentSession, AgentTree,
    BringBackResult, Execution, ModeApplied, ModeSource, Outcome, PackageState,
};

use massorca_protocol::methods::agents::{AgentArchive, AgentQueueList, QueueState, QueuedPrompt};

use crate::terminal;
use crate::workspace_text;

/// The kinds as a table with the package state, the detection and the resolved path, then one line
/// per problem of a kind that has any.
pub fn kinds_lines(kinds: &[AgentKind]) -> Vec<String> {
    if kinds.is_empty() {
        return vec!["no agent kinds".to_owned()];
    }
    let rows = kinds
        .iter()
        .map(|kind| {
            vec![
                kind.id.clone(),
                kind.name.clone(),
                kind.owner.clone(),
                package_state_text(kind.package.state).to_owned(),
                detected_text(kind),
                kind.program_path.clone().unwrap_or_default(),
            ]
        })
        .collect();
    let mut lines = workspace_text::table(
        &["ID", "NAME", "OWNER", "INSTALLED", "DETECTED", "PATH"],
        rows,
    );
    for kind in kinds
        .iter()
        .filter(|kind| !kind.permission_modes.is_empty())
    {
        lines.push(format!("modes: {}: {}", kind.id, modes_text(kind)));
    }
    for kind in kinds {
        for problem in &kind.problems {
            lines.push(format!("problem: {}: {problem}", kind.id));
        }
    }
    lines
}

/// `id (Label, default, bypass, live, may fall back)` for each mode, in declared order (decision 0083).
fn modes_text(kind: &AgentKind) -> String {
    kind.permission_modes
        .iter()
        .map(|mode| {
            let mut marks = vec![mode.label.as_str()];
            if kind.default_permission_mode.as_deref() == Some(mode.id.as_str()) {
                marks.push("default");
            }
            if mode.bypass {
                marks.push("bypass");
            }
            if mode.live_switch {
                marks.push("live");
            }
            if mode.may_fall_back {
                marks.push("may fall back");
            }
            format!("{} ({})", mode.id, marks.join(", "))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The package state of a kind, `unknown` for a value this build does not know.
fn package_state_text(state: PackageState) -> &'static str {
    match state {
        PackageState::Available => "available",
        PackageState::Enabled => "enabled",
        PackageState::Disabled => "disabled",
        PackageState::Unknown => "unknown",
    }
}

/// `yes` when a program resolves, `invalid` for a declaration with problems, or `no`.
fn detected_text(kind: &AgentKind) -> String {
    match (kind.detected, kind.problems.is_empty()) {
        (true, _) => "yes".to_owned(),
        (false, false) => "invalid".to_owned(),
        _ => "no".to_owned(),
    }
}

/// The fields of one session, one per line. The result text is not printed; `--json` carries it.
pub fn session_details(session: &AgentSession) -> Vec<String> {
    let mut lines = vec![
        format!("id: {}", session.id),
        format!("kind: {}", session.kind),
        format!("workspace: {}", session.workspace_id),
        format!("execution: {}", execution_text(&session.execution)),
        format!("activity: {}", activity_text(session.activity.as_ref())),
    ];
    if session
        .activity
        .as_ref()
        .is_some_and(|activity| activity.seen_at_ms.is_some())
    {
        lines.push("seen: yes".to_owned());
    }
    if let Some(title) = &session.title {
        lines.push(format!("title: {title}"));
    }
    for (label, value) in [
        ("branch", &session.meta.branch),
        ("model", &session.meta.model),
        ("effort", &session.meta.effort),
        ("forked from", &session.meta.forked_from),
        ("preview", &session.meta.preview),
    ] {
        if let Some(value) = value {
            lines.push(format!("{label}: {value}"));
        }
    }
    if session.retained {
        lines.push("retained: yes".to_owned());
    }
    if session.released {
        lines.push("released: yes".to_owned());
    }
    if session.archived {
        lines.push("archived: yes".to_owned());
    }
    // Why an exited session cannot resume (decision 0160); a live run is not resumed anyway.
    if session.execution.state == massorca_protocol::methods::terminals::TerminalState::Exited
        && let Some(resume) = session.resume.as_ref().filter(|resume| !resume.available)
    {
        lines.push(format!(
            "resume: unavailable: {} ({})",
            resume.reason.as_deref().unwrap_or("no reason given"),
            resume.code.as_deref().unwrap_or("unknown")
        ));
    }
    if let Some(mode) = &session.permission_mode {
        // A mode no launch argument carried is only requested until something confirms it (decision 0083).
        let state = if session.mode_confirmed {
            ""
        } else {
            " (requested)"
        };
        lines.push(format!("mode: {mode}{state}"));
    } else if session.mode_source == Some(ModeSource::VendorDefault) {
        lines.push("mode: the CLI's default".to_owned());
    }
    if let Some(pending) = &session.pending_permission_mode {
        lines.push(format!("next resume mode: {pending}"));
    }
    if session.read_only {
        lines.push("read only: yes".to_owned());
    }
    if let Some(terminal) = &session.terminal_id {
        lines.push(format!("terminal: {terminal}"));
    }
    if let Some(agent_ref) = &session.agent_ref {
        lines.push(format!("agent ref: {agent_ref}"));
    }
    if let Some(parent) = &session.parent_id {
        lines.push(format!("parent: {parent}"));
    }
    if session.brought_back_at_ms.is_some() {
        lines.push("brought back: yes".to_owned());
    }
    lines
}

/// What `agent mode` did: when the mode applies, then the session.
pub fn mode_set_lines(set: &AgentModeSet) -> Vec<String> {
    let session = &set.session;
    let applied = match set.applied {
        ModeApplied::Live => format!(
            "switched to {} in the running session (requested: no hook confirms it)",
            session.permission_mode.as_deref().unwrap_or_default()
        ),
        ModeApplied::NextResume => format!(
            "{} applies at the next resume",
            session
                .pending_permission_mode
                .as_deref()
                .unwrap_or_default()
        ),
        ModeApplied::Unknown => "mode set".to_owned(),
    };
    let mut lines = vec![applied];
    lines.extend(session_details(session));
    lines
}

/// A delegation tree: the root, then each child indented under it. Result texts are not printed; `--json` carries them.
pub fn tree_lines(tree: &AgentTree) -> Vec<String> {
    let mut lines = vec![tree_line(&tree.root, "")];
    lines.extend(tree.children.iter().map(|child| tree_line(child, "  ")));
    lines
}

fn tree_line(session: &AgentSession, indent: &str) -> String {
    let brought_back = if session.brought_back_at_ms.is_some() {
        "  brought back"
    } else {
        ""
    };
    format!(
        "{indent}{}  {}  execution: {}  activity: {}{brought_back}",
        session.id,
        session.kind,
        execution_text(&session.execution),
        activity_text(session.activity.as_ref())
    )
}

/// What a bring-back did: the merge, the prompt and its note.
pub fn bring_back_lines(result: &BringBackResult) -> Vec<String> {
    let mut lines = vec![format!("brought back session {}", result.child.id)];
    lines.push(match &result.merge_commit {
        Some(commit) => format!("merged: {commit}"),
        None => "merged: no".to_owned(),
    });
    lines.push(format!(
        "prompt delivered: {}",
        if result.prompt_delivered { "yes" } else { "no" }
    ));
    if let Some(note) = &result.note {
        lines.push(format!("note: {note}"));
    }
    lines
}

/// The sessions as a table, newest first.
pub fn sessions_lines(sessions: &[AgentSession]) -> Vec<String> {
    if sessions.is_empty() {
        return vec!["no agent sessions".to_owned()];
    }
    let rows = sessions
        .iter()
        .map(|session| {
            vec![
                session.id.clone(),
                session.kind.clone(),
                execution_with_keeping(session),
                activity_text(session.activity.as_ref()),
                session.title.clone().unwrap_or_default(),
            ]
        })
        .collect();
    workspace_text::table(&["ID", "KIND", "EXECUTION", "ACTIVITY", "TITLE"], rows)
}

/// `live`, `unverifiable`, or `exited (killed)` with how the run ended.
pub fn execution_text(execution: &Execution) -> String {
    let state = terminal::state_text(execution.state);
    match &execution.exit {
        Some(exit) => format!("{state} ({})", terminal::exit_text(exit)),
        None => state.to_owned(),
    }
}

/// One `agent status --watch` line: the session, its execution and its activity.
pub fn change_line(session: &AgentSession) -> String {
    format!(
        "{}  execution: {}  activity: {}",
        session.id,
        execution_text(&session.execution),
        activity_text(session.activity.as_ref())
    )
}

/// The reported activity with where it came from, for example `done (success) from hook`, or `no status` when the
/// agent has reported nothing.
pub fn activity_text(activity: Option<&Activity>) -> String {
    let Some(activity) = activity else {
        return "no status".to_owned();
    };
    let state = match activity.state {
        ActivityState::Working => "working",
        ActivityState::Waiting => "waiting",
        ActivityState::Idle => "idle",
        ActivityState::Done => "done",
        ActivityState::Unknown => "unknown",
    };
    match activity.outcome {
        Some(outcome) if activity.state == ActivityState::Done => {
            let outcome = match outcome {
                Outcome::Success => "success",
                Outcome::Failure => "failure",
                Outcome::Cancelled => "cancelled",
                Outcome::Interrupted => "interrupted",
                Outcome::Unknown => "unknown",
            };
            format!("{state} ({outcome}) from {}", source_text(activity.source))
        }
        _ => format!("{state} from {}", source_text(activity.source)),
    }
}

fn source_text(source: ActivitySource) -> &'static str {
    match source {
        ActivitySource::Hook => "hook",
        ActivitySource::Process => "process",
        ActivitySource::Output => "output",
        ActivitySource::Unknown => "unknown",
    }
}

/// The state of a queued prompt as its CLI word.
fn queue_state_text(state: QueueState) -> &'static str {
    match state {
        QueueState::Waiting => "waiting",
        QueueState::Sending => "sending",
        QueueState::Failed => "failed",
        QueueState::Unknown => "unknown",
    }
}

/// One queued prompt: its id, position, state, the reason it waits or failed, and its text on one line.
pub fn queued_lines(item: &QueuedPrompt) -> Vec<String> {
    let mut line = format!(
        "queued {} #{}: {}",
        item.id,
        item.position,
        queue_state_text(item.state)
    );
    if let Some(reason) = &item.reason {
        line.push_str(&format!(" ({reason})"));
    }
    line.push_str(&format!(" {}", item.text.replace('\n', " ")));
    vec![line]
}

/// The queue of a session, head first, one line per prompt.
pub fn queue_lines(list: &AgentQueueList) -> Vec<String> {
    if list.items.is_empty() {
        return vec![format!("no queued prompts for session {}", list.session)];
    }
    list.items.iter().flat_map(queued_lines).collect()
}

/// The execution of a session, with what keeps its terminal or output: `retained`, `released`, or both (decision 0090).
fn execution_with_keeping(session: &AgentSession) -> String {
    let mut text = execution_text(&session.execution);
    for (flag, word) in [
        (session.retained, "retained"),
        (session.released, "released"),
        (session.archived, "archived"),
    ] {
        if flag {
            text.push_str(", ");
            text.push_str(word);
        }
    }
    text
}

/// What `agent forget --preview` found (decision 0163): whether the session can be forgotten, and whether and why its
/// workspace could be removed with it.
pub fn forget_preview_lines(
    preview: &massorca_protocol::methods::agents::AgentForgetPreview,
) -> Vec<String> {
    let yes_no = |flag: bool| if flag { "yes" } else { "no" };
    let workspace = &preview.workspace;
    let mut lines = vec![
        format!("session: {}", preview.session),
        format!("can forget: {}", yes_no(preview.can_forget)),
    ];
    if let Some(blocker) = &preview.forget_blocker {
        lines.push(format!("forget blocked: {blocker}"));
    }
    let kind = match workspace.kind {
        massorca_protocol::methods::workspaces::WorkspaceKind::Worktree => "worktree",
        massorca_protocol::methods::workspaces::WorkspaceKind::Folder => "folder (its files stay)",
        massorca_protocol::methods::workspaces::WorkspaceKind::Unknown => "unknown",
    };
    lines.push(format!("workspace: {} ({kind})", workspace.id));
    if let Some(path) = &workspace.path {
        lines.push(format!("path: {path}"));
    }
    if let Some(branch) = &workspace.branch {
        lines.push(format!("branch: {branch}"));
    }
    lines.push(format!("removable: {}", yes_no(workspace.removable)));
    if let Some(blocker) = &workspace.blocker {
        lines.push(format!("blocked: {blocker}"));
    }
    lines.push(format!("uncommitted changes: {}", yes_no(workspace.dirty)));
    lines.push(format!("untracked files: {}", yes_no(workspace.untracked)));
    if workspace.ignored_files {
        lines.push("ignored files: yes".to_owned());
    }
    for (what, ids) in [
        ("other sessions", &workspace.other_sessions),
        ("teams", &workspace.other_teams),
        ("tasks", &workspace.other_tasks),
        ("running terminals", &workspace.running_terminals),
    ] {
        if !ids.is_empty() {
            lines.push(format!("{what}: {}", ids.join(", ")));
        }
    }
    lines
}

/// The archived output of a released session, one line per line of its text (decision 0090), or why there is none.
pub fn archive_lines(archive: &AgentArchive) -> Vec<String> {
    if !archive.released {
        return vec!["not released: the session has no archived output".to_owned()];
    }
    if archive.withheld {
        return vec![
            "withheld: the run holds vault secrets; its output is for host-admin callers"
                .to_owned(),
        ];
    }
    let mut lines = Vec::new();
    if archive.truncated {
        lines.push("(older output was cut to keep the newest 256 KiB)".to_owned());
    }
    match &archive.text {
        Some(text) if !text.is_empty() => lines.extend(text.lines().map(str::to_owned)),
        _ => lines.push("(no output)".to_owned()),
    }
    lines
}
