# 0167: One-shot chat titles

Status: accepted, 2026-11-25.

## Context

- A chat's title should say what the work is about once its first prompt and the agent's first reply exist. The
  session owner decides when to ask and renames the workspace; it needs an answer it can trust in a few seconds.
- The previous product (pv96) named a branch by running the same agent once, non-interactively, with the first request
  and the first reply in the prompt, and kept only a short slug.
- A title run must not look like a session: no session row, no terminal, no status hooks, and no run identity that the
  agent could use to act as one of MassOrca's runs (decisions 0160 and 0161).
- A one-shot run still loads the account home's settings: its allow rules, its MCP servers and its tools. Text injected
  into a first prompt could therefore drive tools on the person's account. A review found this, so the run restricts
  its tools.

## Decision

- A kind declares its non-interactive form as data:
  `"oneshot": { "args": [..., "{prompt}", ...], "model_flag_reuse": true, "optional_args": [...] }`.
  `{prompt}` appears once; the prompt is one argument. `model_flag_reuse` passes the kind's model arguments when a model
  is named. Each `optional_args` entry is a flag and its literal values, passed only when the program's `--help` output
  lists the flag as a whole word. The listing is the instruction channel's cached probe, so `--help` runs once per
  installed program; no listing, or a `--help` that does not finish with exit 0, passes none of them.
- Claude declares `-p {prompt} --output-format text`, reuses its model flag, and lists these optional arguments:
  `--max-turns 1`, `--permission-mode default`, `--strict-mcp-config`, `--mcp-config {"mcpServers":{}}`,
  `--disallowedTools` with one comma-separated argument naming Bash, Edit, Write, MultiEdit, NotebookEdit, WebFetch,
  WebSearch and Task, `--tools ""`, and `--no-session-persistence`. The restriction is not verified against an installed
  CLI here: the flags and the comma-separated tool list are assumptions until a real CLI confirms them.
- Hooks in the account's own settings still load. These arguments stop tool use (one turn, denied tools, no MCP servers),
  so a hook that fires only on a tool call does not run; a hook that fires on the prompt itself is not disabled.
- The title request asks for an English title of two to four words in ASCII, whatever the request's language. The
  request is one line: every line break and run of whitespace becomes one space, because a Windows `.cmd` launcher
  refuses an argument with a line break. A leading label such as `Title:` or `Título:` is removed before slugging.
- `agent.generate_title` (capability `agent-title/1`) runs the kind's startable program once, with the account's home:
  the named account, else the kind's active one. The slug is at most four lowercase words, `a-z`, `0-9` and `-`, at most
  48 characters, with accents folded so no letter is lost. A reply with no usable word is `agent.oneshot_failed`.
- Below host-admin a caller may name only the kind's active account; any other account is `permission.denied`. The
  method takes no project, so a project's own choice is not reachable through it.
- The run strips the kind's inherited and account variables and the three run identity variables. It installs no status
  hooks. Its working folder is a fresh folder under the profile's `tmp` folder, removed when the run ends. A new run
  first sweeps `oneshot-*` folders under that `tmp` folder that are older than an hour.
- The time limit is 30 seconds; past it the process tree is stopped and the call fails with `agent.oneshot_timeout`.
- Refusals are typed: `agent.oneshot_unsupported`, `agent.credentials_invalid`, `agent.oneshot_timeout`,
  `agent.oneshot_failed`. A reply is never logged, and an error names the kind and the cause only.
- The naming rules live in `crates/core-agents/src/title.rs`; the run and the account rules in `oneshot.rs`.

## Consequences

- A kind gains a title only when it declares a one-shot form; the session owner keeps its own name until then.
- The prompt and the reply travel in the run's arguments, as the kind's own arguments do; a process listing on the same
  machine can show them while the run lasts.
- Open: Claude's print mode may keep its own transcript under the account home. The restriction passes
  `--no-session-persistence` when the CLI lists it, which is unverified here.
- Open: the hook gap above is not closed by any declared argument.
