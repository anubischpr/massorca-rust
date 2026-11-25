# 0092: Vendor programs on the registry PATH (Windows)

Status: accepted, 2026-11-24. Narrows the deferral in 0032 for vendor programs only.

## Context

A service started before a PATH change does not see the folders that change added, so a CLI or npm installed after the
app started looks missing. The extension detect, install and verify steps (0062) then report "`npm` was not found on
PATH" or a CLI as not installed. 0032 defers the registry merge of the machine and user `Path` to a later step, for
terminals (TERM-17). This decision brings it forward for vendor programs, because that is where the failure hurts.

## Decision

- On Windows, a vendor program is resolved on the service's `PATH`, then the machine `Path`, then the user `Path` from
  the registry. The process `PATH` comes first so a lookup that already succeeds keeps its result (first match wins).
  Machine then user is how Windows builds the `PATH` of a new process.
- The registry is read through the Windows registry API (`RegOpenKeyExW`, `RegQueryValueExW`), not `reg.exe` and not
  PowerShell. A `REG_EXPAND_SZ` value has its `%NAME%` expanded from the service's environment. An unknown name stays as
  typed.
- Entries are trimmed and unquoted, empty ones are dropped, and a repeat is dropped by its first occurrence, compared
  without case and without trailing separators.
- A vendor command runs with the same search path, absolute entries only, so a `.cmd` shim finds the tools it calls. The
  program is resolved on that path and the command's `PATH` is that same path, computed once.
- The registry entries are the machine `Path` plus the user `Path` of the account the service runs under, not of the
  person installing. If the service runs under another account, that account's user `Path` is read, because HKCU is
  that account's environment.
- `vendor_search_env` in `massorca-core-agents` returns the search path and where it came from, `process` or
  `registry`. Agent launch does not use it yet.
- Terminals keep the inherited `PATH`. Unix keeps the process `PATH` alone.
- `MASSORCA_VENDOR_PATH=process` keeps the process `PATH` alone on Windows too. The CLI tests set it, so a host's own
  npm or CLI folders cannot change what they find.

## Alternatives

- Read the registry with `reg.exe` or PowerShell. Rejected: each lookup would start another program.
- Put the registry entries before the process `PATH`. Rejected: a lookup that already succeeds could find a different copy.
- Tell the person to restart the app after an install. Rejected: it is the failure this decision removes.

## Consequences

- Two registry reads happen per vendor lookup. Each is small.
- A folder that only the registry lists is found and run without a restart.
- A registry read that fails counts as no value, and the process `PATH` is used alone.
