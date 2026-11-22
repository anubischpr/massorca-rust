# 0056: Agent credentials per profile

Status: accepted, 2026-11-21.

## Context

Agents authenticate with files in their own home directory (Claude's
config directory, `CODEX_HOME`, `PI_CODING_AGENT_DIR` for Gentle Shell),
each of which an environment variable can redirect
(`docs/inventory/accounts-and-usage.md`). The working rules say the owner's
agent credentials are copied into a clean profile of this project so their
own profile stays untouched, and credentials never leave the machine: not
in the repository, logs, fixtures or on the VPS. Claude refresh tokens are
single use, so a login copied to two places will fail in one of them.

## Decision

- **A credential home per profile and kind**:
  `<profile>/agents/homes/<kind>/`. When it exists, every run of that kind
  in that profile gets the kind's home variable (`CLAUDE_CONFIG_DIR`,
  `CODEX_HOME`, `PI_CODING_AGENT_DIR`, from the declaration) pointing at it.
  When it does not exist, nothing is set and the agent uses its usual home:
  MassOrca never invents an empty home that would look like a logged-out
  agent.
- **Stripped variables.** When a profile home is used, the declaration's
  `strip_env` names (for Claude: `ANTHROPIC_API_KEY`,
  `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`; for Codex:
  `OPENAI_API_KEY`) are removed from the inherited environment, so a stray
  key in the service's environment cannot override the profile's login.
  A caller's explicit `env` on start still wins.
- **Import: `massorca agent credentials import <kind> --from <dir>`.** The
  service copies, from a directory the person names, only the files the
  kind declares (`credentials.files`; Claude: `.credentials.json`,
  `.claude.json`; Codex: `auth.json`, `config.toml`). Rules:
  - only regular files; symbolic links, directories and missing names are
    reported and skipped (a missing required file fails the import);
  - at most 1 MiB per file;
  - each file is written to a temporary name and renamed into place;
    on Unix the home is `0700` and files `0600`; on Windows they inherit the
    profile directory's owner-only access (0004);
  - *Amended 2026-11-21:* 0004 does not make the profile directory
    owner-only on Windows (it keeps its inherited access); the home gets its
    own protected DACL (owner and SYSTEM, inherited by its files) before any
    file is written;
  - the result lists file names and sizes only, never contents; nothing
    about the files is logged beyond names and sizes;
  - an existing home is replaced only with `--replace`.
  A kind with no declared files refuses import with
  `agent.credentials_unsupported`.
- **`credentials show <kind>`** says whether the home exists and which
  declared files are present, with sizes and modification times.
  **`credentials remove <kind>`** deletes the home.
- **Import copies, it does not move.** The single-use refresh token rule
  means the person should stop using the source login after importing;
  the command's text output says so. MassOrca does not check or refresh a
  login in Phase 4; the first agent run shows whether it works.
- **No credential ever in test data.** Tests use the fake agent with a fake
  token file; the real import of the owner's credentials is done by hand by
  the orchestrator, outside any test.

## Alternatives

- **Point agents at the owner's real homes.** Their own profile would not
  stay untouched, and two programs would rotate the same tokens.
- **Copy the whole agent home.** It carries history, caches and project
  data the profile does not need, and makes "what was copied" unclear.
- **Encrypt the copies.** The agent reads them as plain files from the home
  directory; encryption at rest would need the agent's cooperation.
  Restrictive permissions are what the agents' own homes use.

## Consequences

- An independent security review should look at import (path handling,
  symlinks, permissions on Windows), at the environment stripping, and at
  the hook token before M3 uses real credentials.
- Antigravity has no declared home or files yet; its credentials stay in its
  usual place until they are traced.
