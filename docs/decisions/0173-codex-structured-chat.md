# 0173: Codex structured chat

Status: accepted, 2026-11-26.

## Context

Decision 0101 made Claude Code a structured chat run: a vendor CLI over pipes whose output an adapter turns into the
session's transcript journal. Inventory row AG-18 asks the same of Codex. Codex has two machine interfaces:
`codex exec --json`, a one-shot run that prints events and exits, and `codex app-server`, a long-lived JSON-RPC server
over standard streams that keeps a thread, takes turns, streams items and asks for approvals as requests. pv96 uses
`app-server` for its Codex chat.

This first slice makes Codex a working chat: start, stream, tools, approvals, turn end, errors, usage, interrupt and
resume. Out of scope, and kept as follow-ups: the checklist format in the chat's instructions (AG-32), the design-system
writable root of a chat in a linked worktree (AG-36), questions (`item/tool/requestUserInput`), sub-agent threads,
images, rewind, compaction controls, goals, reviews, the model catalogue and rate-limit probes, managed-home
preparation, session index healing and rollout backfill, and workspace trust pre-writing (AG-12).

## Reference

pv96 at commit d3c6311a531, read-only:

- The launch: only root options before `app-server`, the permission flags dropped from the arguments and carried on the
  thread instead (`src/main/codex/codex-structured-launch-args.ts`, `codex-structured-permission-policy.ts`).
- The handshake: `initialize` with `clientInfo` and `capabilities.experimentalApi`, then `initialized`
  (`codex-app-server-handshake.ts`).
- The thread: `thread/start` with the model and the policy, `thread/resume` with `threadId`; a resume that lands on
  another thread is refused (`codex-structured-thread-open.ts`, `codex-structured-thread-facts.ts`).
- Items and their deltas (`codex-structured-item-translation.ts`, `codex-structured-item-streams.ts`), the turn
  endings of 0.141.0 (`__fixtures__/codex-app-server-turn-endings.jsonl`), the error frame's `willRetry`.
- Approval requests and their decisions `accept | acceptForSession | decline | cancel`; every request the client does
  not model gets a safe answer (`codex-structured-prompt-replies.ts`, `codex-server-request-disposition.ts`).
- `turn/interrupt` with `threadId` and `turnId` (`codex-structured-turn-cancellation.ts`).
- Token usage from `thread/tokenUsage/updated`'s `tokenUsage.total` (`codex-subagent-activity.ts`).

The app-server's public documentation (learn.chatgpt.com/docs/app-server) confirms: messages are JSON-RPC 2.0 without
the `jsonrpc` member; `initialize` then `initialized`; `thread/start` answers `{ thread: { id } }`; `thread/resume`
takes `threadId`; `turn/start` takes `input: [{ type: "text", text }]` and answers `{ turn }`; `turn/interrupt` takes
`threadId` and `turnId` and the turn ends `interrupted`; `turn/completed` carries `completed | interrupted | failed`;
`error` carries `willRetry`; `item/agentMessage/delta`, `item/commandExecution/outputDelta`,
`item/reasoning/summaryTextDelta` and `item/reasoning/textDelta` exist; the item shapes of `commandExecution`,
`fileChange` (`changes: [{ path, kind, diff }]`) and `mcpToolCall`; the approval requests' params and decisions; and
`serverRequest/resolved { threadId, requestId }`. `codex app-server --help` of 0.172.0 (run read-only) lists `-c`,
`--enable`, `--disable` and the default `stdio://` transport.

## Decision

D1 Transport: `codex app-server` over standard streams. It keeps one process per chat, as the Claude adapter does, so
interrupt, approvals, held prompts and resume map onto the service's existing chat run without a new process model.
`exec --json` would need a process per turn and has no approval requests.

D2 Launch. The kind's launch plan is built as for a terminal run and translated: `app-server`, then the plan's
`-c key=value`, `--enable` and `--disable`. `-m` becomes the thread's `model`; `--sandbox`, `--ask-for-approval`,
`--approve-for-me` (`workspace-write`, `on-request`, `approvalsReviewer: auto_review`) and
`--dangerously-bypass-approvals-and-sandbox` (`danger-full-access`, `never`) become its `sandbox`, `approvalPolicy` and
`approvalsReviewer`, so the permission mode is always said on the thread and never inherited from the account's
`config.toml`: a start without a mode (the CLI's `agent start` passes none unless `--mode` names one) gets the kind's
default, `workspace-write` and `on-request`. A `-c` of those keys, and the terminal's `notify` hook (a chat reports through its output), are dropped.
`resume <ref>` becomes `thread/resume` of that thread. Everything else is dropped. The effort stays a config override
(`-c model_reasoning_effort=...`), as the kind declares it. The account's credential home (`CODEX_HOME`) is set by the
launch plan, as for a terminal run.

D3 Handshake and thread. The adapter trait gains `opening()`: bytes written before any prompt. Codex's is `initialize`;
its answer sends `initialized` and the thread request; the answer to that names the thread. Prompts sent meanwhile are
held by the adapter (no bytes) and written once the thread is open. A thread that cannot be opened, or a resume that
lands on another thread, is an error notice, and the turns waiting for it end `completed` with outcome `failure`.

D4 The session's ref. `JournalOp` gains `LearnRef`: the opened thread's id becomes the session's `agent_ref` when it
has none, through the rule a hook report's learned ref follows (a fork never keeps its source's). The store gains
`set_agent_session_ref_if_unset`. A resume starts `thread/resume` of that ref.

D5 Mapping.

| Message | Journal |
|---|---|
| `item/started` / `item/completed` `agentMessage` | `assistant_message`, `running` then `completed` with the item's whole text |
| `item/agentMessage/delta` | `append` (adds the message when the delta comes first) |
| `item/reasoning/summaryTextDelta`, `reasoning` with a `summary` | `thinking`; raw `textDelta` is not shown |
| `commandExecution` | `tool_use` `shell`, the command as input; `outputDelta` makes it `updated` with the output so far; completed with a non-zero `exitCode`, `failed` or `declined` is `failed` |
| `fileChange` | `tool_use` `apply_patch`, the paths as input; once completed, one `diff` per change (`add` is `added`, `delete` `deleted`, a `move_path` `renamed`) |
| `mcpToolCall`, `webSearch` | `tool_use` `server/tool` with its arguments and result, `web_search` with its query |
| `turn/plan/updated` | the turn's `plan` |
| `contextCompaction` completed | an info notice |
| `userMessage`, `plan` items | nothing (the service shows the prompt; the plan comes from `turn/plan/updated`) |
| `turn/completed` | `completed` success, `failed` failure (with the reason unless an `error` showed it), `interrupted` |
| `error` | a warning while `willRetry`, else an error notice |
| `thread/tokenUsage/updated` | the open turn's `usage` |
| `item/commandExecution/requestApproval`, `item/fileChange/requestApproval` | a pending `approval` keyed by the request's JSON-RPC id |
| `serverRequest/resolved` of a pending request | the approval `cancelled`; nothing answers it |
| other requests | refused or answered granting nothing, with a notice naming the method |
| messages of another thread (sub-agents) or of an interrupted turn | nothing |
| known lifecycle and account notifications, `codex/event/*` | nothing |
| an unknown notification or item type | one warning notice per name per run, never the content (as 0101 says) |

D6 Approvals. Approvals default to asking: the kind's default mode is `on-request`. An allow answers `accept`; an
`always` allow answers `acceptForSession`, which the item lists as its one suggestion
(`{ type: "acceptForSession", rules: [<the command or paths>], destination: "session" }`, so `can_always`); a deny
answers `decline` (Codex takes no message, so the deny's message is not sent). An approval request without an item id is
declined with a notice. Cancel never authorizes: an interrupt answers every pending request `cancel` before it
interrupts the turn, and a withdrawn request is never answered.

D7 Interrupt and held prompts. The interrupt is `turn/interrupt` of the turn the app-server named (from the
`turn/start` answer or `turn/started`); one asked before the turn is named goes out when it is. The service already
ended the turn, so what the interrupted turn still sends, and its `turn/completed`, are skipped; prompts sent until it
ends wait for it. A forced prompt mid-turn is a `turn/start`, which the app-server steers into the running turn (pv96:
since 0.148), so the held turn ends with the running one as 0101's D7 says.

D8 Usage on the wire. `Turn` gains `usage?: TokenUsage`, the conversation's running totals as the CLI last reported
them during the turn:

```json
{ "type": "turn", "state": "completed", "outcome": "success",
  "usage": { "input_tokens": 1200, "cached_input_tokens": 800, "output_tokens": 40,
             "reasoning_output_tokens": 10, "total_tokens": 1240 } }
```

Every count has a serde default; `usage` is absent when the CLI reports none (Claude's dialect here) and from an older
peer. `JournalOp` gains `Usage`, which sets it on the open turn's item.

D9 Declaration as data. The built-in `codex` kind declares
`"chat": { "adapter": "codex-app-server", "min_version": "0.172.0", "max_major": 0 }` (range `>=0.172.0, <1.0.0`):
0.172.0 is the owner's version, whose `app-server --help` was read. `agent.kinds` lists its `chat` support as for
Claude, from the probed `codex --version` (`codex-cli 0.172.0`).

D10 Tests. The fake agent gains an `app-server` mode that answers the handshake, thread and turn requests and plays a
script of JSON-RPC messages; CLI tests cover the stream, a command, allow, deny, an interrupt that cancels a pending
request, resume of the learned thread, and an unknown message.

## What only a real Codex run can confirm

- That 0.172.0 accepts `initialize` with `capabilities.experimentalApi: true` and the client info as sent, and answers
  `thread/start` and `thread/resume` with `thread.id`.
- That `thread/start` and `thread/resume` take `model`, `sandbox` (`workspace-write`, `read-only`,
  `danger-full-access`), `approvalPolicy` (`on-request`, `never`) and `approvalsReviewer: auto_review` as pv96 sends
  them, and that a thread opened without `cwd` runs in the process's working folder (the workspace).
- That `-c model_reasoning_effort=<level>` on `app-server` sets the turn's effort, and that naming the model on
  `thread/resume` keeps the resumed conversation.
- That a `turn/start` sent while a turn runs is steered into it rather than queued as a turn of its own.
- That the delta params are `{ itemId, delta }`, that `reasoning` items carry `summary` as a list of strings, that a
  `fileChange` change's `kind` is `{ type, move_path? }` or a bare string, and that `item/completed` of an
  `agentMessage` carries the whole text.
- That `thread/tokenUsage/updated` carries `tokenUsage.total` with `inputTokens`, `cachedInputTokens`, `outputTokens`,
  `reasoningOutputTokens` and `totalTokens`, and arrives before `turn/completed`.
- That an approval request's id is answered with `{ decision }` as the documentation lists it, that `acceptForSession`
  allows the same command for the rest of the thread, that `cancel` answers to pending requests before `turn/interrupt`
  are accepted, and that `serverRequest/resolved` is sent for an approval the app-server withdraws.
- That 0.172.0 sends no `codex/event/*` notifications that matter to the journal, and which other notifications it
  sends that the adapter should name (each unknown one is one warning notice per run).
- That the process ends when its standard input closes, so the service's graceful stop is enough.

## Consequences

- The desktop gets Codex chats through `agent-chat/1` with no change; it may show `Turn.usage` when present.
- The `ChatAdapter` trait now takes `launch_args(&mut self)` and has `opening()`; `JournalOp` has `Usage` and
  `LearnRef`. The Claude and test adapters are unchanged in behaviour.
- Follow-ups: AG-32 checklist format in the chat's instructions; AG-36 the design-system writable root; questions;
  sub-agent threads as `subagent` items; images; rewind, compaction, goals and reviews; model and rate-limit probes over
  a read-only app-server; managed-home preparation and session index healing; an old CLI's missing `app-server`
  recognised from its error.
