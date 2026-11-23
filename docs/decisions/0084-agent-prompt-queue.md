# 0084: Agent prompt queue

Status: accepted, 2026-11-23.

## Context

`agent.prompt` (0054) refuses with `agent.busy` while its agent is working or waiting, so the desktop composer can only
offer Stop while an agent is busy. The composer needs to queue prompts for a busy agent. The queue belongs to the
service, not to a client: it survives a client restart, works for every client, and works for a host reached remotely.

The owner's brief (L1) asks for a queue per agent session with the calls `agent.queue`, `agent.queue.list`,
`agent.queue.edit`, `agent.queue.remove` and `agent.queue.send_now`, delivery when the agent turns idle, done or waits
for input, a status per item, an event on change, a capability flag and CLI entry points. L3 approved the design below,
including the one deviation noted in D3.

## Reference

The reference is the pv96 desktop, read-only at commit d3c6311a531. File names below are its source files.

Matched:

- Strict FIFO by position, with no reordering; one item is delivered per step, and the delivered prompt gates the next
  (`structured-agent-session-queued-messages.ts:265-269, 327-386`).
- The queue holds while the agent is asking the person something: the gate is `blocked | working | prompt`
  (`structured-agent-session-queued-messages.ts:93-121`).
- A failed delivery marks its item failed and holds everything behind it; nothing is retried automatically
  (`queued-message-pause.ts:267, 285-287`).
- A stop by the person pauses the items queued before it, as `stopped`; a restart pauses them as `restarted`; an
  accepted turn or an explicit resume lifts the pause (`queued-message-pause.ts:1-14, 168-206`, and `:7-11` for the
  restart).
- Send now overrides only the working hold; it is refused while the agent is asking, and the sent text is not kept as
  an item (`structured-agent-session-queued-mutations.ts:111-189`).
- The queue survives a restart in a table keyed by session and message (`journal-queued-messages.ts:75-80`,
  `queued-message-schema.ts:26-47`, `queued-message-table.ts`).

Deviations:

- A cap of 100 prompts per session. The reference has no count limit, only a frame byte bound. The cap protects the
  host; a full queue is refused with `agent.queue_full`.
- Edit in place. The reference copies a card's text into the composer and deletes the card. Here `agent.queue.edit`
  replaces the text and keeps the item's position and id.
- Kinds with no status mechanism have no queue (S3). The reference has no such case.

## Decision

D1 Store. Table `agent_prompt_queue (id, session_id, position, text, request_id, state, reason, created_at_ms,
updated_at_ms)`, added as a new schema step after the permission-mode step. `position` counts from 1 per session;
`request_id` is unique per session when given. Removing a session removes its queue (a foreign key with cascade).

D2 States on the wire. `waiting` (with a reason), `sending`, or `failed` (with the failure as its reason). The reason of
a waiting prompt is one of `busy`, `asking`, `paused_stopped`, `paused_restarted`, `behind_failed`. "Host unverifiable"
is a client's own state when it cannot reach the host; the service never reports it.

D3 Delivery. Strict FIFO, one item per step. The head is delivered when the agent's activity turns `idle` or `done`,
through the same guards as `agent.prompt` (merging, switching, readiness, typed since the last report). The queue does
not deliver while the agent is `waiting`: typing queued text into a question could answer it. This is the one
deviation from the brief's "waiting for input"; it follows the reference's gate, and L3 approved it.

D4 Failure. A failed delivery marks the item `failed` with its reason and holds every item behind it. There is no
automatic retry. `agent.queue.send_now` on a failed item retries it; `agent.queue.remove` clears it. Editing a failed
item keeps it failed.

D5 Pauses. `agent.stop` by the person pauses the waiting items (`paused_stopped`). A service restart pauses them all
(`paused_restarted`), and a prompt being sent fails with "the service restarted while sending". An accepted
`agent.prompt`, a `send_now`, or `agent.queue.resume` lifts the pause. A resumed run does not deliver paused items by
itself. A prompt the system sends, such as a bring-back's automatic prompt, does not lift a pause: only the person's
`agent.prompt`, `send_now` or `agent.queue.resume` does (N6). The person's stop pauses the queue before the run is killed,
so a grace-period report cannot deliver to a dying run.

D6 `send_now`. Sends one item at once with force, skipping busy and FIFO. It is refused while switching, while a
workspace merge holds the session, or while the agent is asking (`agent.busy`). On success the item leaves the queue.

D7 Edit and remove. Only for waiting or failed items; a sending item is refused with `agent.busy`. The text limit is the
prompt limit (`check_prompt`). The queue holds at most 100 prompts per session (deviation above).
A queued prompt keeps whether its caller held host-admin scope. Once its run holds vault secrets (decision 0071), a
prompt queued without that scope is not typed: it fails with the reason "queued by a caller without host-admin; the run
now holds secrets", which names no secret, and holds the prompts behind it. A host-admin `send_now` may retry it under
the 0071 check.

D8 Kinds without a status mechanism. A kind that reports no activity has no queue: `agent.queue` (add) is refused with
`agent.queue_unavailable`, and prompts go straight through as before. Prompts already queued stay listable, editable,
removable and sendable (N1): only adding is refused.

D9 Events and capability. Every queue change publishes `agent.queue_changed` with the session id. Queue changes are not
sent to `agent.watch` streams. The capability is `agent-queue/1`. CLI entry points: `massorca agent queue add`,
`list`, `edit`, `remove`, `send-now` and `resume`.

D10 Write failures. A write that fails partway marks the prompt failed; retrying it types the whole text again, so the
agent may already have read part of it. Nothing retries on its own.

D11 Dedupe window. A request id is remembered for the session's last 100 delivered prompts (the window is that count of
the most recent ids, not a span of time). A retry with a remembered id is refused with `agent.queue_already_delivered`
and is not typed again. A prompt queued without a request id is not remembered.

## Consequences

- A prompt queued on a session with no live run waits; it is delivered after a resumed run reports its first idle.
- A direct `agent.prompt` is not ordered against the queue. A caller that wants ordering uses the queue.
- A queued prompt is typed on a thread of its own after its hook report returns, so a report never waits for the write.
- A retry of a delivered prompt is refused after 100 later deliveries in the same session, and then types again.
- Delivery is attempted on every hook report, after a mode switch ends, and when a queue call changes what is waiting.
- A queue holds the text of prompts, so reading it needs the same host-admin scope as a prompt to a run that holds vault
  secrets.
