# Prototypes and design system: functional inventory

Prefix: `PROTO`. Domain name: Prototypes and design system. Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits). Read-only reference: `D:\projects\massorca\.claude\worktrees\unified-custom-profile-build-547ea6` at that commit.

## Summary

A prototype is a self-contained HTML file that an agent chat writes in answer to a design request. MassOrca asks the agent for one or more directions (A, B, C) for each request, stores every request in a folder in the chat's visuals folder, and shows the directions in a Prototypes view on desktop and mobile. The person can open a direction, switch between desktop and phone widths, pin elements, edit their values, write notes and send one batched message back to the chat that made the set. Starred directions can be merged into a new main direction. Each project may carry a design system (tokens, components and notes in `design/system/`) that the directions are built on, and that the person can edit and send to the agent.

Most of this area is not on custom/integration in full. custom/integration already had the request folder and its listing, the count setting, the request lexicon, the desktop Prototypes page, the viewer and edit panel, a design system view, the request card and mobile list, set, viewer and draft screens. pv96 adds about 130 prototype-related source paths under `src/` (289 on pv96 against 157 on custom/integration, counted by file name, tests included). The Source line of each feature says which parts were already there.

This domain had no prototype section before this pass, so every feature below is new to the inventory. No existing section was changed and none was removed.

Note on depth: behaviour comes from the pv96 feature records (`odd/tasks/prototypes-section.md` and `odd/tasks/prototypes-design-system.md`, which list the decisions and the review history), from file names and module headers, from the host RPC method names and from the CLI spec. Renderer and mobile bodies were not read. See "Not read / low confidence".

## Owner direction for the new product (2026-11-12)

The owner settled in the design session that prototypes are an extension, not part of the core product. `docs/design/screen-inventory.md` open question 4 is recorded as answered: "the workspace board, GitHub Projects, prototypes and the notebook viewer are extensions, not core (A4)". The Prototypes page (D-AG-17) is marked Redesign and ships as an extension-provided view through slot D-SL-04; core keeps only that slot. Its chat cards (D-AG-18) merge into the chat card slot (D-SL-03), and the Android screens (M-PT-01) merge into D-AG-17. `docs/PLAN.md` names extension points for panels, commands and views as a design principle.

Consequence for this document: every feature is proposed as later, and each carries an extension role (built-in + extension point, or plugin candidate). Owner decisions win over these proposals.

**Update (2026-11-12, night):** the design session reversed the default above. Owner decision taken on the owner's behalf, pending review (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16): prototypes is a core feature, with a Prototypes entry and count in the Workspaces Sidebar, because pv96 builds it as core. This document therefore proposes PROTO-01, PROTO-02, PROTO-04 and PROTO-05 (request, request folder, list and viewer) for the first version, so the Sidebar entry has something behind it, and the rest as later. This is an assumption the owner should confirm; if the owner restores the extension default, all PROTO features go back to later.

## Source directories read

All paths are relative to the reference repository root, at commit `d3c6311a531`.

- `odd/tasks/prototypes-section.md` and `odd/tasks/prototypes-design-system.md`: feature records, approved decisions and review history (read in full).
- `src/shared/`: `prototype-*.ts` and `rpc-contract/prototype-params.ts`. Read for names, types, module headers and the count, lexicon, hidden-instruction, request-folder, set-state, merge-plan, merge-state, archive, design-system-card and delegated-ask rules. Bodies read only where named in the features.
- `src/main/native-chat/`: `prototype-requests.ts`, `prototype-request-disk.ts`, `prototype-request-deletability.ts`, `prototype-design-system-*.ts`, `prototype-set-title.ts`, `prototype-request-lifecycle.ts`. Names and headers.
- `src/main/runtime/rpc/methods/prototypes.ts`, `prototype-design-system-methods.ts`, `prototype-chat-access.ts`: method names (the RPC method table).
- `src/main/bot-teams/bot-team-session-prototype-choice.ts`, `bot-team-task-prototype.ts`; `src/shared/delegation-prototype-choices.ts`, `bot-team-room-prototype-tasks.ts`: headers.
- `src/cli/specs/prototypes.ts`: full command list, usage and notes. `src/cli/handlers/prototypes*.ts`: names only.
- `src/renderer/src/components/prototypes/`: file names and a few headers.
- `mobile/src/prototypes/`: file names; the merge and design-system screen names.
- Presence checks on custom/integration (`f23a48ca8ca`) with `git ls-tree` and `git grep`, and the `git diff --name-status custom/integration d3c6311a531` path list.
- `docs/design/screen-inventory.md` (D-AG-17, D-AG-18, D-SL-03, D-SL-04, M-PT-01, open question 4), `docs/PLAN.md` (customisation principle).
- `docs/inventory/agents.md` AG-16 (inline visuals folder) is referenced and not edited. `docs/inventory/automations.md` AUTO-25 to AUTO-27 (artifacts) are referenced.

## Not read / low confidence

Not read:
- Renderer and mobile component bodies, layout and styles.
- Most bodies in `src/main/native-chat/prototype-*` (merge plan parsing, version store, restore, disk scanning, Gentle Shell binding), and the RPC and CLI handler bodies.
- The full lexicon word lists (only the term families and the start of the negative-term list).
- Tests, except names quoted in the feature records.
- The approved design boards (external artifact links in the records were not opened).

Low confidence:
- The `/prototype` command, the card's "Not a prototype" button and the "Explicit command under Ask each time" case: the records describe the behaviour, the code was not traced end to end. The Ask-each-time case is treated here as showing the card.
- Android note screenshots: the record says a WebView capture may come back blank and was not run on a device.
- Gentle Shell slug binding (set difference at send time) is taken from the record, not traced.
- Whether stars and the main direction survive an app restart was not traced.
- A design system whose project has only a "use" request and no set opens a blank view. The record lists this as an open product question.
- Mobile Stitch chip and Stitch source are named in the records; not traced.

## Cross-cutting behaviour

Where prototype files live:
- A request is a folder in the chat's visuals folder on the host that runs the chat (`prototypes/<request id>/`). Gentle Shell's own `design/<slug>/` folder in the chat's directory is bound to a request and listed as a prototype too.
- A project's design system lives in `design/system/` inside the project root: the nearest folder at or above the chat's launch directory that holds `.git` (file or folder). Folder workspaces, which have no `.git`, use the launch directory.
- Only the host that runs the chat reads or writes these files. Clients send no paths; the host derives them from the chat record. Chats that are not local to the host (WSL, SSH relay) cannot create requests.

What the agent sees and what the person sees:
- Each prompt sent to an agent for a prototype carries a hidden instruction, opened by a marker line. It names the file contract and the count line from Settings. The chat bubble shows only what the person typed. When the agent text differs from the typed text (for example `/prototype`), the typed text is echoed in its own block.
- A marker typed in pasted text is neutralised, so user text cannot spoof a hidden instruction.

Exclusion from other lists:
- Files under `prototypes/` and bound Gentle Shell design files are prototypes. The Artifacts and Results lists leave them out.

## Features

### PROTO-01 Prototype requests from a chat

- What the user does: asks an agent in a chat for a design or a prototype in plain words, or types `/prototype` (alias `/prototipo`) in the composer, or has a parent chat hand a design to a child session (PROTO-12). When the count is not fixed by Settings, picks 1, 2 or 3 directions in a card above the composer.
- Rules:
  - Detection is local and deterministic, with no model call. Matching is on whole words, case- and accent-insensitive, English and Spanish.
  - Positive terms (prototype, mockup, maqueta, wireframe, prototyping, and their inflections) match on their own.
  - Design words (design, diseño, rediseño and their inflections) match only next to a UI noun within about ten words.
  - Single words of five or more letters tolerate one typing error.
  - Negative context (database, schema, architecture, API, design document and similar) suppresses a match, unless the same text also names a design or a UI screen.
  - `/prototype` and `/prototipo` apply the Settings count directly. On Gentle Shell, `/prototype` becomes `/gentle:design`. On Claude it sends the plain instruction. Codex and Antigravity get the plain instruction.
  - The count setting has two modes. "Always N" uses a count from 1 to 4, default 2. "Ask each time" asks on every request. The Settings line reads as one sentence, "Produce N distinct design directions, each a self-contained HTML file, with a one-line tradeoff.", with singular wording for 1.
  - A keyword-only match shows the count card even under "Always N". The card offers 1, 2 and 3, though Settings may pick 4. A fourth button, "Not a prototype", sends the message as typed.
  - An explicit command skips the card under "Always N". Under "Ask each time" the card appears.
  - The message is held until a count is picked. A concurrent send is not dropped silently.
  - Mobile follows the same rules. Its card sits docked above the composer, as on desktop. Mobile requests send the instruction without a card.
- Client requests: creates a request through `prototypes.createRequest` (chat, agent, prompt, count, design-system choice). A chat with no local record, or a remote location, is answered `unsupported`, and the message is sent as typed. A host that does not know the method is detected and the message is sent as typed.
- Persisted: the count setting (mode and count) in global settings. The request folder is PROTO-02.
- Depends on: AG-15 and AG-16 (chat send), PROTO-02, PROTO-11 (card after the count card), PROTO-12.
- Owner direction for the new product (2026-11-12): extension (A4). The composer interception needs an extension point. Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: first version.
- Source: pv96 (unmerged). The count setting, the request lexicon, the count card and `prototypes.createRequest` already exist on custom/integration. The rules above are as the pv96 records describe them; the diff of the lexicon against custom/integration was not run.

### PROTO-02 Request folder and capture contract

- What the user gets: every request is a folder that holds what the agent made. Its directions are plain files that the Prototypes view can list and open, and the agent's replies can name them.
- Rules:
  - Folder: `prototypes/<request id>/` inside the chat's visuals folder. A request id is a UTC date and time plus six random lowercase letters or digits, for example `20261028-201500-k3f9q2`.
  - Files: `request.json` (original prompt, agent, chat, count, time, design-system mode and the version it was built on, title). One HTML file per direction, named by letter (`A.html`, `B.html`). An iteration writes the next version (`B-v2.html`). `directions.json` holds each direction's short name and one-line tradeoff. `merge-plan.json` holds a merge suggestion (PROTO-08). Letters run A to Z; versions start at 2.
  - Extra direction files beyond the count are kept. A cap is a follow-up.
  - Gentle Shell `/gentle:design` keeps its own `design/<slug>/` folder. The request records the existing slugs at send time. The slug that appears afterwards in that chat's directory is bound to the request (keyed by request and session). The binding is saved on first resolution, so a later folder does not rebind it.
  - Antigravity's instruction tells it to write local HTML in the request folder and not to use its design connector for that request.
  - A path with a `.` or `..` segment is not a prototype path.
  - Only files under `prototypes/` and bound Gentle Shell files are prototypes. They are left out of the Artifacts and Results lists.
- Client requests: none directly. Reads go through PROTO-03.
- Persisted: the folder and its files on the host that runs the chat, in the chat's visuals folder.
- Depends on: AG-16 (chat visuals folder), AG-08 (chat create and send), AUTO-25 to AUTO-27 (artifacts), PROTO-03.
- Owner direction for the new product (2026-11-12): extension (A4). The visuals folder itself is core. Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: first version.
- Source: pv96 (unmerged). The request folder, `request.json`, the request disk and listing modules and the Gentle Shell binding already exist on custom/integration (`prototype-request-folder.ts`, `prototype-request-disk.ts`, `prototype-requests.ts`). pv96 adds `directions.json`, `merge-plan.json`, the Gentle Shell binding saved on first resolution, and the Artifacts exclusion, as the record describes.

### PROTO-03 Cross-chat list and owner-host reads

- What the user gets: the Prototypes view lists every prototype set on the host across all chats, grouped by project. Opening a file reads it from the host that owns the chat.
- Rules:
  - The list returns each set's title, request id, chat id, agent, archived flag and deletable flag, plus one summary per project design system.
  - Scans are capped per chat: the newest folders of each chat are read first, so one busy chat cannot starve the others. A review fix added this, tested with 600 folders in one chat.
  - The view refreshes on open, on focus and every 10 seconds (record).
  - Reads and writes go to the runtime that owns the chat, not the local host. The record says an earlier build always read the local host, which broke a paired headless runtime. This was fixed during QA.
  - Clients send no paths. The host derives the project root from the chat record.
  - A read that fails shows one line in the frame: "Couldn't open this prototype".
  - A host that does not know a method is detected and answers with a message rather than an error.
- Client requests: `prototypes.list`, `prototypes.readFile`, `prototypes.writeAttachment`, `prototypes.createRequest`, `prototypes.readDesignSystem`.
- Persisted: none new.
- Depends on: PROTO-02; remote hosts (SSH relay and paired runtimes, `remote-hosts.md` and `server-and-pairing.md`).
- Owner direction for the new product (2026-11-12): extension (A4). Owner-host routing is core RPC. Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). `prototypes.list`, `createRequest`, `readFile`, `writeAttachment` and `readDesignSystem` already exist on custom/integration. pv96 adds the per-chat scan cap, owner-runtime routing, the refresh timing and the unsupported answer for remote chats, as the record describes.

### PROTO-04 Prototypes list and set view

- What the user does: opens Prototypes from the desktop sidebar (a row below Search) or from the chat's ⋯ menu on mobile. Sees project groups. Each set row shows its title, age, agent badge and, when a design system version is recorded, "built on vN". Filters by agent: All, then one chip per agent the boards show. Opens a set to see its directions as a grid of live renders and a strip of siblings.
- Rules:
  - A single-direction request opens the viewer directly. There is no grid and no strip.
  - Archived sets are kept out of the main list and its counts, and shown in an Archived group (PROTO-09).
  - An empty list shows the title and the filters, with no text. A loading frame is blank, with no spinner.
  - List rows show no thumbnails. The grid and the strip show live renders.
  - The list groups sets by project. A project with a design system shows a Design system row (PROTO-10).
- Client requests: `prototypes.list`, and `prototypes.readFile` for each direction shown.
- Persisted: none. The open set and its selection are in memory.
- Depends on: PROTO-03, PROTO-05, PROTO-08, PROTO-09, PROTO-10.
- Owner direction for the new product (2026-11-12): extension view on slot D-SL-04 (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: first version.
- Source: pv96 (unmerged). The Prototypes page, the list panel, the side panel and the mobile list and set screens already exist on custom/integration (`PrototypesPage.tsx`, `PrototypeListPanel.tsx`, `PrototypeSidePanel.tsx`, `MobilePrototypeListScreen.tsx`, `MobilePrototypeSetScreen.tsx`). pv96 deletes the list panel and adds the sidebar tree, the set grid, the set rows, the archived group and the main-first strip.

### PROTO-05 Viewer: viewports, browse and annotate

- What the user does: opens one direction. Switches between Desktop and Phone viewports and between Browse and Annotate. In Annotate, picks an element, which gets a pin. Hides the side panel to give the prototype the full width, and brings it back by picking an element or a pin.
- Rules:
  - Annotate mode adds its script to the frame only then. In Browse the file is shown unchanged; a test pins that a non-annotate document is byte-identical.
  - A pick reports the element and its ancestors. The first pick keeps the element's original values. A pick after an edit reports the overridden values.
  - A click on an inline child climbs to the nearest button, label, link or role=button, so no deep element is dropped.
  - An element has one pin. A pin keeps only changed values. Untouched pins are dropped when the selection moves or closes. A pin can draw its outline without a badge.
  - The header is icon-only: View all, Design main, Desktop and Phone, Browse and Annotate, Merge, panel toggle. Each control has an accessible name and a tooltip. With the panel open the header stays on one line at 410 and 570 px, and the title truncates.
  - On mobile, picks from the frame pass through a host forwarder; the record says they are checked before use.
- Client requests: `prototypes.readFile`.
- Persisted: none. Pins and edits stay in the set's state until sent (PROTO-07).
- Depends on: PROTO-02, PROTO-03, PROTO-06, PROTO-07; the chat visual frame and annotate bridge shared with chat visuals.
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: first version.
- Source: pv96 (unmerged). The viewer, the frame and the shared annotate module already exist on custom/integration (`PrototypeViewer.tsx`, `PrototypeFrame.tsx`, `native-chat-visual-annotate.ts`). pv96 adds the element-pick overrides, the pin persistence rule, the climb to the nearest control, the icon-only header and the collapsible panel.

### PROTO-06 Element edits, notes and note screenshots

- What the user does: picks an element and edits its values. The rows depend on the kind of element. Writes a note. Sees the element's bound design tokens as chips. On Send, each noted element is captured as a screenshot and sent with the message.
- Rules:
  - Rows by kind. Button: fill, text colour, border; radius, border width, shadow; width, height, padding; font, size, style, align. Badge: no width, height or align. Text: colour; width; font, size, line height, style, align. Box: fill, border; radius, border width, shadow; padding, gap. A component root shows layout rows (gap and padding for the whole component).
  - Number fields are clamped: px 0 to 4000, % 0 to 100, font size 1 to 200.
  - Colour has two palette swatches, a custom picker and a hex field.
  - Token chips: four are shown with "+N more" for the rest. A bound token past the fourth stays among the first four.
  - Text content is never edited in place. The panel says so.
  - A value that returns to its original value drops out of the draft. Reverting an item removes it.
  - Each edit is one item of the message. The draft groups items by pin. A click on a draft item jumps to its control and pulses it.
  - Note: free text, with example chips.
  - Screenshots: the desktop captures the noted element's rectangle on Send and uploads it. Mobile captures with a native view-capture library and a no-op on web. A blank capture is omitted.
- Client requests: `prototypes.writeAttachment` (a PNG; a base name of letters, digits, - and _; about 1.5 MB at most).
- Persisted: uploaded note images in the request folder. The draft is in memory only.
- Depends on: PROTO-05, PROTO-07, PROTO-10 (token bindings).
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The edit panel, number field, colour picker and the mobile draft and pin sheets already exist on custom/integration (`PrototypeEditPanel.tsx`, `PrototypeNumberField.tsx`, `PrototypeColorPicker.tsx`, `MobilePrototypeDraftSheet.tsx`, `MobilePrototypePinSheet.tsx`). The mobile capture library is already a dependency there too. pv96 adds element locators and per-kind rows, token chips, the clamp rules and note screenshots uploaded on Send.

### PROTO-07 Iterate and send to the originating chat

- What the user does: optionally writes an "Anything else" note, then presses Send. The message goes to the chat the set came from. A set made by a child session goes to that child's chat.
- Rules:
  - The message opens with the request line, such as `Iterate on B.`. It then lists the notes and edits as items, adds `from C` items for edits on sibling directions, and ends with the free text.
  - The hidden block tells the agent which file names to write (for example `B-v2.html`).
  - Send is disabled when there is nothing to send.
  - A send that fails keeps the draft, with one line under Send: "Couldn't send: the chat is no longer active. Your draft is kept."
  - A send the host accepted but reported as thrown is not resent. This is a follow-up.
  - On mobile, a cross-chat send goes through the host to the set's own chat.
- Client requests: the chat's normal send path (AG-15), with the hidden block.
- Persisted: the chat transcript records the visible text. The hidden block stays out of the bubble (PROTO-02 rules).
- Depends on: AG-15, PROTO-02, PROTO-05, PROTO-06, PROTO-08.
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The outgoing and host-chat send modules already exist on custom/integration (`mobile-prototype-outgoing.ts`, `mobile-prototype-host-chat-send.ts`). pv96 adds the item format with sibling items, the disabled-send rule and the failure line as the record describes.

### PROTO-08 Main direction, stars and merge

- What the user does: sees one main direction per set. Opens the other directions read-only, and can make one the main ("Back to main" or "Make main"). Stars directions in amber. With two or more stars, presses "Merge (n)" and reviews the merge: for each part, which direction it comes from, whether it is suggested, changed or left out, and a plan preview. Sends "Merge into D.". The merged direction becomes the new main, and the stars clear.
- Rules:
  - A read-only direction shows its properties under "Properties · read-only". It takes no edit and no note. Its own pins, edits and notes stay on it, and a former main keeps them visible read-only.
  - "View all" only opens the strip. It does not change the main.
  - A merge needs two stars and no merge already under way. The letter for the result must be free. A merge already sent does not hold the next one.
  - The agent writes a plan (`merge-plan.json`) with one source per part. A plan is accepted only for the merge that asked for it.
  - A plan file is at most 64 KB and holds at most 48 parts. Parts are sections or components. A part present in one direction has one selector.
  - Mobile shows the review with Parts and Preview tabs.
- Client requests: `prototypes.readFile` for the plan; the merge request goes through the chat send path (PROTO-07).
- Persisted: the plan and the merged direction are files in the request folder. Whether stars and the main persist across restarts was not traced.
- Depends on: PROTO-02, PROTO-04, PROTO-05, PROTO-07.
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The single active direction and the set state already exist on custom/integration (`prototype-set-state.ts`). Stars, the main-direction rule, read-only siblings, the merge plan and the merge review are new in pv96 (`prototype-merge-plan.ts`, `prototype-merge-state.ts`, `PrototypeMergeReview.tsx`, and the mobile merge sheets).

### PROTO-09 Versions, archive and delete

- What the user does: sees a direction's earlier versions and restores one as the newest. Archives a set and unarchives it later. Renames a set. Deletes a request whose chat and worktree are both gone.
- Rules:
  - Restoring copies an earlier version into the next version. Nothing is overwritten or removed.
  - Archiving keeps the set out of the main list and counts. After an archive the view opens the neighbouring set.
  - Delete is refused (`not_deletable`) while the chat and its worktree both still exist. The list shows whether each set is deletable. A remote chat is never treated as gone, because losing contact proves nothing.
  - A set title can be changed.
- Client requests: `prototypes.restoreVersion`, `prototypes.archive`, `prototypes.unarchive`, `prototypes.delete`.
- Persisted: the archive flag, the title and the version files, kept with the request on the host. Exact storage not traced.
- Depends on: PROTO-02, PROTO-03, PROTO-04, PROTO-13.
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The archive and restore-version methods have no references on custom/integration, and their modules are new (`prototype-archived-sets.ts`, `prototype-version-restore.ts`, `prototype-set-title.ts`). The request folder that they act on already exists.

### PROTO-10 Project design system

- What the user does: sees a Design system row above a project's sets, with a few colour swatches. Opens it to see Tokens, Components and Source. Edits tokens and component elements into one draft and sends it to the agent of the project's newest set. Re-detects the system from the project. Approves a proposed palette. Restores an earlier version. Sees "Built on Design system vN" on sets.
- Rules:
  - Three files are read from `design/system/`: `tokens.json` (colour, type, space, radius, shadow and motion tokens, with a version, status "detected" or "proposed", sources and palettes), `components.html` (one section per component, bound to tokens) and `system.md` (notes). A valid `tokens.json` makes the system readable. The other two are optional.
  - Two linked worktrees of one repository are two groups, because each has its own folder.
  - Components are shown as Grid, Collage or List. Each is rendered live in a script-less sandboxed frame with the draft applied. The choice is remembered per viewer.
  - Editable: colour, type, space and radius tokens. Shadow and motion are display-only. Component elements use the PROTO-06 rows. A value back at its original drops out of the draft.
  - Send: `Update the design system to vN+1.`, then groups of changed tokens and components, then, when chosen, "Rebuild {set titles} on vN+1" for the sets picked, then the free text. The system is built once and changed only incrementally (user requirement, 2026-10-29). Success clears the draft. Failure keeps it.
  - "Re-detect from project" sends `Re-detect the design system from the project.`
  - Send target: the chat of the project's newest set. With no set, a pending proposal request. With neither, Send is disabled.
  - "Used by N components · M sets": N counts components whose bindings name the token; M counts sets built on the current version.
  - An unreadable file is logged once per version (path, time, size), not on every refresh.
  - On mobile, a Design system screen with token, component and add sheets, a version banner and a send. Mobile edits only colour bindings, each labelled "{Prop} token".
- Client requests: `prototypes.readDesignSystem`, `prototypes.listDesignSystemVersions`, `prototypes.readDesignSystemVersion`, `prototypes.restoreDesignSystemVersion`. Summaries come in `prototypes.list`.
- Persisted: `design/system/*` in the project, written by the agent. Version history in a host store. The components view choice is kept per viewer in local browser storage.
- Depends on: PROTO-02, PROTO-03, PROTO-06, PROTO-11, AG-15 (send); remote hosts (the host reads the files).
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The design system files, the view, the tokens and the palettes tabs, and the `design/system` reader already exist on custom/integration (`prototype-design-system-files.ts`, `PrototypeDesignSystemView.tsx`, `PrototypeDesignSystemTokens.tsx`, `PrototypeDesignSystemPalettes.tsx`). pv96 adds the Components tab with live element editing, Grid, Collage and List, the version history and restore, the incremental and rebuild send, and the whole mobile design system screen.

### PROTO-11 Design system in prototype requests

- What the user does: after picking a count, sees a design system card for the request when the project has a system or a proposal. Picks "Use it", "Propose one first", "Skip" or "Review first". For a proposal, the chat shows a link card "Design system · proposed" with "Review ›".
- Rules:
  - The card line reads "v{n} · {count} tokens from {source names}.", then one of "The direction will use it.", "Both directions will use it." or "All {n} directions will use it.". A project with no system shows a "No design system" card.
  - "Use it" and "Propose one first" send the request with an instruction. For "Use it" the agent uses the system. For "Propose one first" the agent detects or proposes one and stops. "Skip" sends without the instruction. "Review first" opens the Design system view and leaves the card pending.
  - A request sent before any system existed records the system's version the first time the host sees one.
  - A proposed system with palettes shows a palette review. "Approve {name}" sends `Use the {name} palette for the design system.` with a hidden instruction. It sets the status to detected and makes the pending request's directions. "Adjust tokens" opens the Tokens tab.
  - A proposal with no direction files still lists its project's system, through the pending request.
  - Mobile sends the instruction without a card.
- Client requests: `prototypes.createRequest` with the design-system choice (use, propose or skip).
- Persisted: request mode and version in `request.json`.
- Depends on: PROTO-01, PROTO-02, PROTO-10.
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The request mode, the system card model and the proposal fields already exist on custom/integration (`prototype-design-system-card.ts`, `prototype-request-folder.ts`, and the "Propose one first" text). pv96 adds the incremental-update rule, the palette approval send, the review-first path and the mobile instruction. Whether the proposal link card existed on custom/integration was not checked.

### PROTO-12 Prototypes from child sessions, with attribution

- What the user does: asks a chat to hand a design to a child session, for example "have a child session design this". The child starts with the prototype request. The set shows which agent made it and the parent chat it came from, with links.
- Rules:
  - A composer message that asks for a child session or a subagent to design something is sent as typed, so the child's own first prompt makes the request. The detection covers English and Spanish phrasings.
  - The child's request uses the Settings count and no card.
  - A child that runs as a terminal gets no first prompt, so it makes no request.
  - Desktop: a line under the set title credits the child's agent and chat, with "open chat" and "from <parent chat>" links.
  - Mobile: the set subtitle adds "from <parent chat>".
  - The parent chat name comes from the task title, else from the display name. It is hidden when neither exists. A set made directly in a chat shows no origin.
- Client requests: the list row carries the origin (record).
- Persisted: the parent chat is stored with the request as its origin.
- Depends on: AG-26 (delegated child sessions), AUTO-20 (delegation from chat), PROTO-01, PROTO-04.
- Owner direction for the new product (2026-11-12): extension (A4). Delegation itself is in the first version. Proposal: later for the prototype part. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The delegation and bot-team prototype choice modules and the delegated-ask helper are new in pv96 (`delegation-prototype-choices.ts`, `prototype-delegated-ask.ts`, `bot-team-task-prototype.ts`).
- Owner direction for the new product (2026-11-12, child placement rules from the design session): the child placement rules apply to a prototype child like any delegated child: own worktree by default; the current worktree only for read-only work, a plain folder, or the user's manual pick; parallel prototype children always get their own worktree. Proposal unchanged.

### PROTO-13 `orca prototypes` CLI and baseline check

- What the user does: runs `orca prototypes` from a terminal to list sets, create a request, archive or unarchive it, delete it, restore a version, attach a note image, read a file, read or check a design system, or run a baseline check.
- Rules:
  - `list` prints each set's title, request id, chat id, agent and archived state, and for each project design system its version, status, token count, the chat its edits go to and its folder. `--worktree` filters. `--json` is available.
  - `create` needs a chat local to the host. Otherwise it exits 1 with `unsupported`. `--agent` takes claude, codex, antigravity, gentle-shell or other. `--count` takes 1 to 4. `--design-system` takes use, propose or skip.
  - `rm` is destructive and is refused while the chat and its worktree both exist.
  - `write-attachment` takes a PNG of about 1.5 MB at most, with a plain base name.
  - `design-system read` prints the file names; with `--json` it carries the three files. `design-system check` says whether a system is readable and why not, from a chat or a project folder.
  - `baseline-check` reads two files on this computer and talks to no host. It reports faithful or rebuilt: at least 90% of source rules copied verbatim and 90% of classes used. It exits 1 otherwise. It is for a request that keeps an existing page.
- Client requests: the same host methods as the views (PROTO-03, PROTO-09, PROTO-10).
- Persisted: none beyond the host effects.
- Depends on: PROTO-03, PROTO-09, PROTO-10; the CLI domain (`cli.md`).
- Owner direction for the new product (2026-11-12): extension (A4). Proposal: later. The baseline check is a candidate to drop, since it serves one workflow. Superseded: Owner decision taken on the owner's behalf (2026-11-12, `docs/design/pending-owner-review.md` item 12 and screen-inventory open question 16), owner-confirmed 2026-11-27: prototypes is core. Proposal now: later.
- Source: pv96 (unmerged). The `orca prototypes` command group is new in pv96 (`src/cli/specs/prototypes.ts`, `src/cli/handlers/prototypes*.ts`). custom/integration has no prototype CLI commands.

### PROTO-14 Design system shared between projects by link

- Status: new feature, owner direction for the new product from the design session (2026-11-12, `docs/design/screens/workspaces-sessions.md`, "Shared design system"). Neither `custom/integration` nor pv96 was seen to have it: in both, a design system is the files under one project's `design/system/` (PROTO-10).
- What the user does: opens a project's design system menu and picks "Share with project…", which lists the projects the user can open. The linked project then shows "Design system" with a link chip naming the source project; the source shows how many projects link to it.
- Rules:
  - Each project owns its design system. Sharing creates a link, not a copy, so two projects of the same company keep one visual language.
  - Edits happen only in the source project and reach every linked project.
  - A linked project can "Unlink" (keeps nothing and falls back to its own design system) or "Make a copy" (becomes its own, independent system).
  - Deleting the source asks first and names the linked projects.
  - Not designed: how a link works when the two projects sit on different hosts, and what the agent of a linked project may change (assumed: nothing, since edits happen only in the source).
- Depends on: PROTO-10, PROTO-11; the project registry (workspaces domain).
- Proposal: later, with PROTO-10, which is also later. If the owner moves the project design system into the first version, this sharing rule should come with it, because it decides where the files live.
