# 0116: Service tool detection

Status: accepted, 2026-11-25. First version of PLAT-08, service side. The desktop Setup checklist and preflight do not
read it yet.

## Context

The desktop preflight has to know which tools the machine that runs the work has: git, the forge CLIs, and the shells a
terminal can run. Only git was reported (`service info`, decision 0014 and the git status field). A paired or SSH host
runs its own service, so the answer must come from the host that runs the work, never from the desktop that asks.

pv96 (`d3c6311a531`) found each tool with a PATH lookup and asked `--version` under a short timeout. Its preflight
(`src/main/preflight/agent-detection.ts`, `src/main/ipc/preflight-command-exec.ts`) used a 5 second limit for each
command, kept the positive result for the session, and retried a negative one. Its `pwsh -Version` probe (`pwsh.ts`)
was the only shell probe. Its `--version` probe (`agent-cli-version-probe.ts`) capped the output at 4 KiB and killed the
program on the limit.

## Decision

- `service.tools` (capability `service-tools/1`, `Read` permission) returns `{ tools: [...] }`. Each tool has `id`
  (open text), `kind` (`vcs`, `forge`, `shell`, or `other`), `state` (`present`, `missing`, `too_old`, `unknown`, or
  `other`), `version` and `path`, each `null` when not reported. A kind or state a client does not know decodes as
  `other`.
- The tools are git, `gh`, `glab`, `pwsh`, and on Windows `powershell`; on other hosts `bash`, `zsh` and `sh`. WSL is
  never probed: the service runs no WSL command, and a `bash.exe` on a Windows `PATH` is usually WSL's launcher, so
  `bash` is left out there.
- Git comes from `Git::availability` in core-git, the probe `service info` already uses. Its `too_old` and `unknown`
  states map to the same tool states.
- A tool is found with the terminal's PATH lookup (`resolve_program`, which applies PATHEXT on Windows). A missing tool
  is never run.
- A forge CLI is asked for `--version` with a 5 second limit, killed at the limit. Output past 4 KiB is discarded. A
  failed start, a non-zero exit, a timeout, or output with no dotted number is `unknown`.
- A shell is found and reported `present` with no version. Starting a shell to read a version can load the user's
  profile, so no shell is started.
- The probes run on their own threads, so the wait is the slowest probe's. Each probe starts a program through
  `massorca_process`, which keeps the console window hidden. No `cmd.exe /c`, no PowerShell, no network.
- The answer is cached for the service run. `refresh: true` detects again. The cache holds one answer, and a
  concurrent caller waits for the detection in progress.
- Detection runs on the host of the service. `massorca service tools --host` lists the other host's tools.

## Alternatives

- Extend `service.info` with the list. Rejected: `service info` is identity, and the list needs its own refresh and
  its own capability so an older client never misreads it.
- Probe shells with `--version`. Rejected: a shell may load the user's profile, so a probe would run user code.
- Read the version of a shell from file metadata. Rejected for this version: file metadata does not carry the shell's
  version reliably on every platform.
- Keep a timed cache across runs. Rejected: a tool installed after the service started would look missing until a
  restart, the failure decision 0092 removes for vendor programs.

## Consequences

- A `gh` or `glab` that hangs costs 5 seconds once per detection, not per tool in sequence.
- A tool whose version cannot be read is `unknown` with a `path`, so the desktop can say it is there but unverified.
- Each new tool is one entry in `host_tool_specs`, with its `--version` arguments.
- A `.cmd` shim on Windows runs through the standard library, which starts `cmd.exe` for it. `gh` and `glab` are native
  executables there, so this applies only to a shim a user installed.
- The probe kills only the program it started. A grandchild that keeps the output pipe open does not block the answer:
  the output is read for at most one second after the exit.
