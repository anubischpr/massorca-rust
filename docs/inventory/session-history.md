# Functional inventory: agent session history

Domain prefix: `HIST`. Reference: MassOrca Electron app (read-only snapshot at `D:\projects\massorca\.claude\worktrees\massorca-server-language-820c84`).

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits). Lines marked `Source: pv96` say what changed versus `custom/integration`. The scope rename, the automated-run switch and delegated-child nesting were already in `custom/integration`; the pv96 changes are the fixes, the Antigravity attribution, and the bot-team switch and marks listed below.

## Domain summary

MassOrca lists the conversations that coding agent CLIs (Claude Code, Codex, OpenCode, Gemini and about twenty more) have left on disk, under the name "Agent Session History". It never owns those files: it discovers them by walking each agent's own store, reads them read-only, and can re-enter a conversation by launching the agent CLI with that agent's own resume flag. Discovery covers the local machine, running WSL distros on Windows, and SSH or runtime hosts through a relay. Two optional layers sit on top: a full-text index of transcripts (off by default, with consent and retention settings) and a transcript viewer for eight agents. The only write to an agent's store is deletion, which the product allows for 14 agents. Adoption into a structured chat is the one path that copies a transcript into MassOrca's own journal, and it exists for Claude, Codex and Antigravity. There is no general "import from other agents" path today; that requirement is new.

## Source directories read

Repo-relative to the reference repository.

- `src/main/ai-vault/`: agent source table (`session-scanner-agent-sources.ts`), roots, discovery, path helpers for Antigravity, Kimi, Muse, OpenCode, ZCode, Cursor, Devin, Codex; the every-agent fixture (`session-scanner-every-agent-fixture.ts`, `session-scanner-log-agent-fixtures.ts`, `session-scanner-document-agent-fixtures.ts`), session delete, cached session list, WSL home discovery, scan options and parse cache store (headers and selected bodies).
- `src/main/ai-vault-search/`: database path, enablement, policy, retention, schema, typo repair, hit ranking, query planner (headers and selected bodies).
- `src/main/foreign-sqlite-readers/`: dispatch, Cursor profile reader, OpenCode binder reader.
- `src/main/native-chat/`: transcript reader, incremental and tail readers, watch headers, structured adoption (`structured-agent-session-history-adoption.ts`), session file resolver headers.
- `src/main/ipc/`: `ai-vault.ts`, `ai-vault-search.ts`, `ai-vault-resume.ts`, `ai-vault-delete.ts`, `ai-vault-subagent-list.ts`, `native-chat.ts`, `local-log-tail.ts` (names only), `memory.ts`.
- `src/main/memory/` (process RAM dashboard, not AI memory), `src/main/projectless-chats/`, `src/main/claude-accounts/` (history sharing headers), `src/main/codex/codex-ai-vault-session-resume.ts` (header), `src/main/opencode/opencode-data-directory.ts`, `src/main/startup/main-process-preflight.ts` (cache path only).
- `src/relay/ai-vault-*.ts`: handler, service client, protocol, transcript stream, opencode reader (headers and registration).
- `src/shared/`: `ai-vault-types.ts`, `ai-vault-resume-command.ts`, `ai-vault-resume-preparation.ts`, `ai-vault-session-deletion.ts`, `ai-vault-search-settings.ts`, `ai-vault-search-agent-compatibility.ts`, `ai-vault-session-depth.ts`, `ai-vault-session-title.ts`, `ai-vault-automated-run.ts` (header), `ai-vault-search-query-operators.ts` (header), `agent-session-provider-handle.ts`, `native-chat-agent-support.ts`, `ssh-ai-vault-relay.ts`, `tui-agent-config.ts` (launch and detect commands), `grok-session-paths.ts`, `muse-session-log.ts`, `opencode-database-name.ts`, `projectless-chat-folder.ts`.
- `docs/PLAN.md` in the new repository (scope context).
- pv96 changed files (worktree `unified-custom-profile-build-547ea6`): `src/main/ai-vault/` (`session-scanner.ts`, `session-scanner-parse-cache.ts`, `session-scanner-antigravity-conversation-db.ts`, `session-scanner-antigravity-history.ts`, `internal-agent-run-registry.ts`, `internal-agent-run-projection.ts`, `remote-session-scanner.ts`, `delegated-child-sessions.ts`), `src/shared/ai-vault-automated-run.ts`, `src/relay/ai-vault-service-filesystem.ts` (diff), `src/renderer/src/components/right-sidebar/ai-vault-*` and `mobile/src/agent-history/` (names and the bot-team marks only).
- `odd/tasks/session-history-scope.md` (read in full at `d3c6311a531`).
- Commit messages for the pv96 history commits: `c7d082fb308`, `4e60c4f45b6`, `659ff0f97c7`, `17cadabf42e`, `12d58a468c8`, `4c87a8681a7`, `cb248697fe7`, `51b9d336aef`, `3bf3d76a501`, `49553fc89e0`.

Not all files were read line by line; the reasoning relies on names, headers and the fixture layouts where noted below.

## Not read / low confidence

Not read:
- Renderer and mobile UI (`src/renderer`, `mobile/`), per the brief. The session list and viewer screens are described only as far as their IPC calls go.
- Per-agent parser bodies (`session-scanner-*-parser.ts`, `session-scanner-codex-*`, `session-scanner-opencode-sqlite-*`, `session-scanner-devin-*`, Kimi/Muse/Jcode record handling). Record-level formats are not verified beyond what fixtures show.
- SQLite schemas for OpenCode, OpenCode 2, ZCode and Devin `sessions.db`.
- Search engine internals: indexer passes, child service IPC, store, file cursor, retention deletion, snippet logic.
- Transcript watch engine, tail reader and watcher scheduler internals.
- Relay service protocol, restart policy and spawn path; the remote scanner's source modules.
- Claude account history linking code (`claude-profile-history.ts`, `claude-profile-sharing.ts`), beyond the header and the list of shared directories.
- Codex managed-home layout under MassOrca's own data directory.
- Test files (skipped by rule), except where a test name was the only clue.
- pv96: the spawn-record change for headless runs (`4e60c4f45b6`, 17 files) and the Antigravity conversation-database reader were read from commit messages and file names, not line by line.
- pv96: the mobile agent-history screens and the desktop AI Vault rows were read only for the bot-team marks and the automated-run marks; their layout was not read.

Low confidence (flagged again where they matter):
- Vendor CLI resume flags: MassOrca builds `<cli> --resume <id>` style commands. Whether each vendor CLI accepts that exact flag was not checked against the vendor CLI.
- Gemini: the scanner walks `~/.gemini/tmp` recursively, but the real project subfolder layout was not verified.
- Copilot: the fixture writes a flat `<id>.jsonl` under `session-state`; the real CLI layout may be nested. Unverified.
- OpenCode on Windows: code resolves to `%USERPROFILE%\.local\share\opencode` (no `%APPDATA%` branch) unless `XDG_DATA_HOME` is set. Whether real OpenCode on Windows writes there was not checked.
- Muse on Windows: code defaults to `~/.local/share/muse/sessions` on every OS. The upstream Windows default was not checked.
- Antigravity IDE sessions: the name `isAntigravityReferenceSession` suggests they are opened as a reference prompt, not as the same conversation. Inferred from the name and the `--prompt-interactive` branch, not from a run.
- Devin `sessions.db`: the code places it at the `cli` directory beside `transcripts`, so `%APPDATA%\devin\cli\sessions.db`. Read from path arithmetic, not from a real install.
- Structured adoption list: `AGENT_SESSION_ADOPTABLE_PROVIDERS` contains `claude`, `codex` and `antigravity`, but the comment in the adoption module says "only Claude and Codex adopt". The array is the more likely truth; the comment may be stale.
- How SSH-host resume runs: the desktop returns `useRealCodexHome: false` for SSH hosts. The rest of the path (the command typed into a remote shell) was inferred, not traced.

## Session history on disk per agent

Notes for reading the table:
- `%USERPROFILE%` is the Windows home (Node `os.homedir()`). `~` is the home on macOS and Linux.
- "Read" describes how the scanner finds files. The scanner also reads the same layouts under each running WSL distro's home on Windows (`\\wsl$\<distro>\home\<user>\...`, Windows only, running distros only).
- Environment overrides, where the code shows them: `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `COPILOT_HOME`, `GROK_HOME`, `KIMI_CODE_HOME`, `JCODE_HOME`, `DEVIN_HOME`, `CLINE_SESSION_DATA_DIR`, `PI_CODING_AGENT_DIR`, `OPENCLAW_STATE_DIR`, `XDG_DATA_HOME`, `OPENCODE_DB`. Oh My Pi and Prime Agent have their own overrides (see their roots helpers). There is no in-app setting for these roots in the IPC layer that was read.
- The "Resume" column is the command MassOrca builds. It is the vendor CLI doing the resume, against the file where it already lives. See the low-confidence list.

| Agent | Format on disk | Windows | macOS / Linux | How MassOrca reads it | Can MassOrca resume a session found there |
|---|---|---|---|---|---|
| Claude Code | JSONL, one file per session | `%USERPROFILE%\.claude\projects\<cwd>\<sessionId>.jsonl`. `<cwd>` is the working directory with every non-alphanumeric character replaced by `-` | `~/.claude/projects/<cwd>/<sessionId>.jsonl` | Scanner walks the root; `subagents/` directories are skipped (read on demand under the parent). Native chat reader parses records. Managed accounts add their own `projects` dirs | Yes: `claude --resume <sessionId>` in the session's cwd. Also adoptable into a structured chat (HIST-08) |
| CodeBuddy | JSONL | `%USERPROFILE%\.codebuddy\projects\...` | `~/.codebuddy/projects/...` | Scanner walks the root; subagent dirs skipped; message roles and `input_text`/`output_text` blocks parsed | Yes: `codebuddy --resume <id>` (constructed) |
| Qoder | JSONL | `%USERPROFILE%\.qoder\projects\...` | `~/.qoder/projects/...` | Scanner walks the root; subagent dirs skipped | Yes: `qodercli --resume <id>` (constructed; the binary name is `qodercli`) |
| Codex | JSONL rollouts: `sessions\YYYY\MM\DD\rollout-<timestamp>-<uuid>.jsonl`; thread names in `session_index.jsonl` at the Codex home | `%USERPROFILE%\.codex\sessions\...` and `%USERPROFILE%\.codex\session_index.jsonl` | `~/.codex/sessions/...` | Scanner walks the root; hardlinked aliases across roots are deduplicated; titles come from `session_index.jsonl`. Forks hold only post-fork records and inherit the parent prefix | Yes: `codex resume <id>`. A `CODEX_HOME` prefix is added for MassOrca-managed account homes |
| Gemini CLI | JSON (legacy single document with a `messages` array) and JSONL logs | `%USERPROFILE%\.gemini\tmp\...` (recursive) | `~/.gemini/tmp/...` | Parsed per file type. Only the JSONL log format is marked resumable; legacy JSON is rewritten in place and read whole | Yes for JSONL: `gemini --resume <id>` (constructed). Legacy JSON: not resumable per the parser comment |
| GitHub Copilot CLI | JSONL (`<id>.jsonl` in the fixture) | `%USERPROFILE%\.copilot\session-state\` (`COPILOT_HOME`) | `~/.copilot/session-state/` | Scanner walks the root for `.jsonl`. Real nesting unverified (low confidence) | Yes: `copilot --resume=<id>` (constructed) |
| Cursor (agent CLI) | JSONL transcripts under `agent-transcripts\`; JSON metadata (`meta.json`) under `chats\<md5 of cwd>\<uuid>\` | `%USERPROFILE%\.cursor\projects\<slug>\agent-transcripts\**\*.jsonl` and `%USERPROFILE%\.cursor\chats\...\meta.json` | `~/.cursor/projects/...` and `~/.cursor/chats/...` | Transcripts are listed; cwd, title and times come from `meta.json`, joined by chat id through an index of the chats directories (the md5 hashes the cwd, so the join needs the index). The Cursor IDE's `state.vscdb` is separate (see HIST-13) | Yes: `cursor-agent --resume <id>` (constructed) |
| Grok | JSON `summary.json` per session; JSONL `chat_history.jsonl` beside it | `%USERPROFILE%\.grok\sessions\<encodeURIComponent(cwd)>\<id>\summary.json` (`GROK_HOME`). Long cwd names use a slug-plus-hash directory | `~/.grok/sessions/...` | `summary.json` is the discovered file; native chat reads `chat_history.jsonl`. Native chat requires a local transcript, so it is not available over SSH | Yes: `grok --resume <id>` (constructed) |
| Devin | ATIF JSON per session in `transcripts\`; SQLite `sessions.db` beside the transcripts' parent; `agent_logs\` as an extra root | `%APPDATA%\devin\cli\transcripts\*.json` and `%APPDATA%\devin\cli\sessions.db` (`DEVIN_HOME`) | `$XDG_DATA_HOME/devin/cli/transcripts` or `~/.local/share/devin/cli/transcripts` | Transcript JSON is discovered; `sessions.db` is read read-only for index fields, and a change to it re-merges sessions without re-reading transcripts. Path of `sessions.db` is derived (low confidence) | Yes: `devin --resume <id>` (constructed) |
| Hermes | JSON, one file per session named `session_*.json` | `%USERPROFILE%\.hermes\sessions\` | `~/.hermes/sessions/` | Files filtered by the `session_` prefix | Yes: `hermes --resume <id>` (constructed; base is `hermes`) |
| Jcode | JSON, `session_*.json`; `.journal.jsonl` appends are ignored | `%USERPROFILE%\.jcode\sessions\` (`JCODE_HOME`) | `~/.jcode/sessions/` | Files filtered by prefix; internal display roles are not counted as messages | Yes: `jcode --resume <id>` (constructed) |
| Rovo Dev | JSON: `metadata.json` plus `session_context.json` per `<id>\` directory | `%USERPROFILE%\.rovodev\sessions\<id>\` | `~/.rovodev/sessions/<id>/` | `metadata.json` is the discovered file; `session_context.json` gives the opening request | Yes: `acli rovodev run --restore <id>` (constructed; base is the full launch command) |
| Pi | JSONL message graph (session, model_change and message records) | `%USERPROFILE%\.pi\agent\sessions\` (`PI_CODING_AGENT_DIR`). Gentle Shell: `%USERPROFILE%\.gentle-shell\agent\sessions\` | `~/.pi/agent/sessions/`; `~/.gentle-shell/agent/sessions/` | Graph JSONL parser; title read for Pi (title agents list includes Pi) | Yes: `pi --session <id>`. Gentle Shell sessions resume through `gentle-shell -- --session <file>` with the file path |
| Oh My Pi (OMP) | JSONL message graph; Task subagent transcripts inside `<stamp>_<uuid>\` artifact directories | `%USERPROFILE%\.omp\agent\sessions\` (overridable; see OMP roots helper) | `~/.omp/agent/sessions/` | Graph parser; artifact directories pruned from top-level listing. Subagents listed on demand | Yes: `omp --resume <absolute transcript path>` (resumes by file path, not id) |
| Prime Agent | JSONL message graph (Pi fork) | `%USERPROFILE%\.prime\agent\sessions\` (own override variable) | `~/.prime/agent/sessions/` | Same graph parser as Pi and OMP | Yes: `prime-agent --resume <absolute transcript path>` |
| OpenClaw | JSONL under `agents\<agent>\sessions\` | `%USERPROFILE%\.openclaw\agents\...` (legacy `%USERPROFILE%\.clawdbot`); `OPENCLAW_STATE_DIR` | `~/.openclaw/agents/...` (legacy `~/.clawdbot`) | Files must sit under a `sessions` path segment; current and legacy state dirs merge | Yes: `openclaw --resume <id>` (constructed) |
| Droid (Factory) | JSONL | `%USERPROFILE%\.factory\sessions\` and `%USERPROFILE%\.factory\projects\` | `~/.factory/sessions/` and `~/.factory/projects/` | Scanner walks both roots | Yes: `droid --resume <id>` (constructed) |
| Cline | JSON manifest per session (`<id>.json`) plus `<id>.messages.json` | `%USERPROFILE%\.cline\data\sessions\<id>\` (`CLINE_SESSION_DATA_DIR`) | `~/.cline/data/sessions/<id>/` | Only the manifest directly under each session directory is a session; the messages file is a content dependency | Yes: `cline --id <id>` (constructed) |
| Kimi Code | JSON `state.json` per session plus JSONL `agents\main\wire.jsonl`; `session_index.jsonl` at the Kimi home maps the work directory | `%USERPROFILE%\.kimi-code\sessions\wd_<name>_<hash>\session_<uuid>\` (`KIMI_CODE_HOME`) | `~/.kimi-code/sessions/...` | Only `state.json` under `session_*` directories is a session. The index supplies the real work directory | Yes: `kimi --session <session_id>`, run from the work directory (the CLI rejects other directories) |
| Muse | JSONL, one `session.jsonl` per session | `%USERPROFILE%\.local\share\muse\sessions\YYYY\MM\DD\<uuid>\` (per the code default; low confidence on Windows) | `~/.local/share/muse/sessions/YYYY/MM/DD/<uuid>/` (`XDG_DATA_HOME`) | Only `session.jsonl` counts; sibling `.log`, `.sqlite3` and `.msp-view` files are ignored | Yes: `muse resume <uuid>` in the session's cwd |
| Antigravity | JSONL `transcript.jsonl` at `brain\<conversationId>\.system_generated\logs\`; `history.jsonl` beside `brain` | `%USERPROFILE%\.gemini\antigravity-cli\brain\...` (CLI). IDE origins, opt-in: `%USERPROFILE%\.gemini\antigravity-ide\brain\...` and `%USERPROFILE%\.gemini\antigravity\brain\...` | `~/.gemini/antigravity-cli/brain/...` (and IDE origins) | Transcripts are paired with the `history.jsonl` beside `brain` (pairing rule not read); the opening prompt is the join key for IDE history. Title read for Antigravity | CLI origin: `agy --conversation <id>`. IDE origin: not the same conversation; opened with a reference prompt (inferred). Adoptable into a structured chat |
| OpenCode | SQLite `opencode.db`; legacy JSON under `storage\` (`session\<project>\ses_*.json`, `message\<session>\msg_*.json`) | `%USERPROFILE%\.local\share\opencode\opencode.db` (no `%APPDATA%` branch in code) unless `XDG_DATA_HOME` is set. `OPENCODE_DB` overrides the file | `~/.local/share/opencode/opencode.db` (`XDG_DATA_HOME`) | Read-only SQLite on a worker process; legacy JSON read from `storage\`. A synthetic `<db>#<sessionId>` path identifies each SQLite session | Yes: `opencode --session <id>` (constructed) |
| OpenCode 2 | Same `opencode.db`, v2 tables (`session_v2`, `session_message`). Beta files `opencode-next.db` and `opencode-local.db` are excluded | As OpenCode | As OpenCode | Worker checks for v2 tables in the shared file; schema called unstable in the code | Yes: `opencode2 --standalone --session <id>` (constructed; base is the launch command) |
| ZCode | SQLite `db.sqlite` (OpenCode-shaped schema, per code comment) | `%USERPROFILE%\.zcode\cli\db\db.sqlite` | `~/.zcode/cli/db/db.sqlite` | Read-only SQLite on a worker; kept separate from OpenCode so one cannot absorb the other's rows | Yes: `zcode --resume <id>` (constructed) |

Summary for the import requirement:
- Nothing converts a foreign transcript into MassOrca's own store, except structured adoption (HIST-08), which imports a Claude, Codex or Antigravity transcript into a MassOrca journal.
- Terminal resume re-launches the vendor CLI against its own file. It works only where that CLI is installed on the host that holds the file, and it does not move the conversation.
- Every reader opens foreign stores read-only. SQLite files are opened read-only, and the one write (deletion, HIST-09) is limited to 14 agents.
- Discovery covers 24 agent identifiers. The search compatibility list frozen to an older parser covers 20 of them; Codebuddy, Qoder, ZCode and Jcode are absent from it, so search sends those four only to hosts that negotiate the newer list.

## Features

### HIST-01 Browse past sessions

- Source: pv96 (unmerged). The desktop AI Vault list and the phone's agent history show a bot team's character after the session name; the workspace "Show bot teams" switch (off by default, `49553fc89e0`) hides team folders, child worktrees and team rows in the workspace lists, while history rows show the character instead. A replayed scan with a new delegated-child link now refreshes the list (`12d58a468c8`, `659ff0f97c7`).
- What the user does: opens the session history panel and sees past conversations from every discovered agent, newest first by default. Filters by workspace, by project or across all, groups by project, folder or agent, sorts by last update or creation, and can hide automated runs (HIST-16).
- Rules and edge cases:
  - The default scan keeps the newest 1000 sessions. An "unlimited" option removes the cap.
  - Sessions in the user's active scope (up to 64 scope paths) are kept even when older than the cap, so a scoped view still shows its own sessions.
  - Results are cached for 60 seconds; a forced refresh bypasses the cache. A new scan cancels the previous one by request token. Concurrent callers share one scan.
  - Deleted sessions are dropped from the cached result at once.
  - Scan problems (unreadable root, unknown shape, permission) are reported as issues per agent and path, with human wording. The list still shows what could be read.
  - Local and SSH hosts are merged into one list when the user picks "all hosts". Remote scope is refused for hosts it cannot route to.
- Client asks / core pushes: `aiVault:listSessions` (renderer), `aiVault:cancelListSessions`, `aiVault:resolveSessionTitles`, `aiVault:windowFocused` (push, triggers refresh). Relay methods `aiVault.listSessions` and `aiVault.resolveSessionTitles` on remote hosts.
- Persists: parse cache at `<userData>/ai-vault/session-parse-cache.json`, keyed by file identity. Nothing else; the scanner does not write to agent stores.
- Depends on: HIST-02 (row content), HIST-03 and HIST-04 (sources), HIST-10 (remote hosts), HIST-16 (automated-run filter). Workspace and project model (other domain).

### HIST-02 Session titles, previews and metadata

- What the user does: sees each session's title, working directory, branch, model, last activity, message count, token total and a short preview of the latest messages.
- Rules and edge cases:
  - The list keeps at most five preview messages. When older messages were cut, the first preview turn is not the opening request, and the UI must not treat it as one.
  - Titles are read from the transcript for Claude, Codex, Pi and Antigravity only (the title agent list). Other agents take their title from their own metadata or first prompt, which was not verified per agent.
  - The full first user prompt is not in the list payload; it is fetched on demand (for copy, for example).
  - Headless runs keep their opening prompt as the only marker for Claude, Gemini and Antigravity transcripts.
  - Codex sessions can get their thread name after the rollout is written; the title cache avoids pinning the first, fallback title.
- Client asks / core pushes: list results carry the metadata; `aiVault:getFirstUserPrompt` on demand.
- Persists: nothing beyond the parse cache (HIST-01).
- Depends on: HIST-03 and HIST-04.

### HIST-03 Local session discovery for Claude Code and Codex

- What the user does: sees Claude Code and Codex conversations from this machine without any setup, next to the other rows of the list.
- Rules and edge cases:
  - Claude: `~/.claude/projects` (or `CLAUDE_CONFIG_DIR` for the default account). Subagent directories are not separate sessions. Managed Claude accounts add their own `projects` folders; on Windows they are read directly, on macOS and Linux their history is linked into the default folder (see HIST-12).
  - Codex: `sessions/YYYY/MM/DD`. Rollouts can appear under several roots (managed runtime home and the user's own home) as hardlinks; these collapse to one row. Forked rollouts hold only what came after the fork and read as the parent's prefix plus their own part.
  - Both agents have a native transcript reader (HIST-06).
- Client asks / core pushes: same list calls as HIST-01.
- Persists: parse cache only.
- Depends on: HIST-01. The Claude and Codex account homes are owned by the accounts domain.

### HIST-04 Local session discovery for the other 22 agents

- Source: pv96 (unmerged). Antigravity workspace attribution: the CLI no longer writes `history.jsonl`, so each conversation's worktree is read from its own `conversation_summaries.db` (once per scan); the cache files fill only conversations the database lacks; Orca's own added folders and the shared design-system folder are skipped (`51b9d336aef`, `3bf3d76a501`).
- What the user does: sees conversations from the other agent CLIs in the same list, provided those CLIs are installed and have written history on this machine.
- Rules and edge cases:
  - Each agent is a separate store reader with its own file predicate (for example Kimi only counts `state.json` under `session_*`; Muse only counts `session.jsonl`; Cline only counts depth-0 manifests). Formats are in the "Session history on disk per agent" table.
  - Four agents need a SQLite reader (OpenCode, OpenCode 2, ZCode, Devin's index). Those readers run in a worker and open the files read-only.
  - Antigravity IDE history is opt-in per list call (`includeAntigravityIdeSessions`); the CLI origin is always on.
  - OpenCode also has a legacy JSON layout. Beta OpenCode 2 database names are excluded from discovery.
- Client asks / core pushes: same list calls.
- Persists: parse cache only.
- Depends on: HIST-01. For the first version, each agent is an S to M adapter; a per-agent decision is the owner's.

### HIST-05 Full-text search of session history

- Source: pv96 (unmerged). Search hits take the list's automated-run classification, so a background run that was later titled no longer shows as a user chat in search results (`cb248697fe7`).
- What the user does: types words and gets matching sessions across the indexed history, ranked, with snippets, optionally narrowed to the current workspace or project.
- Rules and edge cases:
  - Off by default. Building the index reads every transcript on the machine, so nothing opens the index before the user turns it on in Settings. Enabling is a local Settings action; SSH hosts refuse enabling ("unsupported"). A runtime (environment) host answers for its own index.
  - Retention: all history, or only transcripts modified in the last 1 to 3650 days.
  - The index is a cache over the transcripts, one SQLite file per host at `<userData>/ai-vault/session-search.sqlite`. Schema version 6; a version bump drops and rebuilds it.
  - The index stores transcript content without redaction. The product treats a copy in the index as the same exposure as the transcript itself; what a snippet may carry off the machine is a transport decision.
  - Tokens follow the unicode61 tokenizer with `_ . - / +` kept inside tokens, so paths, identifiers and `C++` match whole. An identifiers column holds the split form of compound names.
  - Query operators `repo:` and `path:` apply only at the start of a token, so `https://` links stay literal.
  - Misspelled terms are repaired from the index's own vocabulary; a suggestion is never a word the index lacks.
  - Long queries are cut to the planner's term limit and the user is told that the tail was dropped.
  - Ranking: a small length prior (more messages rank slightly higher); forks of one conversation (same chained digest of the first messages) fold into one result; ties go to the newer session. The whole ranked list is kept so pages do not reshuffle; snippets are computed only for the page returned.
  - Index covers every agent in the default agent list, including OpenCode and Antigravity.
  - Older hosts that do not know newer agents are sent only the older agent list (frozen compatibility list).
- Client asks / core pushes: `aiVault:searchSessions`, `aiVault:searchStatus`, `aiVault:setSearchEnabled`, `aiVault:clearSearchIndex`. Relay methods `aiVault.searchSessions`, `aiVault.searchStatus`, `aiVault.setSearchEnabled`.
- Persists: `session-search.sqlite` under the user data directory (per host). No other state.
- Depends on: HIST-03 and HIST-04 (sources); the settings domain for consent storage.

### HIST-06 Transcript viewer

- What the user does: opens a past (or live) conversation and reads its messages, with the most recent window first and older pages on request; new messages appear as the agent writes them.
- Rules and edge cases:
  - Structured reader exists for eight agents: Claude, OpenClaude, Codex, Grok, OMP, OpenCode, OpenCode 2 and Antigravity. Other agents get only the five-message preview in the list.
  - Desktop window is 300 messages per page; a page can be raised by the caller.
  - Live updates use a file watcher, with reconciliation polling. A brand-new Claude session can take seconds to minutes to flush its first line, so the watcher keeps polling for it instead of giving up.
  - Per-record limit of 2 MiB (what happens above it was not read).
  - The read cache is keyed by the resolved transcript path, so all clients viewing one session share one parse.
  - Over SSH, Grok, OMP, OpenCode and Antigravity need a transcript on the owning host; their viewer is not offered for remote-only reads.
- Client asks / core pushes: `nativeChat:readSession` (page), `nativeChat:subscribe` and `nativeChat:unsubscribe` (live). Renderer document owns its subscriptions; crash or reload releases them.
- Persists: nothing; cache is in memory.
- Depends on: HIST-03, HIST-04 (file location); the structured chat domain for the message model.

### HIST-07 Resume a past session in a terminal

- What the user does: picks a past session and the app opens a terminal in the session's directory and starts the agent CLI with its resume flag, so the conversation continues.
- Rules and edge cases:
  - The command changes into the session's working directory first (when known), then runs the resume. The shell is taken into account: cmd, PowerShell, and POSIX shells each get their own quoting and separators. Environment variables that must not leak into the agent can be cleared on that agent only.
  - Codex sessions in a MassOrca-managed account home get a `CODEX_HOME` prefix; sessions in the real home run without one, and their daemon-inherited Codex home variables are removed.
  - OMP and Prime Agent resume by absolute transcript path, not by id. Gentle Shell Pi sessions resume through the Gentle Shell launcher with a file argument.
  - Kimi must run from the session's work directory; the CLI rejects other directories.
  - Antigravity IDE sessions are not the same conversation; they are opened with a reference prompt (inferred; see low-confidence list).
  - Refusal: when a structured native chat already holds the conversation, the legacy terminal resume is refused (conflict or unknown owner). A fork is allowed because it writes a new conversation.
  - Mobile clients cannot call the "prepare resume" step; the desktop or runtime host does it.
  - SSH hosts: the desktop does not materialize remote transcript paths and returns "use real Codex home: false".
- Client asks / core pushes: `aiVault:prepareSessionResume` (desktop or runtime host). Runtime hosts answer through the runtime call; unavailable runtime returns "reconnect and retry".
- Persists: nothing in the agent store. Launch state is handled by the terminal domain.
- Depends on: HIST-03 and HIST-04; terminal sessions (terminal domain); structured chat ownership (HIST-08).

### HIST-08 Adopt or fork a past session into a structured chat

- What the user does: opens a past Claude, Codex or Antigravity conversation as a structured (native) chat, continues it there, or forks it into a new conversation.
- Rules and edge cases:
  - The server resolves the transcript and the account home itself; the client names only the conversation. A client-supplied path or home is never used.
  - Candidate account homes are tried in order (selected account first); the first hit wins.
  - One writer per conversation. A conversation held by another structured session or a legacy writer is refused ("held elsewhere" or "ownership unknown"). Retrying the same committed operation replays its result.
  - Adoption imports the transcript into a MassOrca journal; the native provider runs against the adopted handle.
  - Codex does not lock: a second app server on the same thread diverges, so the refusal is the correctness guard.
  - Only the providers in the adoptable list can adopt. The list is `claude`, `codex`, `antigravity`; the module comment mentions only Claude and Codex (see low-confidence list).
- Client asks / core pushes: structured session create with an adoption handle (agent session wire); legacy resume refusal messages.
- Persists: a journal entry per adopted session in MassOrca's own store (the agent's transcript itself is not modified).
- Depends on: HIST-06 (transcript reader), the structured chat domain (lease, journal, provider runtime), accounts domain (account homes).

### HIST-09 Delete a past session

- What the user does: removes a past conversation from the list and from disk.
- Rules and edge cases:
  - Allowed for 14 agents only: Gemini, Copilot, Cursor, Hermes, Devin, OpenClaw, Droid, Pi, OMP, Claude, CodeBuddy, Rovo, Grok and Cline. The rest are excluded on purpose: Antigravity and Kimi (separate registry files would keep dangling entries), Codex (session index and hardlinked aliases would reappear), OpenCode 1.17 (SQLite row, not a file).
  - Local sessions only. The delete validator accepts only paths the scanner itself would list, so a path outside the discovery roots is refused.
  - Local deletion moves the files to the OS trash. On WSL the removal happens inside the distro and is not sent to trash. Missing files count as success.
  - Symlinked roots are resolved before the check.
- Client asks / core pushes: `aiVault:deleteSession` with agent, file path and host; returns deleted, rejected (with reason) or failed. A successful delete invalidates the list cache.
- Persists: removes files from the agent store (the only write to foreign stores).
- Depends on: HIST-01, HIST-03 and HIST-04.

### HIST-10 Remote history on SSH and runtime hosts

- Source: pv96 (unmerged). The relay's own filesystem scanner (`src/relay/ai-vault-service-filesystem.ts`) carries the same Antigravity worktree and automated-run fixes as the local scanner (`51b9d336aef`, `4c87a8681a7`).
- What the user does: sees and resumes history that lives on a remote machine, the same way as local history, when that host is connected.
- Rules and edge cases:
  - SSH hosts: MassOrca asks a relay on the remote host to scan its own stores (how the relay is installed or started was not traced). The relay timeout is 130 seconds for the list and 15 seconds for title resolution. A relay that was given a full budget and still timed out is reported as a host issue; the slower filesystem crawl is not used in that case.
  - If the relay is missing or too old, the desktop falls back to a legacy crawl over the SSH filesystem, within the rest of the leg's budget.
  - Relay list is capped at 1000 rows and 64 scope paths; scope path length up to 4096 characters.
  - Remote search status can be read through the relay, but the desktop refuses to enable search on an SSH host.
  - Native chat transcript view is not offered for Grok, OMP, OpenCode and Antigravity on SSH (they need the owning host).
  - Runtime (environment) hosts answer the same list, search and resume calls through the runtime connection. An unreachable runtime returns a "reconnect and retry" error.
  - Deletion is local only.
- Client asks / core pushes: relay methods `aiVault.listSessions`, `aiVault.resolveSessionTitles`, `aiVault.searchSessions`, `aiVault.searchStatus`; runtime calls for search and resume.
- Persists: on the remote host, the same parse cache and search index as local (inferred from the relay design, not traced).
- Depends on: HIST-01, HIST-05, HIST-07; the remote hosts domain (SSH connection, relay installation, runtime environments).

### HIST-11 WSL history on Windows

- What the user does: sees Claude, Codex and other history that lives inside a running WSL distro, mixed with Windows history.
- Rules and edge cases:
  - Windows only. Only distros that are running at scan time are read; a stopped distro is not started just to scan it.
  - Each agent's layout is read under `/home/<user>` of each running distro in addition to the Windows home.
  - Deletion of WSL files is done inside the distro, permanently (no Recycle Bin).
- Client asks / core pushes: same list calls; delete validation uses the same WSL-aware roots.
- Persists: parse cache entries keyed per distro root.
- Depends on: HIST-03 and HIST-04; the WSL integration domain.

### HIST-12 Claude account history sharing

- What the user does: works with several Claude accounts and sees the same conversation history across them, without losing history when accounts are switched or moved.
- Rules and edge cases:
  - Each managed Claude account has its own config home. History surfaces (`projects`, `sessions`, `session-env`, `file-history`, `shell-snapshots`, `todos`, `paste-cache`, `tasks`, `plans`, `transcripts`) can be linked into the system default on macOS and Linux (symlinks). On Windows the account folders are read directly.
  - Moving history between homes keeps going past a locked entry, so one failure does not hide the rest.
  - Prompt history has its own merge step; cross-filesystem moves are detected and handled separately (details not read).
- Client asks / core pushes: no direct renderer call; the scanner gets account project dirs from the host.
- Persists: the linked or moved history under the account homes; a sharing ledger that records what MassOrca shared (so unshared keys are left alone).
- Depends on: accounts domain (Claude account management, subscription migration). Decide with that domain whether history sharing is part of account migration.

### HIST-13 Cursor IDE sign-in state read

- What the user does: nothing directly in this domain. The product reads the Cursor desktop app's local sign-in state so an existing Cursor login can be recognised.
- Rules and edge cases:
  - Read-only, on the foreign SQLite worker, with a 250 ms open timeout. A missing database is reported as missing; any other failure is reported as a read failure.
  - Reads four keys from the Cursor IDE's item table: access token, cached email, membership type and subscription status. The access token is read as a secret; this inventory does not record its use.
  - Consumer not traced in this read; likely the accounts domain.
- Client asks / core pushes: internal worker call only.
- Persists: nothing in this domain.
- Depends on: accounts domain; owner decision on whether Cursor login migration is in scope. Candidate input for the "migrate agent accounts and subscriptions" requirement.

### HIST-14 OpenCode live session binding

- What the user does: nothing directly. The agent status view can find the OpenCode session that belongs to a tab.
- Rules and edge cases:
  - Reads only rows newer than a cursor from OpenCode's database, ordered by creation time and id, so rows created in the same millisecond are not skipped.
  - An absent database is empty, not an error; other open failures are surfaced to the log.
  - Unknown table shapes read as empty, so an OpenCode schema change makes sessions unbound rather than failing.
- Client asks / core pushes: internal to the agent hooks server.
- Persists: nothing in this domain; where the cursor is held inside the hooks server was not traced.
- Depends on: agent status and hooks domain; HIST-04 (OpenCode store).

### HIST-15 Projectless chat folders

- What the user does: starts a chat without choosing a project; the chat gets its own folder under a "Chats" group, and its history is recorded against that folder.
- Rules and edge cases:
  - Folder root comes from a setting; default location is set by the settings domain.
  - Layout: one folder per local calendar day, one folder per chat, with a six-character lowercase id. Name collisions retry up to 1000 attempts.
  - Not available for SSH locations (refused as "ssh_location_unsupported") and not on the web runtime.
  - The temporary id is replaced when the first agent message names the folder workspace.
- Client asks / core pushes: create-chat call returns the workspace key and folder path; the session list then shows the chat under its folder.
- Persists: folders on disk under the chats root; a group and folder workspace in the store.
- Depends on: workspaces domain (folder workspaces, groups); HIST-01 for listing.

### HIST-16 Automated-run filter, delegated child sessions and subagent transcripts

- Source: pv96 (unmerged). MassOrca's own headless runs are recorded when they spawn and their transcripts are marked automated; a run that never saved its end claims only a session born at its spawn (`4e60c4f45b6`, `c7d082fb308`); Antigravity conversations started for another conversation are flagged as headless (`4c87a8681a7`); a running subagent is listed under its parent before it finishes, and a replayed scan refreshes delegated-child links (`17cadabf42e`, `12d58a468c8`, `659ff0f97c7`).
- What the user does: hides headless or automated runs with one switch; sees delegated child sessions nested under their parent; opens Claude or OMP subagent transcripts from their parent.
- Rules and edge cases:
  - Automated runs are recognised from a fixed opening prompt or title prefix, from the spawn record of MassOrca's own headless runs (chat naming, commit and pull-request text, branch names and reply suggestions; pv96), and, for Antigravity, from a parent id and nesting depth in its conversation database (pv96). Claude and Gemini transcripts have no structural marker found. A Codex exec run with no known prefix still counts as automated.
  - The switch never hides a child from a visible parent. A child whose parent is hidden is shown as a root when the switch leaves it visible.
  - Delegated children are linked by MassOrca's own orchestration records, not by the agent's files.
  - Subagent transcripts are listed on demand under their parent, and only for Claude and OMP; other agents return an empty list.
- Client asks / core pushes: `aiVault:listSubagentSessions`; the list rows carry the parent id and status.
- Persists: nothing new; reads orchestration records.
- Depends on: HIST-01, HIST-02; the delegated-agents domain.

### HIST-17 Process memory dashboard (not AI memory)

- What the user does: opens a memory view that shows RAM use of the app, of each worktree and of each terminal's process tree, with a short trend line.
- Rules and edge cases:
  - Not related to AI agent memory. No agent memory or notes feature was found in the code read. The name "memory" in the repository refers to this RAM dashboard only.
  - Windows reports working set per process (through a process sweep, with a fallback); other platforms use resident set size.
  - Concurrent polls share one sweep, so a burst of polls starts only one child process.
  - Memory pressure on macOS and Linux uses host commands; after a failure that path is switched off for the rest of the session.
  - Samples are kept in a bounded ring per worktree plus one app-total key.
- Client asks / core pushes: `memory:getSnapshot` returns the snapshot.
- Persists: nothing; in-memory rings.
- Depends on: terminal sessions (PTY registry), workspaces (worktree buckets). Better placed in a resources or process domain; listed here only because the brief put `memory/` in scope.

### HIST-18 Import sessions from other agent tools

- What the user does (requirement from the owner): work started in another agent tool continues in MassOrca, from that tool's past conversation.
- Current state in the reference app (building blocks only, no import feature):
  - Discovery (HIST-03, HIST-04) finds the other tool's conversation on the same host.
  - Resume (HIST-07) re-launches the vendor CLI on its own file. It works only with that CLI installed and only on the host that holds the file.
  - Adoption (HIST-08) copies a Claude, Codex or Antigravity transcript into a MassOrca journal and continues it as a structured chat. No other agent has this path.
  - Nothing converts one agent's transcript format into another's, and nothing copies a session from a remote or WSL host into the local machine.
- Rules the owner should decide:
  - Whether "import" means read-only continuation through the vendor CLI (cheapest) or a copy into MassOrca's own store (needed for agents without a structured reader).
  - Which source agents come first. Claude and Codex have the strongest existing support.
  - Whether imported sessions must be deletable from MassOrca without touching the source file.
- Client asks / core pushes: none exists today.
- Depends on: HIST-03 (discovery), HIST-07 (resume), HIST-08 (adoption path to reuse), HIST-04 for other agents, and the structured chat domain.
