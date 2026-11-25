# 0102: The team Conference room

Status: accepted, 2026-11-25.

## Context

Decision 0072 gave Bot Teams its teams, bots, Board and tasks, and reserved a task origin `room` for tasks the Conference
room creates. No room existed: the desktop drew a placeholder (design audit C1 to C9). In pv96, a person's line in a
team's room is stored, then routed to one bot: the bot it names with `@`, else a one-shot classify call picks one. That bot
gets a task, which starts its own agent session. The bot posts back through a CLI, and the host posts when a task ends or
asks a question. pv96 has no lead bot and no room-wide session, and its desktop polls the room every five seconds.

This decision is the smallest slice that makes the room real end to end: lines are stored and read, a person's line
becomes a started task, and the bot answers.

## Decision

- Capability `team-room/1` holds `team.room_list` (read) and `team.room_post` (workspace), plus the topic
  `team.room_changed`, whose id is the team id.
- A line is a row of the new table `team_room_messages`: `author` `person` or `bot`, the bot's id for a bot line, the text,
  the task it is about, and when it was posted. The table is appended as a migration step and no existing row changes. A
  team's removal removes its room. A bot's removal keeps its lines without the bot, so the room keeps its history.
- `team.room_list {team, limit?}` returns the newest `limit` lines (200 without it), oldest first. Each line carries its
  bot's name and, for a line about a task, the task as the Board shows it now.
- `team.room_post {team, text}` posts a person's line, at most 4000 characters:
  1. The line goes to one bot of the team: the bot it names with `@name` (not preceded by a letter or digit; the longest
     matching name wins), else the bot given as `to`, else the team's lead bot, else its first bot. `to` is the
     receiver the desktop composer picks, which defaults to the lead. A team without bots refuses the line with `task.no_bot`, "Add a
     bot to this team first.", and stores nothing.
  2. The line becomes a task with origin `room`. Its title is the line's first non-blank line, cut to 80 characters; its
     body is the whole line; its bot is the bot chosen.
  3. The task starts through the same path as `task.start`, with the same vault grant and circuit breaker.
  4. The bot answers with a line about the task: "Added to the Board: {title}", or "Added to the Board. It did not start:
     {reason}" when the start failed. A task that does not start stays Open, as it does from `task.start`.
  The call returns the person's line and the answer, and publishes `team.room_changed` and `task.changed`.
- `team.room_post {team, text, bot}` posts as that bot of the team, for a person at the CLI. It creates no task.
- An agent run posts only as itself. The CLI run inside an agent sends the run's `session` and `agent_token`
  (`MASSORCA_AGENT_SESSION`, `MASSORCA_AGENT_TOKEN`). The service checks the token against that session's live run,
  finds the team's task that run works on, and posts as that task's bot, about that task. A token that does not match,
  a run with no task of the team, or a `bot` naming another bot is refused with `permission.denied`. The token is never
  printed by `Debug`. This binds an honest agent, which runs the CLI it is given; it is not a boundary against a
  program that drops its own environment, since an agent run already holds the owner's CLI connection.
- When a room task's outcome latches, its bot posts: "Done", "Stuck", "Stopped" or "Interrupted", then the title. Both
  places that latch an outcome write it: the outcome listener, and the start that finds its agent already finished
  when it links the session. A latch sets the outcome only once per run, so each run posts once; a task reopened and
  run again posts again. It is posted whether or not a client is attached.
- The CLI is `massorca team room list <team> [--limit N]` and
  `massorca team room post <team> <text> [--bot <bot> | --to <bot>]`.
- Lines are stored as given, at most 4000 characters, under the service's existing redaction rules.

## Not copied from pv96, or deferred

- The classify call that picks a bot when none is named. The lead, else the first bot, is deterministic and needs no
  extra agent run. A later decision can add the classify step.
- Polling. The service publishes `team.room_changed`, so a client reads the room again only when it changed.
- Questions a waiting task asks, routines created from the room, delegation, attachments, reply threads and cross-team
  requests. Each needs its own decision.
- Reply and task ids stored inside the text. They are columns here.

## Consequences

- An older client does not see the capability and keeps the placeholder room.
- A line posted while the service cannot start agents still becomes an Open task and an answer that says why it did not
  start, so nothing the person wrote is lost.
- A task's outcome is posted once: the line is written only when the outcome latches, and a repeated latch writes nothing.
