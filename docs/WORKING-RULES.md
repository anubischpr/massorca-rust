# Working rules

Decided 2026-11-20. Every session and subagent working on this repository
follows these rules. `docs/PLAN.md` holds the architecture and the phases.

## Autonomy

- The work is autonomous. Decide and record; do not stop to ask the owner.
- Record each decision that someone would later ask "why" about in
  `docs/decisions/NNNN-title.md`: context, decision, alternatives, consequences.
- Stop only for something that could damage the production MassOrca install or
  data outside this project.
- Everything in `docs/inventory/` gets built. The first version is only the
  first milestone, not the end.

## Sources of truth

- Behaviour: `docs/inventory/`. Where it separates "observed current
  behaviour" from "owner direction", owner direction wins.
- Interface: `docs/design/`, starting with `handoff.md` and `agreements.md`.
  The artboards live in the published "MassOrca Screens" artifact.
- The old app is a behaviour reference only. Never copy its code. Never modify
  its repository or its worktrees.

## Roles

- The orchestrating session plans, decides, reviews and merges. It writes no
  product code.
- Child sessions do the work, each in its own git worktree and branch, at most
  on Opus 5.5 at medium effort.
- Child sessions delegate to subagents to move fast. Subagent prompts start
  with a `Routing:` line and pass no `model`. Parallel subagents get disjoint
  files.

## Commits and pull requests

- Conventional Commits. No AI attribution and no `Co-Authored-By` lines.
- A commit is one reviewable unit: one behaviour with its tests and its docs,
  about 400 changed lines as a guide. Never a commit of thousands of lines.
  Generated files and lock files do not count, and go in their own commit.
- A pull request is one coherent slice a person can review in one sitting.
  When a feature does not fit, chain pull requests, each based on the previous.
- Pull request text is plain language for a reviewer who has not seen the
  code: what changes for the user, the mechanism, why this approach, how it was
  checked.
- Work reaches `main` only through a pull request. The GitHub account is
  `anubischpr`; the repository is already configured for it.

## Tests

- Every core feature has a CLI entry point and is tested through it.
- Behaviour changes are test-first where a deterministic test is possible.
- Each session runs only the tests of its own change, plus the workspace
  checks (`cargo fmt --check`, `cargo clippy`, `cargo test`) before a pull
  request.
- Full end-to-end validation runs only at the milestones below. It is
  expensive; do not run it in between.

## Environments

| Environment | Where | Notes |
|---|---|---|
| Windows | this machine | primary development host |
| Linux | WSL (Ubuntu 24.04) locally; the VPS for the real server | VPS is Arch Linux x86_64, reached over Tailscale or SSH |
| Android | the owner's phone over ADB | `adb` is in the Android SDK `platform-tools`, not on PATH |

The VPS runs MassOrca in production. The new server is installed beside it,
in its own directory and on its own port, as an unprivileged user. Never touch
the production install, its data, its ports (6768, 6775) or its services.

## Credentials

- The owner's agent credentials are copied into a clean profile of this
  project, so the owner's own profile stays untouched for a first-run test.
- Credentials never leave this machine: not in the repository, not in logs,
  not in test fixtures, not on the VPS.

## Milestones (end-to-end validation points)

| # | Milestone | Validated on |
|---|---|---|
| M1 | Core and CLI: workspaces, terminals that outlive the client, git | Windows, WSL |
| M2 | Headless server on the VPS, reached over Tailscale, driven by the CLI from Windows | Windows to VPS |
| M3 | Agents: an agent runs in a workspace with the migrated credentials; status, delegation and bring-back from the CLI | Windows, VPS |
| M4 | Desktop app in GPUI covering the first-version scope | Windows, Linux |
| M5 | Android app driving the VPS server | phone to VPS |
| M6 | Extensions and the rest of the inventory, delivered in batches, each batch with its own validation | all |

Each milestone ends with a short report in `docs/milestones/` stating what was
run, on which environment, and what failed or was not checked.
