# 0151: Host programs as tool packages

Status: accepted, 2026-11-25. Covers SKL-30 and the Tools rows of SKL-27 in `docs/inventory/skills-and-plugins.md`, on
top of the extension install (0062) and vendor program detection (0092).

## Context

SKL-30 installs, updates and uninstalls Git, GitHub CLI, Node.js and the agent CLIs on a host, with fixed vendor commands
per OS, and never takes the user's data. The agent CLIs already install through their agent packages (0062). Git, GitHub
CLI and Node.js had no package, and nothing could update a program. The reference app (pv96, behaviour only) keeps its
installer commands in the app; its executors and plans were read by name only.

## Decision

- **Kind.** A package contributes `hostPrograms`: an id and the same `cli` block an agent adapter carries (binary,
  verify, vendor source, install and uninstall steps per OS), plus an optional `update` list. `tools` was already the
  name of agent-callable MCP tools, so the contribution has its own name. A host program needs a `process` permission,
  its id may not repeat an agent adapter id, and its commands must start with an allowlisted vendor program. Only the
  built-in catalogue may declare a vendor command, as for agents.
- **Category.** The listing category `tools` is new; a package with host programs and no agent adapter derives it.
- **Catalogue.** `massorca.git`, `massorca.gh` and `massorca.node` are built in. Windows uses `winget --id <Id> -e`,
  macOS uses `brew`. pv96's exact installer flags were not readable (its executors and plans were read by name only),
  so these follow each vendor's documented command.
- **Manual steps.** An install step may be marked `manual`: it needs administrator rights, so MassOrca never runs it.
  The review returns a `manual` plan with the commands, and the Marketplace and the CLI show "Run this command
  yourself" with the exact command and a Copy button. Only install steps may be manual, and an OS lists either one run
  step or several manual ones. Linux gets its install path this way: `sudo apt-get install` or `sudo dnf install`. The
  package registers, and the program is found once the person has run the command.
- **Detection.** A host program is planned and listed through the service's program resolver, so the registry PATH
  merge of 0092 applies on Windows. Items carry `programs` beside `agents` (new optional field).
- **Install.** The install review, consent fingerprint, vendor command, verify step and lockfile record are the ones
  0062 uses for agent CLIs. The fingerprint of a package without an `update` step is unchanged.
- **Removal.** Removing a tool package runs its uninstall only for a program the package installed, and only with the
  uninstall consent. A program that was already on the host is left alone, so a Node.js from a version manager or a Git
  the user installed is never removed.
- **Update.** `extensions.program_update_review` returns the update command for this OS and a fingerprint over the
  package, its version, the program, the command and the verify command. `extensions.program_update` runs exactly that
  command, then the verify command, and refuses without the fingerprint. Both are refused
  (`extension.program_update_refused`) for a program the package did not install. They sit behind capability
  `extensions-programs/1`; the run needs host administration and is refused for an agent run.
- **Desktop.** The Marketplace has a Tools category. Install uses the existing review dialog: one sentence, the
  technical lines under Show details, and the button Install or Add to MassOrca; a tool already on the host is added
  with "MassOrca will add the <name> you already have installed on this computer." Update shows on the tool's item page
  and in the More menu of its card only when the service offers `extensions-programs/1`, the item is on This computer,
  MassOrca installed the program and the package has an update command for this OS. It opens the same dialog: "MassOrca
  will install the latest <name> on this computer.", the command and consent under Show details, and the button
  Update. Update is not offered for paired hosts yet.
- **CLI.** `massorca extension program-update <id> <program>` prints the command and stops without `--yes` or
  `--consent`. `extension doctor` and `extension show` list host programs.

## Alternatives

- Reuse `extensions.update` for program updates. Rejected: it replaces package files and never runs a vendor command.
- Update a program MassOrca did not install. Rejected: the tool that installed it owns its updates.
- Install on Linux with `sudo apt`. Rejected: MassOrca never elevates or asks for a password.

## Consequences

- An older client lists a tool package as an extension, because it maps an unknown category that way. That is
  accepted: every wire change here is additive.
- Tests run every vendor command against fakes; no session runs a real install, update or uninstall.
- An installer that asks for elevation by itself, such as a machine-wide winget install, is not detected in advance.
- The Git one-click adoption of pv96 remains open.
