# 0101: Structured chat sessions

Status: accepted, 2026-11-24.

## Context

Every agent session today runs its vendor CLI in a service-owned terminal (0051). The desktop draws that terminal; the
service knows only what hooks report (0053), and `agent.watch` carries whole session records. The desktop's Chat
segment needs more: a typed transcript it can draw and replay, permission prompts it can answer, an interrupt that
keeps the session, and paging of older history. Inventory rows AG-08 (structured launch arm), AG-15 (chat platform),
AG-16 (chat controls), AG-17 (Claude structured adapter), AG-03 (version probing) and HIST-06 (transcript paging) ask
for it.

The owner approved, for the first version:

- Claude only. The service speaks Claude Code's stream-json control protocol directly, with no Node sidecar.
- The supported Claude CLI version range is pinned by probing (AG-03). Outside it a chat start is refused cleanly and
  the client falls back to a terminal session with a clear message.
- Capability-gated, so the desktop's Chat segment turns on by itself.
- Approvals default to asking; a cancelled or unanswered request never authorizes.
- Real-CLI verification is the owner's job; this decision lists what only a real run can confirm.

This decision fixes the wire (the types in `crates/protocol/src/methods/chat.rs`). The service that serves it comes in
later slices.

## Reference

The reference is the pv96 desktop, read-only at commit d3c6311a531. Matched:

- A host-owned journal of items with stable ids, an epoch-qualified cursor, and a subscribe stream of `snapshot`,
  `batch`, `reset` and `end` (`src/shared/agent-session-wire.ts`).
- Batches carry an item's current state rather than a delta, so applying one twice converges (same file,
  `AgentSessionJournalBatch`).
- The item vocabulary: message, tool call, diff, approval, status, turn; turn lifecycle `running | completed |
  interrupted | unverifiable`; approvals as durable items with a resolution state so a second answer loses
  (`src/shared/agent-session-journal-types.ts`).
- A snapshot names the newest turn even when its item is older than the page (`latestTurn`).
- Permission prompts are keyed by the CLI's request id; a `control_cancel_request` forgets the prompt and never
  answers it, and a malformed request is denied (`src/main/claude/claude-structured-inbound-control.ts`).
- Claude's frame vocabulary (`src/main/native-chat/agent-session-wire/claude-stream-json-frame-schema.ts`).

Deviations:

- Text deltas. The reference republishes a message's whole state on every change. Here an `append` update carries only
  the new text with the byte `offset` it starts at, so a long reply does not cost its square in bytes; the offset keeps
  a repeated append harmless (D4).
- No questions, rewind, compaction, goals, background tasks or slash-command catalogue in the first slice (D10).

## Decision

D1 A separate start method. A chat starts with `agent.chat.start`, whose params are the `agent.start` params at the top
level (`AgentChatStartParams` flattens `AgentStartParams`), instead of a `run` field on `agent.start`. An older service
ignores a field it does not know, so `agent.start { run: "chat" }` sent to it would start a terminal session and report
success; a separate method fails with `method.not_found` instead. It also keeps every existing `agent.start` caller
and struct unchanged. `cols`, `rows` and `args` do not apply to a chat and are refused with `request.invalid_params`.
The result is the session record, as for `agent.start`. A kind that cannot run as a chat is refused with
`agent.chat_unsupported`, whose `data` is the kind's `KindChat`; the client then offers `agent.start` and says why.

D2 The session stays an agent session. A chat session is one `AgentSession`: `agent.show`, `agent.list`, `agent.watch`,
`agent.stop`, `agent.resume`, `agent.set_mode`, `agent.forget`, delegation and the queue work on it. The record gains
`run: "terminal" | "chat"` (the `RunType` enum in `chat.rs`; absent means `terminal`) when the service slice lands: its
constructors live in `core-agents`, outside this change, and a field nobody fills would mislead. Until then a client
knows a session is a chat because it started it with `agent.chat.start`, or because `agent.chat.subscribe` does not
refuse it with `agent.not_chat`. A chat session has no `terminal_id`.

D3 The journal. The service keeps, per session, an append-only journal of changes. Each change gets the next `seq`
(from 1; 0 is "before the first change"). An item has a stable `id`, a `position` (the `seq` at which it was added,
which orders the timeline and never changes), a `seq` (when it last changed), an optional `turn` (the turn item it
belongs to), an optional `parent` (the sub-agent item whose run produced it), `at_ms`, and a body tagged by `type`:
`user_message`, `assistant_message`, `thinking`, `tool_use`, `plan`, `diff`, `subagent`, `approval`, `notice` and
`turn`. A turn is an item whose `id` is the turn id. A cursor is `{ epoch, seq }`; a rebuilt journal gets a new epoch.
Summaries, outputs and patches are cut at 16 KiB on a character boundary and marked `truncated`.

D4 The stream. `agent.chat.subscribe { session, after?, limit? }` returns a stream, as `events.subscribe` does. Without
`after`, the first event is a `snapshot { cursor, items, has_older, latest_turn?, activity? }` of the newest page
(40 items by default, 200 at most). With a current `after`, the first event is a `batch` of everything after it. Then
`batch { cursor, updates, activity? }` events follow, where an update is `item { item }` (the item's whole current
state: replace by id or add) or `append { id, seq, offset, text }` (text added to an `assistant_message` or
`thinking`: apply when the held text is exactly `offset` bytes, skip when it is longer, resubscribe when it is
shorter). When `after` is older than the replay window, names another epoch, or the subscriber overflows its buffer,
the stream sends `reset { reason }` and then a fresh snapshot. `end { reason }` closes it (the session was forgotten,
or the service shuts down). The snapshot carries `latest_turn` and not a list of turns: the open turn is what a client
needs to show a running state, and the page already holds the turn items in its range.

D5 History. `agent.chat.history { session, before?, limit? }` returns `{ epoch, items, has_older }`, the items before
the `position` in `before`, oldest first. A `before` from another epoch is refused with `agent.cursor_stale`; the
client resubscribes.

D6 Approvals. A Claude `can_use_tool` control request becomes an `approval` item `{ request_id, tool, input_summary,
tool_use_id?, can_always, state: pending }` and the session's activity turns `waiting`.
`agent.chat.approval.answer { session, request_id, decision: allow | deny, scope?: once | always, message? }`
answers it; the first answer wins and the result is the approval item after the answer. `always` writes the permission
rules the CLI offered with the request, and is accepted only when the item says `can_always`. `message` is what a deny
tells the agent. A `control_cancel_request`, an interrupt, a stop or the run's end turns every pending approval
`cancelled` and nothing answers the CLI for it. Answering a request that is not pending is refused with
`agent.approval_settled`. Only `allowed` authorizes; `ApprovalDecision` has no unknown variant, so an unrecognised
decision is refused, never read as allow. A request the service cannot decode is denied without an item, with a
`notice`.

D7 Prompts. `agent.prompt` and the queue (0084) are reused unchanged in shape. For a chat session `agent.prompt` writes
a stream-json user message and replies with receipt `sent_to_chat`; the journal shows a `user_message` item. While a
turn runs or an approval is pending, `agent.prompt` is refused with `agent.busy` unless `force`, as for a terminal
session, and the queue delivers its head when the turn ends, through the same gates. A forced prompt mid-turn is
handed to the CLI, which holds it for its next turn.

D8 Interrupt. `agent.chat.interrupt { session }` sends Claude's `interrupt` control request. The running turn ends
`interrupted`, its open tool calls and sub-agents end `interrupted`, its pending approvals end `cancelled`, and the
run and session stay for the next prompt. The result is `{ interrupted, turn? }`; with no running turn it is
`{ interrupted: false }` and not an error. `agent.stop` still ends the run.

D9 Resume. `agent.resume` on a chat session starts the CLI again in stream-json with `--resume <the session's
agent_ref>` (Claude's session id, read from its `system:init` frame). The journal and its epoch carry on; a turn left
open by the previous run ends `unverifiable`. A chat session resumes as a chat; a chat resume refused by the version
probe is refused with `agent.chat_unsupported`.

D10 Version probing and fallback. The service runs the kind's program with `--version` when it lists kinds and caches
the result per program path and modification time. `agent.kinds` lists `chat: { supported, reason?, version?, range? }`
on every kind that declares chat support, with `reason` one of `not_detected`, `version_unknown`,
`version_outside_range`. Absent `chat` means no chat. The range is pinned in the Claude adapter from probes of real
releases; a version outside it is refused rather than tried, because a changed frame vocabulary would be misread.

D11 Capability gating. The capability is `agent-chat/1`. A service offers it once it serves every method above; a
client shows the Chat segment when it is offered and the selected kind says `chat.supported`. Every new field is
optional with a serde default, so a peer without the capability reads and writes exactly what it did before. This
change does not register the capability in the server: no service serves it yet.

D12 Out of the first slice: rewind, questions (`AskUserQuestion`), compaction, goals, background tasks, slash
commands, attachments, model catalogue, context usage, and every kind but Claude. Items of a type a client does not
know decode as `unknown` and are skipped, so these can be added later without a new capability version.

## What only a real Claude run can confirm

- The supported version range, and that `claude --version` prints a version this probe parses.
- That `-p --input-format stream-json --output-format stream-json --verbose --permission-prompt-tool stdio` keeps
  the process alive across turns, and that a user message written mid-turn is held for the next turn (D7).
- That `--include-partial-messages` (or its current name) is needed for text deltas, and how `content_block_delta`
  maps to `append`.
- That `can_use_tool` carries a request id, `tool_use_id` and permission suggestions in the shape D6 expects, and what
  writing the suggestions does for `always`.
- That `interrupt` ends the turn with a `result` frame and keeps the process, and what happens to a pending
  `can_use_tool` (a `control_cancel_request` or nothing).
- That `--resume <id>` in stream-json mode replays nothing on stdout, or what it does replay.
- How sub-agent frames name their parent (`parent_tool_use_id`) and whether plans arrive as `TodoWrite` tool calls.

## Consequences

- The desktop builds its Chat segment against these types now and turns it on when `agent-chat/1` is offered.
- The service slices follow: the Claude adapter and version probe, the journal store, the stream, approvals and
  interrupt, then history. `AgentSession.run` lands with the first of them.
- Error codes are defined in `chat.rs` with their methods and move to `codes.rs` when the service starts sending them.
