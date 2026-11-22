# 0051: Agent sessions, workspaces and terminals

Status: accepted, 2026-11-21.

## Context

An agent runs as a terminal program (AG-04): in the old app a worktree's
agent was its workspace terminal, with a session record learned from hooks.
Here the service already owns terminals that outlive every client (0033),
and the shell field of a terminal is the program the terminal is (0032).
The owner's placement rule for delegation is that two sessions never write
in the same worktree at the same time (`docs/inventory/agents.md`, AG-15
owner note).

## Decision

- **An agent session is a record in the service.** Fields: `id` (UUID v7,
  never reused), `kind`, `workspace_id`, `title`, `read_only`, `parent_id`
  (delegation, 0055), `agent_ref` (the agent's own conversation id, when
  known), `terminal_id` of the current run, `created_at_ms`, execution and
  activity (0052) and the last `result` text.
- **A session runs in one service-owned terminal.** `agent.start` creates a
  terminal in the session's workspace whose program is the agent CLI itself,
  spawned directly (no shell in between), so the terminal's exit is the
  agent's exit and `terminal.kill` ends the agent's whole process tree
  (0031). Everything terminals do applies unchanged: any number of clients
  attach, a client leaving never stops the agent, attach resumes or
  snapshots. `agent attach` is `terminal attach` on the session's current
  terminal.
- **A session belongs to one workspace** for its whole life and starts in
  the workspace root. Removing a workspace removes its session records.
- **One writer per workspace.** A workspace holds at most one live session
  that is not `read_only`. A second writing `agent.start` is refused with
  `agent.workspace_busy`, naming the session that holds it. `read_only` is a
  declaration recorded on the session and used for placement, not a sandbox:
  MassOrca cannot stop an agent from writing.
- **Sessions persist; runs do not.** Session records live in SQLite
  (`agent_sessions`), so listing per workspace and resume survive a service
  restart. Terminals still end with the service (0033): on start the service
  marks every session whose run was live as `exited` with reason
  `service_stopped`.
- **Resume starts a new run in the same session** when the kind declares a
  resume form and `agent_ref` is known (0054). The session keeps its id; it
  gets a new `terminal_id`.
- **Stop** kills the current run's terminal (graceful, then forced after the
  terminal's grace; `force` at once). The record stays, `exited` with reason
  `killed`. `agent.forget` deletes an exited session record.
- **Identity on the wire and in the run.** The agent's environment carries
  `MASSORCA_AGENT_SESSION` (the session id), `MASSORCA_AGENT_TOKEN` (0053),
  `MASSORCA_AGENT_KIND` and `MASSORCA_BIN` (the absolute path of the
  `massorca` executable that runs the service), next to the terminal
  variables of 0032. So `massorca` commands run by the agent or its hooks
  reach the same profile and know which session they come from.
- **Retries.** `agent.start` and `agent.delegate` take a `request_id`; a
  repeated id returns the first session.

## Alternatives

- **A separate agent process host.** The service already outlives clients;
  the same reasoning as 0033.
- **An agent typed into a shell terminal.** The shell would survive the
  agent, so the terminal's state would say nothing about the agent, and a
  kill would hit the shell first.
- **Several writing sessions per worktree.** Matches observed behaviour but
  not the owner's rule; it lets two agents overwrite each other's edits.

## Consequences

- Restarting the service ends every agent run; resume brings back kinds
  that support it.
- The terminals list shows agent terminals too, with the agent's program as
  their shell; the agent session links to its terminal, not the reverse.
