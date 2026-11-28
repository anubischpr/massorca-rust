# Open questions for the owner

These are decisions for the owner. Nothing in this document is decided, except the items marked **Answered**. Those record decisions the owner took in the design session on 2026-11-12 (`docs/design/screen-inventory.md`), or decisions taken on the owner's behalf while the owner was away (`docs/design/pending-owner-review.md`). Items marked **decided on the owner's behalf, pending review** need the owner's confirmation.

The inventory describes pv96 (commit `d3c6311a531`, `custom/integration` plus 852 unmerged commits): 542 features, 190 proposed for the first version (README summary table, as read for this revision).

Order of this document:

1. **Blocking the first-version scope.** Open items whose answer changes what is in the first version or its largest costs, largest impact first.
2. **Other decisions, by impact.** Every other open item.
3. **Research notes.** Browser engine research, unchanged in content.
4. **Answered decisions (for reference).**

Items are numbered within each section. Each open item gives the question, why it matters, the options (where the source gives them), and a link to the source. Options marked "proposed here" are not in the source documents.

## Blocking the first-version scope

Ranking judgement: I ranked by how far each answer changes the first-version set or its largest costs, following the order of the owner's brief. Security items come after the core model items, and the scope items for extensions, browser and remote work come last.

1. **First-version agent set.** How many agents must the first version support? Which source agents come first for importing sessions? Does the first version in [first-version.md](first-version.md) (190 features) stand?
   - Why it matters: sets the Phase 4 scope and the adapter work for each agent. Items 2 and 3 depend on it.
   - Options: one agent; two agents.
   - Source: [agent-status.md](agent-status.md) feature index; [session-history.md](session-history.md) HIST-18, HIST-04.

2. **DeepSeek Harness as an agent.** Is DeepSeek Harness a first-version agent (CUS-03), or a later adapter?
   - Why it matters: it would add one agent to the set in item 1.
   - Options: first version; later adapter.
   - Source: [customisation.md](customisation.md) "Open decisions" 12. Related: item 1.

3. **Structured chat in the first version.** The Conference room and the model picker in every chat composer are chat surfaces, so the structured chat platform (AG-15) and the Claude chat adapter (AG-17) are proposed for the first version. Both are XL, and so is the Conference room (AUTO-13). Should they be?
   - Why it matters: they are the largest items in the first-version proposal. Current bot tasks already run as structured chat sessions.
   - Options: confirm (proposed); give the Conference room a minimal chat of its own and keep agent chats later; read terminal agents as chats through transcript chat (AG-24, also XL).
   - Source: [agents.md](agents.md) AG-15, AG-17; [automations.md](automations.md) AUTO-13, AUTO-14.

4. **Trimming after the owner's decisions.** The owner's decisions added 31 features to the first-version proposal, which now stands at 190 of 542. Five rows (WS-41, SKL-27, CLI-14, CLI-23, CLI-32) changed to first version in the README while this revision was written. Which should be cut?
   - Why it matters: the plan names scope as the main risk.
   - Options: none stated. Candidates with the least effect on the owner's decisions: AUTO-08 (host ownership, if only one host schedules at first), AUTO-07 (completion from agent status only), ACC-15 (simple fixed refresh), SCM-22 with only the Board as source.
   - Source: [README.md](README.md) "Reading the proposal" and summary table.

5. **One schedule engine, or pv96's two.** At pv96, Routines run on their own scheduler: a host check every 30 seconds claims the due runs. The owner's direction (design open question 8, answered) is one core scheduling engine with two presentations.
   - Why it matters: two schedulers mean two run histories, two missed-run rules and two places where a task can fire. This sets the cost of AUTO-03 and AUTO-28.
   - Options: build one engine, as the owner's direction says (proposed here); keep the two schedulers that pv96 has.
   - Source: [automations.md](automations.md) AUTO-28 and AUTO-03; [screen-inventory.md](../design/screen-inventory.md) open question 8.

6. **Lead bot, or pv96 routing.** pv96 has no lead bot. One host query classifies each Conference room request as a bot task, a composite task, a task for another team, no team, or a conversation. The owner's direction says a composite requirement goes to a lead bot, which delegates the parts by competence.
   - Why it matters: the lead bot decides how a composite requirement is split and who receives each part (AUTO-20). Adopting pv96 routing removes that step.
   - Options: keep the lead bot (owner direction, design open question 6); adopt pv96 routing (one host query, no lead role).
   - Source: [bot-teams.md](bot-teams.md) (requests are classified by one host query); [automations.md](automations.md) AUTO-13, AUTO-20.

7. **Prototypes as core.** **Owner-confirmed 2026-11-27** (design open question 16; pending-owner-review item 12): prototypes is core, with its entry in the Workspaces Sidebar. This reverses the earlier default, which made it an extension. The first version carries only PROTO-01, PROTO-02, PROTO-04 and PROTO-05, at the lowest priority; PROTO-03 and PROTO-06 to PROTO-14 stay later.
   - Why it matters: it decides whether the first-version prototype features (4 of 13 in the prototypes domain) are built into the core or shipped as an extension.
   - Options: core with the Sidebar entry (the decision taken); extension that contributes the Sidebar entry.
   - Source: [prototypes.md](prototypes.md); WS-41 (first version).

8. **Security rules for the vault and credentials.** pv96's Marketplace vault seals OpenMetadata, Engram Cloud, Azure DevOps and Gitea tokens on the host. Local terminal spawns (not SSH or WSL) receive the OpenMetadata and Engram Cloud tokens as environment variables, with no per-agent filter, so every local terminal gets them. Separately, on Windows the desktop app may publish a credential file before its owner-only ACL is applied (ACC-21). What rule does the new core follow?
   - Why it matters: a terminal that does not need a token can still read it. On Windows, a credential file can be readable for a short time before it is locked down. The naming and placement rules (design open questions 10 to 12) are answered; this item covers delivery and storage only.
   - Options (proposed here, not in the source): deliver each token only to the agents its scope names; or keep environment delivery for every local terminal, as pv96 does. For Windows: apply the owner-only ACL before the file is published, as orcad and the CLI already do.
   - Source: [settings-and-files.md](settings-and-files.md) SET-29; [customisation.md](customisation.md) CUS-11; [accounts-and-usage.md](accounts-and-usage.md) ACC-21 (first version).

9. **Linux secret store.** Which Linux secret store should the Rust core use for credentials? On some tiling compositors Chromium falls back silently to plaintext, so this choice is part of credential storage.
   - Why it matters: the rule in item 8 needs a store that holds credentials on Linux. Without one, the fallback may be plaintext.
   - Options: none stated.
   - Source: [app-platform.md](app-platform.md) PLAT-25. Related: item 8.

10. **Phone permission class.** At pv96 the phone allowlist grew from about 320 to 373 method names: 53 added, none removed (29 `botTeams`, 8 `marketplace`, 7 `prototypes`, 4 `delegation`, 2 `agentSessionChecklist`, 2 `files`, 1 `repo.initGit`). Phones are checked against the allowlist alone, and the permission classes are not consulted for phones. Should phones get a narrower permission class?
   - Why it matters: the allowlist includes the pairing-admin methods (item 11), several admin and settings-write methods, and file methods. Device scopes (SRV-09) are proposed for later, so the allowlist stays the only check unless this changes.
   - Options (proposed here, not in the source): keep the allowlist alone, as pv96 does; or a phone class that leaves out admin and file methods, checked together with the permission classes.
   - Source: [server-and-pairing.md](server-and-pairing.md) SRV-09 and its phone allowlist note; [protocol-surface.md](protocol-surface.md).

11. **Phone access to pairing-admin methods.** Should phones be allowed to call the pairing-admin methods (`pairing.getEndpoints`, `pairing.provisionRelay`) and several accounts-admin, host-admin and settings-write methods? Code comments say pairing admin is refused unless granted to runtime clients, but phones are checked only against the allowlist. Decide together with item 10.
   - Why it matters: the code comments and the allowlist disagree.
   - Options: none stated.
   - Source: [server-and-pairing.md](server-and-pairing.md) "Findings to review" 2 (SRV-09).

12. **TLS and auth hardening.** What hardening bar must the network listener and the agent-status ingest meet in the first version?
   - Why it matters: the network listener is plain `ws://` today, protected only by the mechanism in SRV-11. Phones are checked against an allowlist that includes two pairing-admin methods. The status path accepts status JSON from pane output without a token check. These decide transport security and what a paired phone may do. RPC bearer tokens are compared with plain equality; that is a defect to fix, not a decision.
   - Options: not stated in the inventory. The server doc notes that an operator can advertise a `wss://` or `https://` address behind a reverse proxy with its own certificate.
   - Source: [server-and-pairing.md](server-and-pairing.md) SRV-12, SRV-09 and "Findings to review" 1 to 3; [agent-status.md](agent-status.md) STAT-09. Related: item 11; other item 4.

13. **Listen address for web address triggers.** pv96's web address triggers listen on 127.0.0.1 only, on port 47391 or the next free port up to 47395. Each address has one key. Should they stay local only, or be reachable from other machines with authentication?
   - Why it matters: a network entry point into the core needs its authentication rules designed first (item 14).
   - Options: local only, as pv96 does; reachable from other machines, with authentication designed first.
   - Source: [automations.md](automations.md) AUTO-29.

14. **Web address triggers in the first version.** Do web address triggers (AUTO-29), shown on the Routines page, belong in the first version?
   - Why it matters: they open a network entry point into the core, whose form and security rules are not designed.
   - Options: later (proposed); first version, with authentication rules designed first.
   - Source: [automations.md](automations.md) AUTO-29. Related: item 13.

15. **One host daemon for SSH hosts, or relay only.** Should the rebuild run one host daemon (the Rust server) for both the relay and the managed-server roles?
   - Why it matters: if the rebuild has no compatibility requirement, the relay-to-server conversion path and most of the migration machinery can be dropped.
   - Options: one daemon for relay and managed server; relay only.
   - Source: [remote-hosts.md](remote-hosts.md) REM-18. Related: other items 11 (pane traffic) and 16 (provider calls).

16. **Scope of multi-account switching.** Should the first version keep Claude and Codex multi-account switching, or use one active account per agent?
   - Why it matters: sets the size of the account migration (ACC-24).
   - Options: keep multi-account switching; one active account per agent.
   - Source: [accounts-and-usage.md](accounts-and-usage.md) "Siguiente paso". Related: other items 15 (imported sessions), 17 (GitHub login), 41 (profile switcher), 44 and 45 (session history).

17. **Extension web views on desktop (confirm).** **Decided on the owner's behalf, pending review** (pending-owner-review item 1): extensions may show a fully custom UI as a sandboxed web view, only as a Work panel tab or a full-canvas page, and only on desktop. Everywhere else they use the shared components. The owner must confirm.
   - Why it matters: this lets users build their own applications inside MassOrca. It adds a second web surface, depends on the browser engine choice (see the research notes), and does not exist on Android.
   - Options: web views for extensions on desktop (the decision taken); declarative components only in the first version, with a smaller security surface.
   - Source: [customisation.md](customisation.md) "Open decisions" 4 and Part C.6; pending-owner-review item 1.

18. **Extension model.** What is a plugin, in which language is it written, and how is it sandboxed and distributed?
   - Why it matters: shapes the core extension points, the protocol and every customisation feature. PLAN.md lists it as a Phase 0 research item.
   - Options: see items 19 to 22 here and other items 23 to 31, which list the options from customisation.md Part C.
   - Source: [customisation.md](customisation.md) "Open decisions" and Part C; [PLAN.md](../PLAN.md).

19. **Third-party code in the first version.** Must the first version run third-party code at all, or only declarative contributions and MCP servers?
   - Why it matters: decides whether a runtime and a sandbox are needed in v1.
   - Options: third-party code in v1; declarative contributions and MCP servers only.
   - Source: [customisation.md](customisation.md) "Open decisions" 2.

20. **Plugin runtime for the first version.** Which runtime, or which combination, supports third-party code in the first version?
   - Why it matters: decides isolation, authoring languages and effort for third-party code.
   - Options (Part C.4): A WebAssembly components (WIT, WASI) on Wasmtime; B Extism-style WebAssembly modules; C embedded Lua or Rhai; D embedded JavaScript; E out-of-process plugins over the core protocol; F MCP servers for tools; G declarative manifests only; H the current model (trusted Node worker with sandboxed iframe panels).
   - Source: [customisation.md](customisation.md) "Open decisions" 1; Part C.4 and C.9.

21. **Sandboxing.** Is an OS-level sandbox for out-of-process plugins required for v1, or is consent with disclosure enough?
   - Why it matters: the current plugin worker has full access to the machine, and the consent copy says so. The capability model limits the API, not the process.
   - Options: OS-level sandbox (Part C.5, option 3); consent with disclosure and no sandbox (option 4).
   - Source: [customisation.md](customisation.md) "Open decisions" 3; Part C.2 and C.5. Related: items 19 and 20.

22. **Extension points in Phase 1.** Which core seams are stable extension points from Phase 1 (CUS-08), and which features use them, for example CUS-01 and CUS-04?
   - Why it matters: PLAN.md requires stable extension points from Phase 1, so the list must be fixed before the core is built.
   - Options: none stated.
   - Source: [customisation.md](customisation.md) "Open decisions" 8; [PLAN.md](../PLAN.md) "Product decisions".

23. **Agent browser automation in the first version.** Is agent browser automation (about 80 command keys, BRW-13, XL, later) needed in the first version?
   - Why it matters: it is the largest feature in the domain, and it is needed first only if agents must verify web UIs.
   - Options: first version (only if agents must verify web UIs); later (the proposal).
   - Source: [browser-and-devices.md](browser-and-devices.md) BRW-13.

24. **WSL on Windows.** Is WSL a first-version feature on Windows (REM-20, L, later)?
   - Why it matters: the first-version scope does not settle whether Windows users get WSL execution. Related: item 25.
   - Options: none stated.
   - Source: [remote-hosts.md](remote-hosts.md) REM-20.

25. **WSL as a per-project runtime.** Should WSL be a supported per-project runtime (WS-28, L, later), or is the first version Windows-host only?
   - Why it matters: supporting two runtimes per project is a large cross-cutting change, and real Windows users rely on WSL.
   - Options: Windows host only in the first version, WSL later (proposed); per-project Windows or WSL runtime later.
   - Source: [workspaces.md](workspaces.md) WS-28. Related: item 24.

26. **Degraded plain-SSH mode.** Should a degraded plain-SSH mode be kept? It has no relay runtime, terminals end on disconnect, and only SFTP is available.
   - Why it matters: it is the only path for hosts with no usable Node runtime. Dropping it leaves those hosts with no terminals that survive a disconnect and no file browsing, because SFTP is not planned.
   - Options: keep as a degraded mode; drop (the feature is currently marked drop).
   - Source: [remote-hosts.md](remote-hosts.md) REM-09.

27. **Child placement rules meet Bot Teams and folder workspaces.** The owner's child placement rules (2026-11-12) say two sessions never write in the same worktree at once, and parallel children always get their own worktree. Two earlier owner directions meet them: a lead bot delegates the parts of a composite requirement (AUTO-13, AUTO-20), and a coordinator launches children "sequential or parallel" (AUTO-17). In a plain folder, including a Bot Teams team folder that is not a git repository, a child cannot get its own worktree, so writing children there can only run one at a time. Not resolved here.
   - Why it matters: decides whether Bot Teams can work on several parts at once, and whether team folders must be git repositories.
   - Options: accept sequential writing in plain folders and team folders; make team folders git repositories so each bot child gets a worktree; give a writing child in a plain folder a separate copy of the folder (costs disk and needs a way back).
   - Source: [agents.md](agents.md) AG-26; [automations.md](automations.md) AUTO-13, AUTO-17, AUTO-20; [bot-teams.md](bot-teams.md).

## Other decisions, by impact

Ordered by the same judgement as the blocking section. Each item keeps its domain label in bold.

1. **Git: client-supplied command templates.** Should clients be allowed to supply a custom command template and an agent command override in a commit-message generation request?
   - Why it matters: when these are present, the runtime uses them without re-resolving the user's settings, so a client could choose the executed command. This is a security question.
   - Options: none stated; the doc calls it an open decision.
   - Source: [git.md](git.md) GIT-10.

2. **Agent status: plain-text status content.** Should prompts, tool input previews and last assistant messages stay in plain text in the status file, kept for up to seven days?
   - Why it matters: this content sits unencrypted in the app data folder, and no runtime expiry was found.
   - Options: none stated.
   - Source: [agent-status.md](agent-status.md) "Main risks for the owner", item 2 (STAT-13).

3. **Agent status: managed hooks at every launch.** Should managed hooks keep being installed at every launch into user-global agent configurations, where every agent session on the machine runs them?
   - Why it matters: the app rewrites files that the user's agents own, and hand edits in managed areas can be replaced. STAT-04 is first version (L).
   - Options: none stated.
   - Source: [agent-status.md](agent-status.md) "Main risks for the owner", item 1 (STAT-04, STAT-06).

4. **Agent status: trust of the OSC 9999 status path.** Should the OSC 9999 status path be trusted as a channel, given that it accepts status JSON from pane output without a token check?
   - Why it matters: anything printed to a pane can set that pane's status. The doc says to confirm this before relying on the path as secure.
   - Options: none stated.
   - Source: [agent-status.md](agent-status.md) "Main risks for the owner", item 6 (STAT-09). Related: blocking item 12.

5. **Agent status: copying hook approvals into the user's Codex trust.** Should user hook approvals keep being copied from the managed Codex home back into the user's own Codex trust configuration?
   - Why it matters: this changes what the user's own Codex trusts. The app never writes trust for its own hook.
   - Options: none stated.
   - Source: [agent-status.md](agent-status.md) "Main risks for the owner", item 5 (STAT-06).

6. **Agent status: fail-open hooks.** Should a broken hook keep failing open, so that its status silently stops?
   - Why it matters: the agent is never blocked, but the user gets no sign that status has stopped.
   - Options: none stated.
   - Source: [agent-status.md](agent-status.md) "Main risks for the owner", item 3 (STAT-02).

7. **Agent status: status on SSH link loss.** Should a dropped SSH link keep each host's last status (current behaviour), or clear that host's rows?
   - Why it matters: a stale busy row can stay until it is marked stale or replaced by a replay.
   - Options: keep rows until replay, expiry or pane teardown (current); clear host rows on loss (an existing function has no production caller).
   - Source: [agent-status.md](agent-status.md) "Main risks for the owner", item 4 (STAT-15, STAT-17).

8. **Agent status: owner of repository setup and archive hooks.** Which domain should own repository setup and archive hooks (`orca.yaml`), which are not agent hooks?
   - Why it matters: the name collides with agent hooks, and the archive gate was not read.
   - Options: none stated; the doc says to decide in the workspace domain.
   - Source: [agent-status.md](agent-status.md) STAT-25.

9. **Feedback and crash reports as public GitHub issues.** Feedback and crash reports now create public GitHub issues through the user's `gh` account. Keep this, drop it, or route reports elsewhere?
   - Why it matters: each report becomes a public issue under the user's login, with the report content. SCM-32 says the new product needs its own feedback channel (owner to confirm).
   - Options (proposed here, not in the source): keep the GitHub route; drop the feature (SCM-32 is already marked drop); route to a product-owned feedback channel.
   - Source: [app-platform.md](app-platform.md) PLAT-16 (later); [cloud.md](cloud.md) CLD-17 (later); [providers-and-trackers.md](providers-and-trackers.md) SCM-32 (drop).

10. **Cloud dictation consent.** Should cloud dictation show a consent notice before audio is sent to the OpenAI transcription endpoint?
   - Why it matters: no consent notice was found in the files read, and user audio leaves the device.
   - Options: none stated.
   - Source: [browser-and-devices.md](browser-and-devices.md) BRW-30.

11. **Browser: pane traffic through SSH and WSL hosts.** Keep the SOCKS-proxied pane traffic design, or replace it with plain port forwarding decided together with BRW-07?
   - Why it matters: decides how remote dev servers and intranet sites are reached. It is coupled to the localhost preview design (BRW-07, first version).
   - Options: keep this design; plain port forwarding (the doc says it may replace this design).
   - Source: [browser-and-devices.md](browser-and-devices.md) BRW-19 (XL, later). Related: blocking item 15.

12. **Browser: first-version split.** Accept the proposal: first version is BRW-01, BRW-02 and BRW-07; drop BRW-04 and BRW-20; all other features later?
   - Why it matters: sets the first-version scope. The BRW-04 setting exists only because Electron leaks markers. BRW-20 should be replaced, not ported.
   - Options: accept as proposed; for BRW-04, drop only the user setting and let the engine set one identity policy internally; for BRW-20, reopen only if headless automation needs pages without a desktop client.
   - Source: [browser-and-devices.md](browser-and-devices.md) "Proposal summary"; BRW-04; BRW-20.

13. **Browser: profiles in v1.** Should the first version include only the default browser profile?
   - Why it matters: BRW-03 says the first version needs only the default profile, but the proposal summary places BRW-03 in Later. BRW-02 depends on profile inheritance, so the doc contradicts itself.
   - Options: default profile only in v1; full profiles later (the summary).
   - Source: [browser-and-devices.md](browser-and-devices.md) BRW-03; "Proposal summary".

14. **Browser: certificate handling.** Should first-version certificate handling only fail closed with a warning and offer no trust decisions, with passkeys later?
   - Why it matters: BRW-18 is labelled first version, but the proposal summary places it in Later.
   - Options: fail closed with no trust decisions (first version); per-endpoint trust for the session (reference behaviour).
   - Source: [browser-and-devices.md](browser-and-devices.md) BRW-18. Related: blocking item 12.

15. **Session history: deleting imported sessions.** Must imported sessions be deletable from MassOrca without touching the source file?
   - Why it matters: deletion today is limited to 14 agents and removes source files, so an imported copy needs its own delete rule. The import itself is answered (copy into MassOrca's store).
   - Options: none stated.
   - Source: [session-history.md](session-history.md) HIST-18.

16. **Providers: host for provider calls.** Which host should run provider calls for SSH and remote repositories: the core's host, where the repository lives, or the desktop?
   - Why it matters: the app splits this today (`gh` runs on the desktop, `glab` on the remote), so auth scope and behaviour differ by provider.
   - Options: core host (where the repository lives); desktop.
   - Source: [providers-and-trackers.md](providers-and-trackers.md) "Open decisions for the owner" (SCM-29). Related: blocking item 15.

17. **Providers: GitHub login.** Does the first version need one GitHub login, or per-repository account binding?
   - Why it matters: binding adds per-host, per-project account state and gh version gating.
   - Options: one GitHub login; per-repository account binding.
   - Source: [providers-and-trackers.md](providers-and-trackers.md) SCM-02. Related: blocking item 16.

18. **Providers: plaintext token fallback for Jira and Linear.** Should Jira and Linear keep the plaintext token fallback when the OS secret store is unavailable?
   - Why it matters: tokens are then written in plaintext, with only a logged warning.
   - Options: keep the fallback with a warning (current behaviour); no alternative stated.
   - Source: [providers-and-trackers.md](providers-and-trackers.md) SCM-05, SCM-06. Related: blocking items 8 and 9.

19. **Tracker sources on the Tasks page.** The Tasks page on the rail is one list with a source filter (SCM-22, first version, with team Boards as the first source). Should GitHub and GitLab issues and PRs (SCM-18, SCM-19) join it in the first version?
   - Why it matters: the owner's Tasks page lists issues and PRs from connected providers; the inventory keeps tracker browsing later.
   - Options: Boards only first (proposed); add GitHub issues and PRs; add all trackers.
   - Source: [providers-and-trackers.md](providers-and-trackers.md) SCM-18 to SCM-22.

20. **Marketplace: catalog content in v1.** Placement is answered (see the answered section). Open: what the Marketplace page shows in v1. At pv96 it is a catalog of tools, agents, skills and services. The page itself (SKL-27) is first version (L); the plugin system and marketplace sources (SKL-18 to SKL-22) are later.
   - Why it matters: the page needs something behind it. Showing the full pv96 catalog depends on the plugin items, which are later.
   - Options: hide the icon until extensions exist; show the skills catalog first; bring a minimal extension browser forward; show the pv96 catalog of tools, agents, skills and services (SKL-27).
   - Source: [skills-and-plugins.md](skills-and-plugins.md) SKL-18 to SKL-22, SKL-27; [customisation.md](customisation.md).

21. **Cloud sign-in state.** Should cloud sign-in stay dev-only, as the settings copy says, or be live, given that production endpoints are compiled in?
   - Why it matters: the settings copy and the build contradict each other. The doc has not established from the repository whether a production build completes sign-in.
   - Options: none stated.
   - Source: [cloud.md](cloud.md) CLD-04; [app-platform.md](app-platform.md) "Observations for the owner".

22. **Cloud accounts.** Does the owner want cloud accounts?
   - Why it matters: accounts are currently out of the plan. A revisit would reopen the Drop status and the private auth service dependency, and the doc ties any revisit to keeping the relay.
   - Options: none stated.
   - Source: [cloud.md](cloud.md) CLD-04.

23. **Plugin UI on React Native.** Which model should plugin UI use on Android: declarative native views (R1), WebView panels (R2), plugin JavaScript on the device (R3), or none in v1?
   - Why it matters: Google Play forbids downloading executable code from outside Play. It allows JavaScript in a WebView, and it governs interpreted code loaded at runtime. R3 needs a policy read, and whether that applies to our update path is unverified. R2 needs a strict CSP and no bridge beyond a fixed message set.
   - Options: R1; R2; R3; none in v1.
   - Source: [customisation.md](customisation.md) "Open decisions" 5; Part C.1 and C.6.

24. **Plugin distribution.** Which distribution model should plugins use: a curated registry with review, pinned to commits (D1); git URLs pinned to commits (D2); or both?
   - Why it matters: sets the trust point and how plugins are discovered. Part C.7 also lists a package registry (D3).
   - Options: D1; D2; both; D3 is also listed.
   - Source: [customisation.md](customisation.md) "Open decisions" 6; Part C.7.

25. **Plugin signing.** Are signed plugin artifacts with verification (D4) required in v1, or later?
   - Why it matters: adds key management. No source reviewed covers the tooling.
   - Options: required in v1; later.
   - Source: [customisation.md](customisation.md) "Open decisions" 7; Part C.7.

26. **Plugin manifest format.** Should the current manifest (`orca-plugin.json`, `publisher.id`) be kept, or should a new format be defined in the new protocol?
   - Why it matters: the manifest is the contract that authors, the lockfile and the consent depend on.
   - Options: keep the current format; define a new format.
   - Source: [customisation.md](customisation.md) "Open decisions" 9.

27. **Plugin API freeze.** When does the plugin API freeze, and what compatibility promise is made to plugin authors?
   - Why it matters: the API is experimental and off by default today. Zed publishes a compatibility table between releases and API versions.
   - Options: none stated.
   - Source: [customisation.md](customisation.md) "Open decisions" 10; Part C.8.

28. **Creator mode.** Is an agent-driven Creator mode, in the style of DeepSeek Harness, wanted, and in which language should generated plugins be written?
   - Why it matters: the answer changes the authoring language and the sandbox needs.
   - Options: none stated.
   - Source: [customisation.md](customisation.md) "Open decisions" 11.

29. **Our own features on extension points.** Must our own features be built on the same extension points, or only where practical?
   - Why it matters: the answer decides whether first-party features exercise the plugin contract. PLAN.md says "where practical".
   - Options: none stated.
   - Source: [PLAN.md](../PLAN.md) "Product decisions"; customisation.md Part C.1.

30. **Plugin kill list trust policy.** What trust policy should the new product own for the plugin kill list?
   - Why it matters: the list is fetched from a fixed vendor address, and the doc says the new product must own this trust policy.
   - Options: none stated.
   - Source: [skills-and-plugins.md](skills-and-plugins.md) SKL-23. Related: blocking item 18.

31. **Owner of agent-state rules and quick commands.** Who owns CUS-05 (agent-state rules) and CUS-07 (quick commands) once the agents and terminals domains are written?
   - Why it matters: determines which domain must carry them.
   - Options: none stated.
   - Source: [customisation.md](customisation.md) "Open decisions" 13.

32. **Legacy coordinator loop.** Should the legacy coordinator loop (the `orchestration.run` and `runStop` RPCs) be dropped?
   - Why it matters: it holds the largest share of coordinator state rules. The Run, Task and worker commands replace it. The Run concept itself is first version (answered). Whether any UI still calls the loop was not verified.
   - Options: drop (proposed); keep (not stated).
   - Source: [automations.md](automations.md) AUTO-15 and AUTO-24.

33. **Hermes and OpenClaw cron managers.** Should these cron managers be dropped?
   - Why it matters: MassOrca does not own these jobs. Their only value is importing or migrating them, which the doc calls a separate decision.
   - Options: drop (proposed); import or migrate (a separate decision, not scoped).
   - Source: [automations.md](automations.md) AUTO-11.

34. **Team memory.** What does team memory (AUTO-31) hold, and do bots write to it?
   - Why it matters: the owner's reference shows only the entry.
   - Options: none stated.
   - Source: [automations.md](automations.md) AUTO-31; design row D-AG-26.

35. **Jev routing service (AG-33).** Should the optional third-party decision service, which picks each delegated task's tier from a one-line summary, stay as an option, become an extension, or be dropped?
   - Why it matters: it is off by default, but when on it sends a one-line summary of each delegated task to a third-party API. AUTO-20 (first version) already names an optional Jev advisor for automatic routing, so the two overlap. Jev is pv96's internal routing name, which pending-owner-review item 15 keeps out of UI copy.
   - Options (proposed here, not in the source): keep it as an option; make it an extension (the README marks it a plugin candidate); drop it.
   - Source: [agents.md](agents.md) AG-33; [automations.md](automations.md) AUTO-20.

36. **Checklist card in agent chats (AG-32 and WS-36).** The same checklist card appears twice: AG-32 in the agent domain and WS-36 in the workspaces domain. Both are later. The owner asked for it for QA tracking (checklist-card, 2026-11-02). Promote it, or keep it later?
   - Why it matters: it is M in size. The checklist tab (WS-37) depends on it. Note that AG-32 is the checklist, not the bring-back feature.
   - Options: keep later (proposed, count once); promote to the first version (the owner asked for it).
   - Source: [agents.md](agents.md) AG-32; [workspaces.md](workspaces.md) WS-36, WS-37.

37. **Bring-back of child work (AG-31).** AG-31 is later, but AUTO-32 and GIT-30 cover the bring-back in the first version. Confirm that AG-31 stays later and is counted once.
   - Why it matters: avoids two bring-back flows. The agents doc already says "count once".
   - Options: keep AG-31 later and count it once, as the agents doc says (proposed); promote AG-31 as a separate feature (not proposed).
   - Source: [agents.md](agents.md) AG-31; [automations.md](automations.md) AUTO-32; [git.md](git.md) GIT-30.

38. **Terminal history on disk and cold restore.** Should disk terminal history and cold restore ship in the first version, or later?
   - Why it matters: without them, a host crash or machine reboot ends running terminals and loses their screens and scrollback.
   - Options: first version; later (the proposal, which accepts the loss in those cases).
   - Source: [terminals.md](terminals.md) TERM-08 and TERM-09.

39. **Host-survival hardening.** Should the Linux systemd scope and the macOS login-session watch stay in the first version, or move later?
   - Why it matters: they are host-survival hardening, and moving them later changes how the host survives session loss.
   - Options: keep in the first version (the proposal); move later.
   - Source: [terminals.md](terminals.md) TERM-18.

40. **Generated-name retirement.** Should generated-name retirement start as simple unique suffixing and be tightened later, or be tightened from the first version?
   - Why it matters: without retirement, generated names are reissued. Agent CLIs key conversation state by path, so old history can reach a new workspace.
   - Options: simple unique suffixing now, tightened later (proposed); tighter retirement now.
   - Source: [workspaces.md](workspaces.md) WS-06.

41. **Profile switcher.** Should the profile switcher and project transfer between profiles be dropped, as proposed, pending the owner's confirmation?
   - Why it matters: the switcher is gated off, and transfer only matters if profiles stay. Related: blocking item 16.
   - Options: drop (proposed); keep profiles and the switcher.
   - Source: [workspaces.md](workspaces.md) WS-30.

42. **Worktree checkout pool.** Should the pre-warmed worktree checkout pool be dropped, as proposed, or carried over?
   - Why it matters: it is a latency optimisation of the current design. The doc asks for creation latency to be measured before any carry-over.
   - Options: drop (proposed); carry over only after measuring creation latency.
   - Source: [git.md](git.md) GIT-17.

43. **First-version scope for Git.** Confirm the Git features proposed for the first version. The README summary table now marks 13 Git features (this item said twelve), so confirm the list against the table.
   - Why it matters: the first-version marks are in the README summary table, so the scope can be checked there.
   - Options: none stated.
   - Source: [git.md](git.md) "How to read the proposals"; [README.md](README.md) summary table.

44. **Cursor login migration.** Is Cursor login migration in scope?
   - Why it matters: the doc names it as candidate input for the "migrate agent accounts and subscriptions" requirement.
   - Options: none stated.
   - Source: [session-history.md](session-history.md) HIST-13. Related: blocking item 16.

45. **Claude account history.** Is Claude account history sharing part of account migration?
   - Why it matters: history linking and moving is tied to account switching, so the decision must be made together with the accounts domain.
   - Options: none stated.
   - Source: [session-history.md](session-history.md) HIST-12. Related: blocking item 16.

46. **Mobile web bundle over RPC.** Should the mobile web bundle served over RPC be dropped from the first version?
   - Why it matters: the PLAN says the new Android client is React Native with a Rust core, so a desktop-served bundle is an artifact of the current shell.
   - Options: drop from the first version (the doc recommends this); keep (implied, not stated).
   - Source: [server-and-pairing.md](server-and-pairing.md) SRV-19.

47. **Distribution model.** Is the manual-only distribution marker, which names the current repository, still the current distribution model?
   - Why it matters: builds made with the marker never auto-update, so the update policy depends on the answer.
   - Options: none stated.
   - Source: [app-platform.md](app-platform.md) "Observations for the owner" (PLAT-06).

48. **Channel model.** Which channel model should the rebuilt product use instead of the Electron dev channels?
   - Why it matters: the rebuilt product must choose its own channel model.
   - Options: none stated.
   - Source: [app-platform.md](app-platform.md) PLAT-05.

## Research notes

The browser engine research is unchanged in content. The extension-model research is in [customisation.md](customisation.md) Part C, not here; its options are listed as decisions in blocking items 18 to 22 and other items 23 to 31. Heading levels changed; text did not.

### Browser pane: embedding a web engine in a GPUI app

Research note, as of 2026-11-12. GPUI has no webview, so the browser pane needs an embedded web engine for that pane only ([PLAN.md](../PLAN.md) "Product decisions"). The engine must meet the requirements in [browser-and-devices.md](browser-and-devices.md) "Web engine requirements": items 1 to 10 are needed for the first version (BRW-01, BRW-02, BRW-07), and items 11 to 24 are needed later. Items 11 to 20 are shaped by Chromium. Requirement 1 (several views in one window) and requirement 12 (windowless rendering) are the main drivers of the options below.

#### Options and tradeoffs

| Option | Windows / macOS / Linux | Agent automation | Devtools | Size | Maturity and license | GPUI effort |
|---|---|---|---|---|---|---|
| A. System webview (wry, via `gpui-wry` or `gpui-component`) | Windows yes, macOS yes, Linux X11 only (Wayland needs a GTK container) | Three protocols: CDP on Windows, Safari inspector on macOS, WebKit inspector on Linux | Yes; macOS uses private APIs | Small (system engine) | wry is mature; GPUI wrappers are "experimental"; Apache-2.0 or MIT | Low on Windows and macOS, high on Linux. Native child view paints above GPUI |
| B. CEF via `cef-rs`, windowed | All three, x64 and ARM64 | Full CDP | Full Chromium DevTools | Large (not verified) | cef-rs Apache-2.0 or MIT; CEF is BSD; Tauri v3 CEF runtime is alpha | High; same z-order and Wayland problems as A |
| B'. CEF off-screen rendering (OSR) with GPU texture import | Same as B | Full CDP | Full Chromium DevTools | Large (not verified) | `cef` crate 156.x has an `accelerated_osr` feature for wgpu | Medium-high; input, IME, focus and HiDPI handled by the app; GPUI texture hooks differ per OS |
| C. Servo (`servo` crate) | Windows, macOS, Linux (also Android) | WebDriver only; no CDP found | Own partial DevTools server | Not verified | 0.8.0 on 2026-11-07; not 1.0; monthly breaking changes; license not confirmed (historically MPL-2.0) | Medium; web compatibility is the risk |
| D. External Chrome or Chromium with CDP (`chromiumoxide`) | All three | Full CDP | Real Chrome DevTools | None if the user's Chrome is used | Very mature; Apache-2.0 or MIT | Lowest, but not embedded: separate window, no unified tabs |

#### Analysis

- **System webviews (A)** split automation into three protocols, so a uniform "agent drives the browser" feature is hard. Native child views always sit above GPUI content, so popovers and menus over the pane are not possible.
- **CEF (B, B')** is the only option with identical Chromium behaviour, CDP and DevTools on all three systems. Windowed mode has the same child-view problem as A. Off-screen rendering removes it, at the price of owning compositing and input. No published GPUI and CEF integration was found, so this would be new work.
- **Servo (C)** has no CDP and no stable API. Revisit later.
- **External Chrome (D)** meets agent control, screenshots, devtools and localhost previews now. It fails the "integrated pane" requirement.

#### Recommendation (proposal, not decided)

1. **Now (MVP): option D.** Launch Chromium or Chrome with a remote debugging port and drive it with `chromiumoxide`. Define a `BrowserBackend` trait whose surface is CDP.
2. **Embedded pane: option B' (CEF OSR),** starting on Linux and Windows, with a CPU `on_paint` fallback on macOS until a texture hook exists. The CDP agent layer from step 1 is reused, since CEF is Chromium.
3. **Optional shortcut: wry through `gpui-wry` on Windows only**, because WebView2 has CDP. Do not use wry as the primary path on macOS or Linux.
4. **Skip Servo for now.** Track it.

**Owner decision:** accept this path, or choose another option from the table. Why it matters: the browser pane is in product scope, and the engine is a Phase 0 research item that shapes the pane design.

#### Caveats (unverified claims)

- Binary sizes were not verified from a primary page.
- CEF's `remote_debugging_port` behaviour was not verified. The CEF wiki fetch failed.
- Servo's license was not confirmed from a primary page.
- The claim that Win32 `SetParent` cannot work on macOS or Wayland is an inference; no source was checked.
- The status of Zed issues is as of the research date.

#### Sources

- wry README: https://github.com/tauri-apps/wry
- wry docs: https://docs.rs/wry/latest/
- wry discussion on CEF, WebKitGTK and Servo: https://github.com/tauri-apps/wry/discussions/1014
- gpui-wry 0.5.0 README: https://docs.rs/crate/gpui-wry/0.5.0/source/README.md
- gpui-wry / gpui-kit webview 0.7.3: https://lib.rs/crates/gpui-wry
- gpui-component webview.rs 0.5.1: https://docs.rs/crate/gpui-component/0.5.1/source/src/webview.rs
- Zed issue #21208, "Webview via Extensions": https://github.com/zed-industries/zed/issues/21208
- Zed issue search for "webview": https://github.com/zed-industries/zed/issues?q=is%3Aissue+webview
- Zed wgpu Linux renderer commit: https://git.secluded.site/zed/commit/af8ea0d6c26192c45f44f473c0d4a7d6f72ed018
- kael 0.2.0: https://docs.rs/crate/kael/0.2.0
- cef-rs repository: https://github.com/tauri-apps/cef-rs
- cef crate OSR texture import (v156.4.0): https://docs.rs/crate/cef/latest/source/src/osr_texture_import/mod.rs
- Tauri v3 alpha with `tauri-runtime-cef`: https://newreleases.io/project/github/tauri-apps/tauri/release/tauri-v3.0.0-alpha.0
- Servo 0.1.0 announcement: https://servo.org/blog/2026/04/13/servo-0.1.0-release/
- Servo crate page (0.8.0): https://docs.rs/servo/latest/servo/
- Servo monthly posts: https://servo.org/blog/2026/04/30/march-in-servo , https://servo.org/blog/2026/05/31/april-in-servo/
- Tauri Verso experiment: https://v2.tauri.app/blog/tauri-verso-integration/
- WebView2 CDP method: https://learn.microsoft.com/dotnet/api/microsoft.web.webview2.core.corewebview2.calldevtoolsprotocolmethodasync
- Playwright on WebView2: https://playwright.dev/docs/webview2
- WKWebView `isInspectable`: https://developer.apple.com/documentation/webkit/wkwebview/isinspectable
- WebKitGTK and WPE remote inspector: https://trac.webkit.org/wiki/RemoteInspectorGTKandWPE
- chromiumoxide: https://github.com/mattsse/chromiumoxide


## Answered decisions (for reference)

Design open questions (`docs/design/screen-inventory.md`, "Open questions for the owner"):

- **Account migration (design Q1).** **Answered (owner accepted default 2026-11-12):** copy the stored credentials and check each one on import (A1). Whether the new core can decrypt the safeStorage and Keychain items is not verified, so the migration size is M or L.
- **Session import (design Q2).** **Answered (owner accepted default 2026-11-12):** copy sessions into MassOrca's store; the source is only read (A2). Deleting imported sessions is open (other item 15).
- **Android OTA web shell (design Q3).** **Answered (owner accepted default 2026-11-12):** not kept; every Android screen is native (A3).
- **Extensions in the core (design Q4).** **Answered (owner accepted default 2026-11-12):** the workspace board, GitHub Projects and the notebook viewer are extensions (A4). Prototypes changed later; see design Q16.
- **Relay pairing (design Q5).** **Answered (owner accepted default 2026-11-12):** not in the first version; only local network and SSH (D-PR-04 dropped). The cloud items CLD-01 to CLD-03 stay later.
- **Bot Teams tasks (design Q6).** **Answered (owner, 2026-11-12):** tasks come from the Conference room (D-AG-24). A composite requirement goes to the lead bot. Whether pv96 routing replaces the lead bot is open (blocking item 6).
- **Orchestration (design Q7).** **Answered (owner, 2026-11-12):** orchestration is in the first version, as one delegation engine (X-DL-01) for Bot Teams and Workspaces.
- **Routines and automations (design Q8).** **Answered (owner, 2026-11-12):** one core scheduling engine with two presentations. The calendar shows both on one timeline with a mode filter. Whether pv96's second scheduler is retired is open (blocking item 5).
- **Old 'vault' tab (design Q9).** **Answered (owner, 2026-11-12):** merged into the Sessions list of the Project sidebar (D-SH-04). It keeps no row of its own.
- **Linked secret names (design Q10).** **Answered (owner accepted default 2026-11-12):** no; a linked secret keeps the same variable name in every scope (A29). The delivery rule is open (blocking item 8).
- **Vault location (design Q11).** **Answered (owner accepted default 2026-11-12):** the vault opens from the Settings view, as its own section (A27).
- **Vault on Android (design Q12).** **Answered (owner accepted default 2026-11-12):** Android gets the list and add only; edit, rotate, delete and unlink stay on desktop (A36).
- **Where views open (design Q13).** **Answered (owner, 2026-11-12):** a diff opens as a Work panel file tab. Tasks, Calendar, Marketplace, Settings, the Bot Teams pages and Automations open as full pages over the canvas, with the Sidebar hidden and Back returning to the session. Artifacts items open in a Work panel tab (X-AR-01).
- **Title bar (design Q14).** **Answered: decided on the owner's behalf, pending review** (pending-owner-review item 13): the title bar stays in the frame (D-SH-03).
- **Mode switch (design Q15).** **Answered: decided on the owner's behalf, pending review** (pending-owner-review item 14): from any other page the first press returns to the current mode's home (D-SH-10).
- **Prototypes (design Q16).** **Answered: owner-confirmed 2026-11-27** (pending-owner-review item 12): prototypes is core, with its Sidebar entry (D-WS-01); see item 7 for the first-version rows.

Owner answers that open items in earlier versions of this document already settled:

- **Bot Teams stays.** **Answered (owner, 2026-11-12):** no drop. Bot Teams is a core mode, switched from the activity rail. See [bot-teams.md](bot-teams.md).
- **Timing of automations and orchestration.** **Answered (owner, 2026-11-12):** sooner. Orchestration is in the first version, and Routines and Automations are one schedule engine (design Q7 and Q8).
- **Marketplace placement.** **Answered (design Q13, owner):** the activity rail icon opens a full page over the canvas. **Answered: decided on the owner's behalf, pending review** (pending-owner-review item 10): no Marketplace entry on Android in v1. What the page shows is open (other item 20).

Pending owner review (every item is **decided on the owner's behalf, pending review**; see [pending-owner-review.md](../design/pending-owner-review.md)):

- **Item 1, extension web views on desktop.** Sandboxed web view as a Work panel tab or full-canvas page, desktop only. Confirmation: blocking item 17.
- **Item 2, typography.** Geist for the interface; JetBrains Mono for terminal, code, paths and IDs. Both open-licence and bundled.
- **Item 3, accent colour.** Calm blue (`accent.default` #2c63dc dark, #2563eb light); users may pick another.
- **Item 4, light theme.** Frame #eeeef2, canvas #fbfbfc.
- **Item 5, agent marks.** Full colour inside the usage ring on the Activity rail.
- **Item 6, calendar.** Routines and automations on one timeline, filtered by mode and team.
- **Item 7, artifacts.** No project-wide list in v1; they appear in the Session summary Results card and open in a Work panel tab.
- **Item 8, themes.** Dark and light in v1 with a theme validator; no high-contrast or colour-blind theme; status never relies on colour alone.
- **Item 9, Android colour.** Follows the system light or dark setting, with a manual override; no dynamic colour.
- **Item 10, Android navigation.** Rail entries move to the Home top bar, the overflow menu and the host card; no Marketplace entry on Android in v1.
- **Item 11, narrow window.** Regions give way in this order: Session summary, Project sidebar, Sidebar, Work panel.
- **Item 12, prototypes.** Core, with its Workspaces Sidebar entry. Confirmation: blocking item 7.
- **Item 13, title bar.** Stays as a band of the frame.
- **Item 14, mode switch.** From other pages, the first press returns to the mode's home; a press at home switches the mode.
- **Item 15, UI copy.** Glossary names (Work panel, Session summary, Project sidebar); "Jev" does not appear in UI copy.
- **Item 16, close shortcut.** `Mod+W` closes the active Work panel tab and never closes the window.
- **Item 17, Mobile entry.** The "MassOrca Mobile" Sidebar entry opens the phone pairing page on the desktop.
- **Item 18, mode homes.** Workspaces returns to the start page; Bot Teams returns to the team list; Prototypes opens as a full-canvas page.
- **Item 19, routines.** A new routine opens the simplified routine form built in pv96.
