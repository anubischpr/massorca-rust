# 0129: Team messages

Status: accepted, 2026-11-26.

## Context

- Inventory AUTO-18 describes agent-to-agent messages for the orchestration Runs model: `orchestration send` with nine
  types, three priorities, Run, dispatch, terminal and group addressing, an inbox, replies by id, and delivery contracts.
  That model (AUTO-15, AUTO-16, AUTO-17) does not exist in this product.
- Owner direction (2026-11-12): children report back to the lead bot or the coordinator, and the delegation tree shows
  each child's status (X-DL-01).
- Decision 0128 built Bot Teams delegation: a bot's task run delegates a child task to a teammate. Its gaps include
  "a delegated child's result is not sent as a prompt to the lead's run". This decision is the first version of AUTO-18
  for those delegations, and closes that gap (AUTO-20's "parent not woken").

## Decision

- **Who.** `team.message_send`, `team.message_reply` and `team.inbox` (capability `team-messages/1`) are called by a
  bot's task run with its session and run token, as `team.delegate` is (decision 0128). A token no live run holds is
  refused with `permission.denied`; a call with no run, or a run with no task or bot, with `team.message_not_a_bot`.
  The sender is always the run's own task and bot: no parameter names a sender.
- **Edges.** A message goes to the sender task's parent (`to: "parent"`, or the parent's id) or to a child task the
  sender's task delegated (its id). Anything else, a sibling or an unknown task included, is refused with
  `team.message_no_edge`, which never says whether the task exists.
- **Content.** `type` is `status`, `question`, `worker_done`, `handoff` or `escalation`; a newer peer's type decodes as
  unknown and is refused as invalid. `priority` is `normal` (default), `high` or `urgent`. The subject is one line of at
  most 200 characters; the body is at most 6000 bytes and holds no control character other than tab and line breaks,
  so a delivered message fits one agent prompt and can never drive the terminal. A message opens a thread named by its
  id unless it names a thread; `reply_to` keeps the answered message's thread, and must name a message the sender sent
  or received (`team.message_unknown` otherwise). Bodies are never logged; `Debug` of every params type redacts the
  body and the token.
- **worker_done.** It needs an outcome, `succeeded` or `failed`; any other type carrying one is invalid. It settles the
  task it names, by default the sender's own task, which must be the sender's own task and a child of the recipient
  (`team.message_unknown_task`, `team.message_not_your_child`). The message is stored and the task's outcome latched
  (succeeded is Done, failed is Stuck) in one transaction. A second `worker_done` for the same task is a duplicate:
  nothing changes and the first message comes back with `duplicate: true`. A task whose own run already ended it with
  the same outcome takes the message without a second latch; one that ended with another outcome refuses with
  `team.message_already_settled`, whose data names that outcome.
- **Waking the lead.** A `worker_done`, or a `high` or `urgent` message, sent up to a parent is queued into the
  parent's run when that run is live or unverifiable (loss of contact is never an ended run). It goes through the
  agent's own prompt queue (decision 0084) under the request id `team-message-<id>`, so it reaches the agent at its next
  idle, once. A kind without a queue gets the prompt typed when it is ready. The queued prompt is not a host-admin
  one, so a parent run holding vault secrets does not take it. The prompt frames the message as a teammate's
  information, not the person's instruction, and ends with the reply command. Any refusal leaves the message in the
  inbox; a send never fails because the parent cannot be woken. Messages down the tree and normal ones wait in the
  inbox.
- **Room.** A `worker_done` posts one line by the child's bot in the parent's team room: `Done: <title> (reported to
  <lead>)`, or `(waits in <lead>'s inbox)` when the parent run was not woken. A high or urgent message that waits posts
  `<Priority> <type> for <lead> waits in its inbox: <subject>`. Every message publishes the parent team's room change,
  so its task cards re-read the delegation trees.
- **Inbox.** `team.inbox` lists the run's unread messages, oldest first; `mark_read` marks exactly those read after
  listing them. `team.message_reply` answers a message sent to the caller's task, back to its sender, on its thread,
  with the subject `Re: <subject>` and the type `status` unless one is given.
- **Tree.** `team.delegation_tree` and `team.delegation_list` give each child a `report`: the subject of its latest
  `status` message to its parent and the outcome of its `worker_done` (an optional field, absent when empty and from
  older hosts). The desktop's room task card lists each child as `title · state · reported <outcome> · <status>`.
- **Surfaces.** CLI, only inside a bot's run (refused before any connection otherwise): `massorca team message send
  --to parent|<child task> --type <t> [--priority p] --subject s --body b [--outcome succeeded|failed] [--reply-to id]`,
  `massorca team inbox [--mark-read]`, `massorca team message reply <id> --body b [--type t] [--priority p]`.

## Mapping to AUTO-18

| AUTO-18 | Here |
| --- | --- |
| status, question, worker_done, handoff, escalation | Built |
| dispatch, merge_ready, decision_gate, heartbeat | Not built; they decode as unknown and are refused |
| Priorities normal, high, urgent | Built; high and urgent wake a live parent |
| Addressing a Run, a dispatch, a terminal handle | A delegation edge: `parent` or a child task id |
| Groups (`@all`, `@idle`, provider groups, `@worktree:<id>`) | Not built |
| worker_done settles exactly the named task; typed rejections; duplicates | Built for tasks: unknown task, not your child, already settled, duplicate |
| Rejections for dispatches (unknown, mismatch, inactive, stale) | Not built: there are no dispatches |
| check / inbox, reply by id | `team.inbox`, `team.message_reply` |
| Delivery contracts and restart reconciliation | The agent prompt queue keeps a woken message across a restart; the inbox keeps every message |

## Not built (gaps)

- Groups and broadcast, dispatches, the four remaining types, and payloads.
- Only task runs message: a bot's reply run in the Conference room cannot, as it cannot delegate (decision 0128).
- A `worker_done` names only the sender's own task; a lead cannot settle a child on its behalf.
- A message waiting in the inbox of a parent whose run ended is not delivered when that run is resumed; the lead reads
  it with `team inbox` or in the room.
- The desktop shows reports on the room task card only, not on the Board card, and has no inbox view.
- No end-to-end test runs a live bot that calls `team message`: the fake agent cannot run the CLI with its token. The
  edges, settlement, duplicates, inbox, replies, the wake decision (live, unverifiable, ended) and the room lines are
  unit tests of the service module, with the wake call stubbed; the refusals run over the wire and through the CLI.

## Migration

- One step: `team_messages` (sender and recipient task, their bots, type, priority, subject, body, thread, reply-to,
  outcome, settled task, delivery, read time, creation time), with indexes by recipient and by sender, and a unique
  index on the settled task. Removing either task removes the message; removing a bot keeps it without the bot. No
  existing row changes.

## Consequences

- A delegated child can report without the lead polling, and the lead hears a finished child at its next idle.
- The edge rule keeps messages inside one delegation tree, so a bot cannot reach a teammate it neither delegated to nor
  works for.
- When the Runs model arrives, these messages are its delegation-edge subset; the wire names (`type`, `priority`,
  `outcome`) already match AUTO-18's.
