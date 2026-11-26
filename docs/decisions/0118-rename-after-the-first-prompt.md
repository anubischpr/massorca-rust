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
- **Trigger.** The first prompt of a session that the person sends names its workspace. Two paths reach the same
  function: the start's own prompt (`agent.start`), and every prompt of the person that the service writes to a session,
  whether `agent.prompt` sent it or the prompt queue typed it later. Delegated children never trigger, and neither do
  system prompts such as a delegated child's report.
- **Once per workspace.** The first prompt of a workspace's first session decides it. Later sessions of the workspace
  do not rename it, whatever their prompts.
- **Run.** The title run is a background task, so no prompt waits for it. It asks `agent.generate_title`'s domain call
  for a slug with the session's kind, its model when the start named one, the active account, and the prompt; it sends
  no reply. Then it calls the trigger's domain rename with `from_generated: true`, and for a worktree `expected_branch`
  set to the branch the trigger saw.
- **The rename ends the generated state.** A successful trigger rename writes the name, the branch and
  `name_generated = false` in one store write (decision 0111). The name is then the person's, so no later run, after a
  restart or otherwise, renames the workspace again. This is internal: the wire's `workspace.rename` keeps the flag.
- **Outcomes.** A rename, a setting that is off, and the definitive refusals (`workspace.name_not_generated`,
  `workspace.branch_published`, `workspace.branch_changed`) settle the workspace in memory for the run. A workspace whose
  flag is already off is settled by the first check of the run, before any title is asked for. Any other failure, such
  as a failed title, leaves the name and lets the same session's next prompt retry once. A second failure settles it.
- **Logs.** Each outcome is one service log line that names the workspace, so a failure explains itself.

## Consequences

- The in-memory record of which workspaces are decided is only a shortcut for one run. The durable guard is the flag: a
  trigger rename clears it, so a restart cannot rename the same workspace again.
- A workspace whose trigger rename never happened (a failed title, a setting that was off) keeps its generated name. A
  later run can still rename it, as the in-memory record allows for one run only.
- The desktop and CLI show a trigger-renamed workspace as a typed name (`workspace show` no longer prints
  `name generated: yes`).
- A prompt's text is never printed: the listener's event type has no `Debug`.
- The title comes from the kind's one-shot form (decision 0167): the prompt, not a reply, is what the title reads. The
  kind's model is known only for a session started in this run; after a restart the kind's default model is used.
- No protocol message changes. The setting is a core setting, so `config list` shows it and the desktop settings page can
  show it from the registry.
- Debug entry point: `massorca workspace rename <WORKSPACE> <NEW_NAME> --from-generated` and `massorca agent title`
  (decisions 0111 and 0167); the trigger needs no separate command.
