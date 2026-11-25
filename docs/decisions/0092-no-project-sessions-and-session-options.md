# 0092: Sessions with no project, and model and effort

Status: accepted, 2026-11-24.

## Context

Design agreement A95 says every project menu offers "No project", and that conversations without a project are saved in a
folder set in Settings (General), default `~/MassOrca/Conversations`, and run with no worktree. Until now `agent.start`
required a `workspace`, and a session row points at a workspace. Agent CLIs also let the person pick a model and an
effort, which a start had no way to pass.

## Decision

- `agent.start` takes an optional `workspace` (capability `agent-no-project/1`). Without it the session runs in the
  conversations folder, in a folder of its own named after the session id. The folder is the setting
  `conversations.folder` (already declared, default `<home>/MassOrca/Conversations`), created on first use. A
  non-absolute value is refused by the setting and, if stored, by the start.
- Why a folder of its own per session: a vendor CLI keeps its history per folder, so two sessions sharing one folder
  would share history. Why the session id as the name: it is unique, so the workspace name and path never collide.
- A folder workspace needs a project. The conversations folder is registered under a hidden project, marked with the
  new column `projects.conversations` (migration 0092, appended). Every project listing leaves hidden projects out. This
  is the one schema change: the alternatives (a nullable `agent_sessions.workspace_id`, or a project per session) would
  reach every session call site or show in the project list.
- The hidden project is never selected by id, name or path, so no project command acts on it, and `project inspect`
  reports the folder as unregistered. Adding the conversations folder as a project is refused with `project.exists`.
  A project a person already added at the conversations folder makes a start with no project fail with
  `config.invalid_value`, naming that project.
- A start that fails after its conversation folder was made removes the folder's workspace, and the folder while it is
  still empty, so a failed start leaves nothing in the workspace list.
- The client refuses a start with no workspace when the service does not offer `agent-no-project/1`, and a start with a
  `model` or an `effort` when it does not offer `agent-options/1`, with `ClientError::CapabilityMissing`, before sending.
  The new field is `skip_serializing_if` absent, so an older peer never sees a `workspace: null` it would misread.
- `agent.start` takes optional `model` and `effort` (capability `agent-options/1`). A kind declares the flags they take:
  `"model": {"flag": "--model"}` and `"effort": {"flag": "--effort"}`, optional fields of a kind declaration. The
  values follow the flag after the mode's arguments and before the caller's. A kind without the flag refuses the option
  with `agent.option_unsupported`, naming the kind. A value is 1 to 128 letters, digits, `-`, `_`, `.`, `:` or `/`, and
  must not start with `-`.
- A resumed run does not keep the model or effort of its first start. Storing them would need a column; a resume uses
  the kind's default, which is the behaviour before this decision.

## Consequences

- Schema: one column on `projects`, appended. Existing rows are unchanged (`DEFAULT 0`).
- Wire: `AgentStartParams.workspace` is optional; `model` and `effort` are new optional fields. Both are gated by
  capabilities, so a peer that predates them is never sent a request it would misread.
- A workspace list without a project filter still shows the conversation folders, because they are real folder
  workspaces. The project list does not.
- The built-in `claude` kind declares `--model` and `--effort`, the flags the reference's Claude catalog passes
  (`src/shared/agent-session-option-catalog-claude-codex.ts` at d3c6311a531). The other built-in kinds declare none
  and refuse both options until their declarations are checked against their CLIs.
