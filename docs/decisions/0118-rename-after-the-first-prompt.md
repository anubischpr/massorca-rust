# 0118: Rename after the first prompt

Status: accepted, 2026-11-26.

## Context

- pv96 names a worktree's branch after the first agent work says what the workspace is for. Decision 0111 added the
  rename primitive (`workspace.rename`), and decision 0167 the one-shot title (`agent.generate_title`). Nothing connected
  them, so a generated workspace kept its creature name.
- A name a person typed is never renamed (decision 0111). The trigger must respect that before it asks for a title.

## Decision

- **Setting.** `workspaces.rename_from_first_prompt` is a boolean core setting, `true` by default. It is read when the
  trigger fires, and `false` turns the rename off. `massorca config set` changes it.
- **Trigger.** The service handles the first prompt of a session that it starts or that `agent.prompt` delivers: the
  start's own prompt, or the first prompt sent to the session. A prompt the prompt queue types later does not trigger.
  Delegated children never trigger.
- **Once per workspace.** The first prompt of a workspace's first session decides it. Later sessions of the workspace
  do not rename it, whatever their prompts.
- **Run.** The title run is a background task, so no prompt waits for it. It asks `agent.generate_title`'s domain call
  for a slug with the session's kind, its model when the start named one, the active account, and the prompt; it sends
  no reply. Then it calls `workspace.rename` with `from_generated: true`, and for a worktree `expected_branch` set to the
  branch the trigger saw.
- **Outcomes.** A rename, a setting that is off, and the definitive refusals (`workspace.name_not_generated`,
  `workspace.branch_published`, `workspace.branch_changed`) settle the workspace for the run. Any other failure, such as a
  failed title, leaves the name and lets the same session's next prompt retry once. A second failure settles it.
- **Logs.** Each outcome is one service log line that names the workspace, so a failure explains itself.

## Consequences

- The memory of which workspaces are decided is held in the service and is not stored. After a restart a session started
  before it is never a first prompt, but a session started after it can rename a workspace that an earlier run already
  renamed, because the workspace keeps `name_generated`. Keeping the decision across restarts needs a stored flag, which
  this slice does not add.
- The title comes from the kind's one-shot form (decision 0167): the prompt, not a reply, is what the title reads. The
  kind's model is known only for a session started in this run; after a restart the kind's default model is used.
- No protocol message changes. The setting is a core setting, so `config list` shows it and the desktop settings page can
  show it from the registry.
- Debug entry point: `massorca workspace rename <WORKSPACE> <NEW_NAME> --from-generated` and `massorca agent title`
  (decisions 0111 and 0167); the trigger needs no separate command.
