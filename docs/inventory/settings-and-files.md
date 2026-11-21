# Settings, persistence and files: functional inventory (SET)

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read with read-only git from worktree `unified-custom-profile-build-547ea6`. The earlier pass read worktree `massorca-server-language-820c84`.

## Domain summary

This domain covers how MassOrca stores its settings and per-profile state and what survives a restart; the profile model; keyboard shortcuts; the interface language; and the file features used from a workspace: the file explorer, file operations, the built-in editor's read and save path, file watching, quick open, text search, drops and uploads, downloads, opening files in external editors, previews, notebooks and PDF export. Most behaviour sits in the Electron main process, reached through IPC channels from the desktop renderer and through runtime RPC methods from mobile and CLI clients. Renderer UI detail is out of scope and is noted only where it changes what the core must do. At pv96 the core also keeps a sealed token store for Marketplace services (SET-04, SET-29) and extra desktop UI state (SET-02, SET-10), and the settings record gains a few keys (SET-01).

## Source directories read

All paths are relative to the reference repository (`src/...`).

- `src/main/persistence/loading-store/`: `user-data-path.ts`, `store.ts`, `state-serialization-secret-handling.ts` (headers and rules), `legacy-opencode-go-api-key-migration.ts`; other files by name only.
- `src/main/persistence/applying-settings/`: `settings-update.ts` (write-boundary rules from comments); other files by name only.
- `src/main/persistence/profile-state/`: `legacy-json/README.md`, `profile-state-database.ts` (schema version and open rules), backup rotation and worker constants, authority and migration file names.
- `src/main/sqlite/`: `sync-database.ts`, `harden-database-files.ts`.
- `src/main/orca-profiles/`: `profile-storage-paths.ts`, `profile-index-store.ts` (header), `profile-ui-scope.ts`; cloud, transfer and owner-transfer files by name only.
- `src/main/startup/configure-process.ts` (user-data directory selection).
- `src/main/keybindings/` (`keybinding-service.ts`, `keybinding-file.ts`, parser diagnostics) and `src/shared/keybindings/` (action catalogue counted by prefix; other files by name).
- `src/main/i18n/main-i18n.ts`, `src/shared/ui-locale.ts`, `src/shared/ui-language.ts`.
- `src/main/external-editor-launch.ts`, `src/main/editor-launcher-name.ts`; `src/main/jetbrains-windows-gui-launchers.ts` by name only.
- `src/main/active-view-preference.ts` (rules and file name); `src/main/app-icon.ts` (signatures only); `src/main/file-transaction-lock.ts` (head).
- `src/main/ipc/`: `settings.ts`, `ui.ts`, `keybindings.ts`, `export.ts`, `notebook.ts` (rules), `doc-preview-grant-ipc.ts` (rules), `diagnostics.ts` (names only), `filesystem.ts`, and the `filesystem-*.ts` and `filesystem/*.ts` modules for listing, search, mutations, reads, writes, content inspection, watchers, downloads, imports, uploads and path authorization (mostly via comments and constants).
- `src/main/ripgrep/` (bundled text search and path helpers, rules only); `src/shared/text-search.ts`; `src/shared/file-name-sort.ts`.
- `src/main/runtime/rpc/methods/files*.ts` and `src/main/runtime/runtime-rpc/runtime-rpc-mobile-method-allowlist.ts` (what remote clients may call).
- `src/main/notebook/` (kernel and environment discovery); `resources/notebook/` (bridge file name and import check).
- `src/main/lib/html-to-pdf.ts` (named only, through the export handler).

Method: the earlier pass had no `.codegraph/` index in its reference repository, so every read used Read, Grep and Glob, with comments and constants as the main evidence. The pv96 pass used read-only git (`diff --name-status`, `diff --numstat`, `show` and `grep` on the commit) against `custom/integration`, then Read on the changed files, and one CodeGraph query for the `ai-vault` question.

pv96 delta (`git diff custom/integration d3c6311a531`, read by name-status, numstat and `git show`): `src/main/persistence/tracking-repos/` (`worktree-identity-migration.ts` and its new session-references header), `src/main/persistence/loading-store/normalize-loaded-global-settings.ts` (diff), `src/main/orca-profiles/profile-project-session-field-disposition.ts` (diff), `src/shared/default-global-settings.ts`, `global-settings-types.ts`, `persisted-ui-state-types.ts`, `ui-chrome-types.ts`, `rpc-contract/client-settings-params.ts` (diffs), `src/shared/keybindings/definitions-core-2.ts` and `types.ts` (diffs; the new action ids counted), `src/main/startup/native-theme-sync.ts` (header), `src/main/ipc/shell.ts` (diff), `src/main/marketplace/marketplace-vault.ts`, `marketplace-vault-crypto.ts`, `marketplace-vault-master-key.ts` and `marketplace-agent-env.ts` (for SET-04 and SET-29: key location, file format, environment variables), `src/shared/rpc-contract/marketplace-vault-params.ts` (name only).

## Not read / low confidence

- Not read (other domains or not needed): `persistence/leasing-ssh-ptys/`, `persistence/restoring-sessions/`, `persistence/tracking-repos/`, `persistence/terminal-topology/`, `persistence/scheduling-automations/`, `persistence/migrating-orcad-catalog/`, and the normalizers in `loading-store/` (`normalize-loaded-global-settings.ts`, `prepare-loaded-profile-settings.ts`, `loaded-cohort-migrations.ts`, `terminal-settings-migrations.ts`).
- Profile-state internals not read: document schema validation, writer thread and worker protocol, backup worker, startup recovery dialog, the `orca profile state` command implementation, revision re-admission.
- Profile cloud sign-in, organization members, project and session transfer, owner transfer: names and headers only.
- Filesystem handlers in part: quick-open listing and path search are summarized from comments; the local rename path in `filesystem-mutations.ts` is partly read; `filesystem-watcher-remote-*`, `filesystem-watcher-wsl.ts`, `filesystem-import-ssh.ts`, `filesystem-download-handlers.ts` and `filesystem-runtime-upload-staging.ts` are summarized from names and comments only. Git status, diff, commit and source-control handlers were skipped (source-control domain).
- Renderer (explorer, editor, shortcut editor, locale catalogues) skipped per the brief.
- `src/main/ipc/app.ts` and `src/main/ipc/shell.ts` not read.
- Not confirmed in code: the runtime-upload byte budget value; the editor autosave default and delay; the quick-open exclusion details; what a local rename does when the target already exists; the watcher capacity value; the exact default shortcut key per OS; the app-icon variant list.
- Low confidence: active-view restore (read from header and grep only), app-icon behaviour, Ghostty and Warp theme import (preview endpoints only).
- Cross-domain, names only: diagnostics bundle upload (`ipc/diagnostics.ts`), crash reporting, the browser session profile files.
- pv96: `src/main/ai-vault/` (the AI Vault session index, 332 files) was read by file list and headers only. It is session history, not a secret store.
- pv96: the Marketplace vault files were read for key location, file format and environment variables only; the Marketplace services, their RPC and their access rules were not traced.
- pv96: the keybinding and settings diffs were read as diffs; the renderer rail, side panel and settings screens were not read.
- pv96: `src/shared/secure-file.ts` and the Windows ACL helpers were read for their write-path logic only; their Windows behaviour was not run.

## Features

### SET-01 Settings: read, change and push updates

- Source: pv96 (unmerged). The settings record gains keys for the chat summary visibility, team character animations, the Jev decisions opt-in and the sidebar's worktree card layout, and the settings update params accept the card layout.

- **What happens.** The user changes a preference in Settings and every open window sees the new value. Settings are read as one record and changed through partial updates. A synchronous read exists for terminal panes that bind before settings finish loading.
- **Rules and edge cases.**
  - All writers (desktop IPC, the web and mobile RPC, and the CLI) pass one normalization boundary, so values are coerced to their type before they are saved. A truthy non-boolean is never stored as an enabled flag.
  - Some settings are trusted only from specific writers. Floating-workspace grants are accepted only when written by the owning path, and the artifact-publish capability must be an exact boolean on disk.
  - Nested records merge on partial update, so changing one consent flag does not reset its sibling identifiers.
  - Telemetry for a settings change is emitted only for values that actually changed.
  - Mobile may read and update only a subset: settings get and update, terminal quick commands, and native-chat session options. The generic settings setter is not on the mobile allowlist.
  - The settings record also carries per-host overrides (owned by the remote-host domain).
- **Client requests and core pushes.** Requests: get, update, set, list system fonts, read the default folder for projectless chats, preview theme imports (SET-28). Pushes: a changed-settings event to all windows.
- **Data persisted.** The global settings record in the profile state (SET-02). Credential-bearing keys are sealed (SET-04).
- **Depends on.** SET-02, SET-04, SET-05.

### SET-02 Persistent state that survives restarts

- Source: pv96 (unmerged). UI state now also keeps the side panel width and its open state per workspace, the rail mode and its one-time hint, the Marketplace host choice, bot-team workspace visibility and calendar options; session references in split panes are repointed when a worktree's identity changes.
- **What happens.** Projects and worktree metadata, session records, settings, UI state and caches are saved and reloaded on the next start. Changes are saved after a short debounce, and pending changes are flushed when the app quits.
- **Where it lives (per OS).**
  - The production data folder is named `orca` inside the operating system's application-data location (Electron's `appData`: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, `~/.config` on Linux). Dev builds use `orca-dev`, so they never share state with the installed app.
  - A production build can be pointed elsewhere with the user-data-dir switch; dev builds accept an override variable. Automated test runs use an isolated folder.
  - Inside the data folder, the profile index `orca-profile-index.json` sits at the root and each profile has `profiles/<profile id>/` with its database `profile-state.db` and its backups (SET-03).
- **Sidecar files.** A GitHub cache snapshot (`orca-github-cache.json`, written at quit, best effort, described in the code as safe to lose); `active-view.json` (SET-10); the mobile pairing registry and end-to-end key pair, which are copied forward from an older data folder into the canonical folder when they are missing there (the sources are left in place, and the copies are re-restricted to the current user); a secret-protection report; and several per-feature files (plugin state, runtime pointer, usage caches).
- **Rules and edge cases.**
  - Writes are queued. Some run on a dedicated writer thread. Quit flushes pending writes, and the view-state sidecar's final write is never overtaken by a later debounced write.
  - Loading repairs the state rather than failing on it: malformed or unknown sections are dropped or normalized, pane identities are normalized, and residue of repositories that are no longer registered is swept at load.
  - Database files are restricted to the owning user (owner-only mode on POSIX; on Windows the data folder's current-user-only access rules apply).
  - When a profile moves to the database, the legacy JSON copy is no longer rewritten on shutdown. Only explicit exports write JSON.
  - Writes can be frozen during an import or maintenance, so nothing races a snapshot.
- **Client requests and core pushes.** Clients do not save state directly. They change settings and UI state through their own calls, and the core persists them.
- **Depends on.** SET-03, SET-04, SET-05.

### SET-03 Schema versions, backups and refusal of newer data

- **What happens.** Old data is migrated when the app upgrades, data written by a newer build is refused rather than damaged, and rolling backups of the profile database are kept without user action.
- **Rules and edge cases.**
  - The profile database is at schema version 3, recorded in its header. An older schema is migrated forward on open (an automation-runs table migration exists). A database with a newer version than the build supports is refused.
  - Backups keep five copies, taken at most once an hour. A failed backup is retried after one minute, and a backup worker gets up to ten minutes before it is abandoned.
  - A profile folder that holds only the legacy JSON is migrated to the database on first start. Profiles from any older release can upgrade directly; no intermediate release is needed.
  - The in-app updater refuses releases below 1.4.214, the first stable database release, including pinned and locally built versions. Older database-capable builds still need a compatible schema.
  - A durable authority marker records that the database was established. If the database files are missing while the marker exists, startup does not revive stale JSON or create an empty profile.
  - If the legacy JSON and the database both changed separately, startup refuses to pick one silently and asks for a recovery choice (SET-09).
- **Data persisted.** Backup copies named after the database with a backup suffix; the authority marker; the recovery evidence archive.
- **Depends on.** SET-02.

### SET-04 Sealed secrets on the device

- Source: pv96 (unmerged). A Marketplace vault adds a second sealed store: format v2 with authenticated encryption and a 12-byte IV, ciphertext in `~/.orca/marketplace`, and a 32-byte master key that the OS keychain wraps when it is usable; otherwise the key sits in an owner-only file in `~/.orca/keys` (on Windows the user profile ACL is the only protection).
- **What happens.** Credentials kept in settings and profile state (for example agent API keys and account tokens) are stored encrypted with the operating system's credential store. The user never sees ciphertext, and the plaintext exists only in memory.
- **Rules and edge cases.**
  - Values are sealed with Electron's safe storage and opened when state loads. When the OS store cannot be used, the state is tagged as degraded in its fingerprint; the exact effects were not read.
  - Inactive profiles keep their sealed values without decrypting them.
  - An explicit clear must reach disk even though a sealed value looks empty in memory.
  - A sealed value that cannot be opened (after a keychain reset, or sealed by a different app identity) is retained rather than dropped, so a later build with the right keychain can still read it. Dropping it is treated as irreversible, so the code keeps it even after a definitive decrypt failure.
  - A single OpenCode Go key from an older slot is moved once into the current slot; the old copy stays until the move succeeds.
  - Serialized and exported state carries sentinel placeholders for secrets, not plaintext (exact rules not read in full).
- **Depends on.** The OS credential store; SET-02.

### SET-05 Profiles: list, create and switch

- **What happens.** The user keeps several profiles, each with its own settings, state database and optional cloud account. The default profile is a local one named "Personal". The user can create more local profiles and choose which one is active.
- **Rules and edge cases.**
  - The profile index is a small JSON file at the data root. Profile ids are 1 to 128 characters: a letter or digit first, then letters, digits, underscores or dashes. Ids become folder names, so an edited index cannot point outside the profiles folder.
  - Each profile folder holds its database, a browser-session metadata file and, for profiles migrated from the older layout, the legacy JSON copy.
  - The offline command line can list revisions, export and roll back each profile (SET-09).
- **Client requests and core pushes.** Requests: list profiles, create a local profile, select the active profile.
- **Depends on.** SET-02, SET-03; SET-06 (cloud account); SET-07 (moves between profiles).

### SET-06 Cloud account attached to a profile

- **What happens.** A profile can sign in to an Orca cloud account in the browser. The app then keeps the session, shows the account's capabilities and organizations, and lists organization members.
- **Rules and edge cases.**
  - Sign-in uses a PKCE flow with a local callback page. Refresh requests are guarded against replay, and sessions are stored per profile and can be invalidated.
  - Choosing an organization changes which members and cloud artifacts the profile sees. A cloud-artifact cleanup path exists for the profile; its triggers were not read.
  - A development-only sign-in path exists beside the production one.
- **Depends on.** The accounts and cloud domain (by name); SET-04; SET-05.

### SET-07 Moving projects and sessions between profiles

- Source: pv96 (unmerged). Pane focus is not carried with a transferred project or session, and a stored focus that no longer matches the restored panes restores nothing.
- **What happens.** A project (its repository and worktrees) and its sessions can be transferred to another profile, and the source is cleaned up afterwards.
- **Rules and edge cases.**
  - Every move is recorded as a durable move intent outside both profile databases, so an interrupted move can be resumed or reconciled. The source is removed only after the target has committed.
  - Worktree ids are remapped for the target, and retired worktree names are carried over.
  - Both profiles must have database authority before a transfer, even when the source is still a JSON copy.
  - Session transfer moves ownership of terminal and agent sessions, and session transcripts in Markdown are moved with them.
- **Depends on.** SET-05, SET-02; the workspaces and sessions domains (by name).

### SET-08 Importing the existing app's profile state

- **What happens.** On a machine that has the Electron app's profile data, the new core can read the old profile JSON and carry over the owner's agent accounts and subscriptions, which the product plan names as a requirement. The old file is never modified.
- **Rules and edge cases.**
  - The old profile is normalized through a frozen read-only store, so nothing writes while it is copied.
  - If the primary JSON cannot be read, the import falls back to the old five-file backup ring.
  - Publishing the imported state and committing secret retention happen together with the database write. A failed import leaves the previous sealed values in place.
  - Legacy plaintext keys are sealed on import (SET-04).
  - Profiles from any older release can be imported directly.
- **Depends on.** SET-02, SET-03, SET-04; the agent accounts domain (by name).

### SET-09 Rollback to JSON and JSON exports

- **What happens.** A profile can be exported to a versioned JSON file (the latest five are kept), or switched back to JSON-only use so an older build can run it.
- **Rules and edge cases.**
  - An explicit rollback archives the current database and its recovery evidence first. An interrupted rollback keeps the evidence and can be retried by revision.
  - Divergent histories are never merged. The user picks one copy: the current JSON, the current database, or the latest JSON export.
  - A profile changed by a transfer or an offline settings command must establish database authority first.
- **Depends on.** SET-03, SET-08.

### SET-10 Active view remembered across restarts

- Source: pv96 (unmerged). The remembered top-level view can now be the calendar or the marketplace view, and an older paired host that rejects a saved view gets the update without it.
- **What happens.** The top-level view that was open (the main navigation area) is reopened at the next launch.
- **Rules and edge cases.**
  - The value is saved in a small `active-view.json` next to the profile database and also held in the persisted UI state. Writes are debounced by about 100 ms.
  - An unknown or corrupt value falls back to the default view.
  - The quit-time write is the final one, and a pending write cannot overtake it.
  - Mobile reads and sets the same UI state through the UI calls.
- **Depends on.** SET-02.

### SET-11 Default keyboard shortcuts

- Source: pv96 (unmerged). Two actions are added to the Tabs group, New checklist tab (Mod+Shift+H) and Open a session (Mod+Shift+Y), so the catalogue has 93 named actions.
- **What happens.** Every command has a default shortcut shown in menus and in the shortcut settings. The catalogue has 93 named actions, grouped by prefix: tabs (21), terminal (15), sidebar (10), editor (9), browser pane (8), worktrees (6), workspaces (5), file explorer (5), zoom (3), floating workspace (2), app (2), floating terminal, voice, view, source control, settings, dashboard and chat (one each).
- **Rules and edge cases.**
  - Defaults are kept per operating system, so a chord can differ between Windows, macOS and Linux.
  - A terminal shortcut policy setting decides how chords pressed in the terminal are handled (exact rules not read).
  - Existing installs keep their old tab-switch chords. They are pinned once, per action, when the defaults changed. A user's own rebinding always wins.
- **Depends on.** The commands owned by the tabs, terminal, worktree, browser-pane and workspace domains (by name).

### SET-12 Custom keyboard shortcuts in a user file

- **What happens.** The user rebinds an action in Settings, or edits a JSON file in the home folder at `.orca/keybindings.json`. Changes apply to every window and rebuild the application menu at once.
- **Rules and edge cases.**
  - The file holds shared bindings plus one section per operating system, and the current system's section overrides the shared one.
  - An unreadable file gives no overrides and reports an error. The app never overwrites a file it cannot parse.
  - Unknown action names and unparseable shortcut strings are ignored with a warning. Conflicting custom bindings are removed on read, with an error diagnostic.
  - A rebinding that conflicts with another action is refused. Resetting an action removes only the current system's entry and leaves a shared binding that the user wrote by hand.
  - Bindings from the old global-settings location migrate once, only when the file does not yet exist. After that the file is the only source.
  - The file can be opened in the default editor or revealed in the file manager.
- **Client requests and core pushes.** Requests: read the snapshot, create the file if missing, set one action, reload, open, reveal. Pushes: a keybindings-changed event to all windows, then a menu rebuild.
- **Depends on.** SET-11.

### SET-13 Interface language

- **What happens.** The user picks the interface language, and menus and dialogs follow it.
- **Rules and edge cases.**
  - Six languages are supported: English (the default), Chinese, Korean, Japanese, Spanish and French. "System" follows the operating system's locale and maps regional variants to these six.
  - Main-process text carries English fallbacks. Non-English catalogues load on demand after startup, and menus wait for the catalogue before rendering.
  - Plugins can register language packs that add or override strings under their own language identifiers.
  - A pseudo-localization mode exists for testing.
- **Depends on.** The UI catalogues of each client (by name).

### SET-14 File explorer listing

- **What happens.** The explorer lists a workspace's folders and files, folders first and then by name. Symbolic links are shown as files.
- **Rules and edge cases.**
  - Symbolic links are not followed while listing. On macOS this avoids touching privacy-protected containers.
  - A remote workspace's listing is re-sorted locally, because an older relay might sort differently.
  - A failed listing is recorded with the failing step and a redacted path shape, for diagnosis.
  - A folder can be listed only inside an authorized root: a registered repository and its worktrees, linked worktrees that git reports, or a folder workspace. Paths are canonicalized before the check, so a symlink that escapes the root is refused. Windows and WSL UNC forms are normalized before comparison.
  - Markdown documents in a folder can be listed separately.
  - A server directory browser lets a remote client pick a folder on the host.
  - Mobile clients can list folders and read files in allowed workspaces.
- **Client requests and core pushes.** Requests: list a folder, list markdown documents, stat a path, check that paths exist, browse a server folder. Live changes come from SET-17.
- **Data persisted.** Nothing; the listing is computed on each request.
- **Depends on.** Workspaces (worktrees and folders), SSH hosts, SET-17.

### SET-15 Create, rename, copy and delete files and folders

- **What happens.** The user creates a file or folder, renames, duplicates or deletes one from the explorer.
- **Rules and edge cases.**
  - Creating a file refuses to overwrite an existing name. Creating a folder creates any missing parents.
  - Duplicating refuses an existing destination. The caller picks a free name first.
  - Renaming renames a symbolic link itself, not its target. Over SSH a rename never replaces an existing target. Local behaviour when the target exists is not confirmed.
  - Deleting moves the item to the operating system's Trash or Recycle Bin. A symbolic link is trashed as a link. Paths on WSL network shares have no Recycle Bin, so they are removed inside the Linux distribution. An item that is already gone counts as deleted.
  - Explorer undo, redo, copy path and copy relative path are keyboard actions (SET-11). Their behaviour lives in the renderer and was not read.
  - Mobile clients can create files but not rename, copy, delete or write (allowlist).
- **Depends on.** SET-14.

### SET-16 Built-in editor: open and save

- **What happens.** A text file opens in the built-in editor, and saving writes it back to disk. The user can search and replace, toggle word wrap, jump between changes, copy context, add review notes and toggle Markdown preview.
- **Rules and edge cases.**
  - Text files up to 50 MB open. Larger text files are refused as not editable. Files are checked for binary content from their first 8 KB. Large unknown files are probed before they are fully read, so archives are not loaded only to be refused.
  - Files between the editor's comfortable size and the 50 MB limit open with reduced editor features (renderer behaviour).
  - Reading uses chunked reads for large files. Local files are opened without blocking, so a named pipe cannot hang the read.
  - Saving writes the whole file.
  - Saved preferences: autosave on or off with a delay, minimap, editor font (follows the terminal font unless set), word wrap (on by default), and preview tabs. The autosave default and delay are not confirmed.
- **Depends on.** SET-14, SET-15, SET-17, SET-11, and SET-24 for previews.

### SET-17 File watching and live refresh

- **What happens.** The explorer, the editor and other views refresh when files change on disk, without a manual reload.
- **Rules and edge cases.**
  - A worktree is watched recursively while a view needs it, and released when nothing needs it.
  - Events are coalesced over a short trailing debounce. The last event per path wins. A delete followed by a create is reported as both, so the subtree is cleared and the parent refreshed. A create followed by a delete is dropped.
  - A batch holds at most 5,000 paths. A deletion storm or any watcher error becomes one "refresh everything" signal, so caches are never trusted across a gap.
  - High-churn folders are excluded at the watcher level: `.git`, `node_modules`, `dist`, `build`, `.next`, `.cache`, `target`, `.venv` and `__pycache__`. The explorer's display filter is separate and only hides rows.
  - The native watcher runs in a separate process, so a crash in it does not take down the app. Its errors are treated as overflow.
  - Watched roots are limited by capacity. When the limit is reached, a root waits in a dormant state and is re-armed when a slot frees.
  - Remote (SSH) roots use the relay's watcher with retries. WSL roots take their own path.
- **Client requests and core pushes.** Requests: watch and unwatch a worktree. Pushes: a changed event carrying each batch.
- **Depends on.** Workspaces; SSH relay. Git status freshness in the source-control domain depends on this feature, and that domain must decide how it stays fresh.

### SET-18 Quick open by file name

- **What happens.** The user types part of a name and gets a list of matching files in the workspace.
- **Rules and edge cases.**
  - Files are listed with ripgrep. A superseded or cancelled query is dropped and its work is stopped by request token.
  - When the main worktree is the repository root, linked worktree folders are excluded, so files are not listed twice.
  - Older SSH relays cap results at 33. Newer relays use the requested maximum.
  - With no provider (cold start or disconnected), the list is empty rather than an error.
  - A failed spawn is reported as system pressure, not as a broken install. A timeout discards partial output, because a truncated path would be wrong.
- **Depends on.** SET-14; workspaces.

### SET-19 Text search across a workspace

- **What happens.** The user searches the text of a workspace and gets matching lines grouped by file.
- **Rules and edge cases.**
  - Options: case sensitivity, whole word, regular expression, include globs and exclude globs. Exclude globs are passed to ripgrep as negated patterns.
  - Local search uses the bundled ripgrep. A remote workspace is searched by the SSH relay, which can use git grep.
  - Results are capped at 2,000 matches. A 15-second limit stops the scan and returns what was found, flagged as truncated, so the UI can say the list is incomplete.
  - Long matching lines are shortened for display.
  - A running search can be cancelled by its request token.
  - Highlight positions are computed in JavaScript when the pattern can be read that way. Otherwise the whole line is highlighted.
- **Client requests and core pushes.** Requests: search, cancel search.
- **Depends on.** Workspaces; SSH relay; bundled ripgrep.

### SET-20 Dropped files as agent attachments and runtime uploads

- **What happens.** The user drags files into an agent chat or into a remote runtime, and they arrive as attachments or uploads in the workspace.
- **Rules and edge cases.**
  - Attachments are capped at 50 MB each, and their names at 200 bytes. A name that tries to leave its folder is rejected. A leading double dot is allowed when it is not an escape.
  - A whole drop shares one byte budget, so many small sources cannot get around a per-source limit. The numeric value was not confirmed.
  - Local worktrees receive the dropped path directly. Remote runtimes receive streamed uploads, read slice by slice in the main process. The renderer cleans up its own temporary files.
- **Depends on.** Agent sessions and the remote runtime (by name).

### SET-21 Copying external files and folders into a workspace

- **What happens.** The user brings files or folders from outside a workspace into it as a copy.
- **Rules and edge cases.**
  - The destination must be authorized before any copy starts.
  - A folder copy that fails partway is rolled back; the import tests cover the rollback.
  - Over SSH, the remote existence and path safety are checked before copying.
  - Nested repositories can be imported through their own handler.
- **Depends on.** Workspaces; SSH hosts (by name).

### SET-22 Downloading remote files and folders

- **What happens.** A file or folder on a remote workspace is saved to this computer, with progress and cancel.
- **Rules and edge cases.**
  - Files arrive in chunks, and the finished partial file is promoted to its final name by a no-clobber rename. Cancelling removes the partial file.
  - Downloading a folder opens a folder picker, after the capability and abort checks.
  - If cleanup fails during a folder download, the transfer error is reported, not the cleanup error.
- **Depends on.** SSH hosts; workspaces (by name).

### SET-23 Opening files in external editors and other applications

- **What happens.** The user opens a workspace or file in VS Code, Cursor, JetBrains, Vim or another editor set in settings. A file can also be revealed in the file manager or opened in its default application.
- **Rules and edge cases.**
  - The setting can be a plain command (the default is `code`), a full path to a launcher, or a compound shell command with the path appended.
  - Cursor opens a new window, so the folder is not routed to a window that is already open.
  - On Windows, a JetBrains console shim is replaced by its GUI executable beside it and started detached, so no console window stays open.
  - Vim and Neovim keep their console on Windows.
  - For VS Code, WSL paths open through its WSL remote and SSH workspaces through its SSH remote.
  - Launches run detached. The app reports only whether the process started.
- **Client requests and core pushes.** Requests: reveal a file, open a file in its default application (also used for the keybindings file). Mobile can ask the host to open a file or diff in the editor tab. That is a separate path.
- **Depends on.** Workspaces; the built-in editor (SET-16) as the alternative.

### SET-24 Image, PDF and Markdown previews

- **What happens.** Images and PDFs show inline, Markdown documents render as previews, and terminal artifacts can be previewed.
- **Rules and edge cases.**
  - Previewable binaries (images and PDFs) are returned as encoded data, up to 50 MB.
  - A document preview page gets a grant from the main process. The grant names the document and its containing folder, is registered for the page that opens it, and can be revoked.
  - Preview guests cannot reach the grant-issuing channel directly.
- **Depends on.** The explorer (SET-14); the browser pane (by name).

### SET-25 Notebooks (.ipynb) with Python kernels

- **What happens.** A `.ipynb` file opens as a notebook, and its cells run in a Python kernel. The user can create a virtual environment or install ipykernel when it is missing.
- **Rules and edge cases.**
  - Each notebook gets its own kernel. Kernels belong to the window that opened them and are shut down when that window reloads, crashes or closes.
  - One execution runs at a time per kernel. Output frames stream back. Interrupt and shut down are separate actions.
  - The kernel's working folder is the notebook's own folder, resolved to its real location when inside a project.
  - Environments are discovered from `.venv` and `.conda` folders, from the notebook's folder up to the workspace root, nearest first. Their Python version is read from files (`pyvenv.cfg`, `conda-meta`) with bounded reads. A repository's own interpreter is not run to inspect it.
  - A missing ipykernel reports a status with an install action.
- **Depends on.** Python on the host (by name); the editor (SET-16).

### SET-26 Exporting a document to PDF

- **What happens.** The user exports a rendered document (HTML) to a PDF through a save dialog.
- **Rules and edge cases.**
  - The suggested file name comes from the title. Characters that Windows forbids are replaced, and the name is at most 100 characters. An empty title becomes "export".
  - Rendering has a time limit. An empty document is refused with a message. Cancelling the save dialog is not an error.
- **Depends on.** The document view (by name).

### SET-27 App icon choice and Dock icon

- **What happens.** The user chooses an app icon variant, and on macOS the Dock icon follows the choice.
- **Rules and edge cases.** The choice is applied at runtime, and macOS persists the Dock icon. Icon images come from packaged assets. The variant list and startup behaviour were not read.
- **Depends on.** Packaging (by name).

### SET-28 Importing terminal themes from Ghostty and Warp

- **What happens.** The user points the app at a Ghostty configuration or a Warp theme file, sees a preview of what would be imported, and then decides.
- **Rules and edge cases.** The preview endpoints exist for both formats. The apply step lives in the renderer and was not read.
- **Depends on.** Terminal appearance settings (by name); SET-01.

### SET-29 Secret vault with scopes, links and in-chat requests

- **Status.** New feature, owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md` section 19: D-VT-01, X-VT-01, X-CH-07, D-VT-02, M-VT-01). The custom/integration app has no secret vault; it seals its own credentials only (SET-04). The "vault" tab in the right sidebar lists agent session history from `src/main/ai-vault/` (the AI Vault session index) and is not this feature.
- Source: pv96 (unmerged). A narrower Marketplace vault now exists: it seals OpenMetadata, Engram Cloud, Azure DevOps and Gitea tokens on the host, and local terminal spawns (not SSH or WSL) receive the OpenMetadata token and the Engram Cloud token and server URL as environment variables without a per-agent filter, while Azure DevOps and Gitea tokens stay with MassOrca's own source-control calls. It has no variable names, scopes, links, in-chat requests or secret entry screen, so only storage and environment delivery of this feature exist; the owner should review that every local terminal, not only agents, receives those tokens.
- **What happens.** The owner stores API keys, tokens, usernames and passwords in a vault in the core, so agents can use them without the owner pasting them into the composer and without storing them outside the app. Each secret has a variable name, a value and an owning scope: a project, a team or global. Sharing a secret with another scope creates a link, never a copy. The vault screen lists secrets per scope with their owner and links, and shows which agents and sessions used each one. The owner can edit, rotate, delete and unlink.
- **In-chat requests.** When the owner asks, or an agent finds it needs a secret, a request card appears in the conversation with the variable name and a state (pending, stored, linked, declined, failed), never the value. Add opens the entry modal: variable name, masked value with a reveal toggle, and a scope picker that can also link an existing secret.
- **Rules and edge cases.** Deleting a secret at its owning scope removes it from every linked scope, and the confirmation names how many linked projects and teams lose it. Unlinking asks nothing (assumed in the design). Until the owner confirms otherwise, a linked scope uses the same variable name (design open question 10). Where the vault screen opens from is assumed to be settings (design open question 11). On Android, listing and adding are proposed, with the other actions on desktop (design open question 12). How a secret reaches an agent process (environment variable, file, or on request) is not designed.
- **Depends on.** SET-04 (sealed storage); structured chat (agents domain, AG-15) for the request card; Workspaces; Bot Teams (AUTO-12) for the team scope.

### SET-30 App light and dark themes

- **Status.** Owner direction for the new product (2026-11-12, D-SH-14 and D-ST-03 in `docs/design/screen-inventory.md`). The current app's own appearance setting was not inventoried in this document; terminal themes are TERM-21.
- Source: pv96 (unmerged). The rail carries the theme toggle; the native window theme follows the saved theme whichever writer saved it (Settings, the rail or the CLI); and `orca theme get|set` reads and sets the same value (CLI-36).
- **What happens.** A theme toggle on the activity rail switches the app between light and dark. The full theme choice stays in settings.
- **Rules and edge cases.** Light and dark ship together, on the same frame and canvas structure. Plugin themes (CUS-06) would register into the same theme registry.
- **Depends on.** SET-01.
