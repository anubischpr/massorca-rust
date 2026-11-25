# 0097: Import source detection

Status: accepted, 2026-11-24.

## Context

`agent accounts import <KIND> --from <DIR>` (decisions 0056 and 0059) copies a kind's declared `credentials.files`
from one absolute folder, and every declared file is required. Two problems follow:

- People do not know which folder to name.
- No single folder may hold every file. On a default Windows install Claude keeps `.credentials.json` in
  `<home>\.claude` but `.claude.json` in `<home>`.

## Decision

- A kind's `credentials` declares, as data and per file, where its CLI normally keeps each credential file. Both
  fields are additive:
  - `optional`: declared file names an import may skip when they are missing. A declared file not listed is required.
    Absent means every declared file is required, the rule before this decision.
  - `sources`: each `{ file, env?, dir, windows?, macos?, linux? }`. `dir` is the default folder template, and a
    platform key replaces it on that platform. The only placeholder is `{home}`. When `env` names a variable that is
    set, not empty and an absolute path in the service's environment, the file is looked for in that folder instead.
  - The kind's problems list a source or optional entry that is not a declared file, a file named twice, an `env`
    that is not a variable name, and a template that neither starts with `{home}` nor is an absolute path.
- `{home}` is `USERPROFILE` on Windows and `HOME` elsewhere, which the vendor CLIs read, when that variable is an
  absolute path. Otherwise the platform's own answer (`std::env::home_dir`) is used. Resolution is a pure function of
  the platform, the home and an environment lookup, so tests never touch the real home.
- The built-in declarations:
  - `claude`: `.credentials.json` required in `{home}/.claude`; `.claude.json` optional in `{home}`. Both are moved by
    `CLAUDE_CONFIG_DIR`.
  - `codex`: `auth.json` required and `config.toml` optional, both in `{home}/.codex` and moved by `CODEX_HOME`.
  - `gentle-shell` and `antigravity` are unchanged.
- `agent.accounts.import_candidates` (read, capability `agent-import-detect/1`) takes `{ kind? }` and returns
  `{ candidates: [{ kind, ready, folder?, files: [{ name, required, found, path?, env? }] }] }`.
  - Without `kind`, it covers every valid, enabled kind that declares credential files. An unknown kind is
    `agent.kind_not_found`.
  - `found` means the import would accept the file: a regular file, not a link, at most 1 MiB, judged from its
    metadata alone. Detection never opens a file and never returns contents.
  - `ready` means every required file was found. `folder` is the folder of the first required file. `env` names the
    override variable when it was set and used.
  - Below `workspace` scope (decision 0071), `folder` and `path` are omitted.
- `agent.accounts.import` takes an optional `from`. Without it, each file is read from its detected path, which needs
  capability `agent-import-detect/1`; the client refuses it before sending when the service does not offer it.
  - With either source, a missing optional file is skipped. A missing required file gives `not_found`, with a reason
    that names each missing required file and where it was looked for.
  - Every file read keeps the existing checks: no link, a regular file, at most 1 MiB as read from the open handle.
  - The deprecated `agent.credentials.import` keeps its folder and follows the same optional rule.
  - An account counts as having its files when every required file is in its home.
- The CLI:
  - `agent accounts import <KIND>` without `--from` prints each file it will copy, then imports. When a required file
    is missing it copies nothing and fails with `agent.credentials_invalid`, naming each missing file and where it
    was looked for. `--from DIR` overrides detection.
  - `agent accounts candidates [<KIND>]` prints, per kind, its readiness and folder, then each file's name, whether
    it is required, whether it was found, and its path. `--json` prints the method result.

## Consequences

- An import works on a default install with no folder named, even when the files live in different folders.
- A kind declaration gains two optional fields. A peer that predates them still parses the kinds it knew, but a
  declaration that uses them is unknown to it, since declarations deny unknown fields.
- `AccountsImportParams.from` becomes optional on the wire. A client that always sends it is unaffected.
- The `not_found` reason now names the missing files and their paths, instead of a fixed sentence. Import needs
  `workspace` scope, which already sees host paths.
- The service's own home is used for detection. A service started with a different `HOME` or `USERPROFILE` than the
  person's shell looks in that home.
