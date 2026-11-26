# 0168: Session metadata, long first prompts and fork

Status: accepted, 2026-11-25.

## Context

- AG-05: a first prompt reached a terminal agent as a launch argument, a flag, a startup query, or a paste at the
  first `idle` (kinds with `prompt.mode` `none`). A long or multi-line prompt always rode the command line. pv96
  falls back to a paste for a prompt that does not fit the typed command line.
- HIST-02: a session record carried its title, workspace, times and last result, but not the model and effort it
  was started with, its branch, or a short preview for a list.
- AG-13: a session could be resumed, never forked. pv96 forks the two agents whose conversations it can own:
  `claude --resume <id> --fork-session` and `codex fork <id>` (`src/shared/agent-resume-argv.ts`,
  `getAgentForkArgv`).

## Decision

### First prompts that do not fit a launch argument

- `ARGV_PROMPT_MAX_BYTES` is 4 KiB. A Windows command line holds at most 32,767 UTF-16 units in all, shared with the
  program path, the kind's and the caller's arguments, the settings path and the per-launch instructions (0161), and
  quoting can double a prompt; 4 KiB keeps a quoted prompt under a quarter of it.
- A first prompt in the `argv`, `flag` or `startup-query` mode is held and typed at the run's first `idle`, as a
  `none` prompt is (0054), when it is longer than the limit or holds a line break, the kind maps an event to `idle`,
  and, for a line break, the kind pastes bracketed (plain paste would submit each line on its own).
- Otherwise it stays on the command line. Nothing new is refused, except below: the run is spawned without a shell, so
  line breaks reach the agent intact, and the 8 KiB prompt limit (0054) keeps any prompt within the platform's limit.
- A `.cmd` or `.bat` program runs through `cmd.exe`, whose line holds 8,191 characters, and quoting can double a prompt.
  Its argument limit is therefore 2 KiB (`CMD_SHIM_ARGV_PROMPT_MAX_BYTES`): a longer prompt is typed at the first `idle`
  when the kind can take it so, and otherwise refused with `agent.prompt_invalid`.

### Session metadata

- The session record gains, all optional and additive: `model`, `effort`, `branch`, `preview`, `forked_from`.
- One appended migration step adds three nullable columns to `agent_sessions`: `model`, `effort`, `forked_from`.
  They are written with the row in one transaction. `forked_from` is plain text, not a foreign key, so forgetting
  the source keeps the fork.
- A resume passes the stored model and effort again through the kind's flags, when the kind still takes them: a model
  the kind's choices no longer list, or an effort its levels or that model no longer list, is left out. The run notes it
  (see the run's notes below), and the stored options are rewritten to what the run used, so the record never shows an option the run
  does not use. A fork applies the same rule to its copy; the source keeps its record.
- `branch` is the session's workspace's branch now; absent for a plain folder.
- `preview` is at most 200 characters (`MAX_PREVIEW_CHARS`) on one line: for a chat session the newest
  `user_message` or `assistant_message` with text in its journal, for a terminal session its `result`.
- Each view reads the origin, the branch and the newest message in one SQL statement; no file is read per row. The statement reads at most 400 chunks of the newest message. A view a caller sees carries these details and the run's notes; the exit sweep, which runs every 200 ms, reads the execution alone.
- The run's notes (the file `run-notes` in its run directory) say in plain words what a resume or a fork left out of the stored options, and that a first input was dropped when no `idle` came within 120 s. The next run of the session replaces them.
- A run holding vault secrets has its `preview` withheld from callers below host-admin scope, as its `result`
  (0071).

### Fork

- A kind declares its fork form as data: `"fork": [..., "{ref}", ...]`, with `{ref}` the source's agent ref. It
  needs `session_ref.learn` (checked by `problems`): the fork's own ref is never assigned, since passing an id
  together with a fork flag is not a combination pv96 uses. The run reports it, as a Codex first start does.
- Built-ins: `claude` declares `["--resume", "{ref}", "--fork-session"]`, `codex` `["fork", "{ref}"]`, both as
  pv96 builds them. `antigravity` and `gentle-shell` declare none: pv96 has no fork form for them.
- `agent.fork { session, prompt?, permission_mode?, title? }` (capability `agent-fork/1`) starts a new session in
  the source's workspace with a new id and `forked_from` set. It keeps the source's read-only mark and the model and
  effort its kind still takes (see resume), and takes `title` or the source's title. Its prompt follows the fork form like a first prompt, with the
  fallback above.
- The source must have exited: a live run's transcript may be mid-turn, and the fork would write in the same
  workspace, which the one-writer rule refuses anyway. A live source is `agent.busy`.
- `agent.fork_unavailable` when the kind declares no fork form, the source's ref is not known, or the source's
  resume state is `agent.resume_not_found` (0160). A chat session is `agent.chat_unsupported`.
- A source holding vault secrets makes the fork hold them, and forking it needs host-admin scope, as resuming it does.
- A ref the fork learns that equals its source's is dropped, so a fork never binds to the source's transcript: it keeps
  no ref, and resuming it is `agent.resume_unavailable`.
- CLI `massorca agent fork <SESSION> [--prompt TEXT] [--mode MODE] [--title TITLE]`; the client refuses the call
  before sending when the service does not offer `agent-fork/1`.

## Consequences

- AG-05: all four delivery forms work, and a long or multi-line prompt no longer depends on the command line where
  the kind can take it after launch.
- HIST-02: own sessions carry model, effort, branch and a preview. Titles from vendor transcripts (HIST-03,
  HIST-04) stay out of scope.
- AG-13: Claude and Codex sessions can be forked from the service and the CLI. A desktop action is not part of this
  decision.
- A kind package may add `fork`; an older service rejects the unknown field, as for any new kind field.
