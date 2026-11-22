# 0073: Profile root access on Windows

Status: accepted, 2026-11-22. Revises the Windows part of 0004's
**Permissions** rule.

## Context

0004 let the Windows profile directory keep the access it inherits from its
parent, and relied on the owner-only pipe descriptor (0003). The service also
reads files from the profile, for example `agents/kinds/*.json` and agent
homes such as `agents/homes/claude/.claude/settings.json`, whose hooks run
code as the user. When `MASSORCA_HOME` sits in a folder other users can
change, another user can create the profile directory, plant files in it,
rewrite files the user owns, or swap a folder on the path for a link to one
they control.

Every case above needs a home in a place others can change, so the rule is
to refuse such a place rather than make it safe silently. That includes the
default home, `%APPDATA%\MassOrca`, when an older build created it: it
inherited what `%APPDATA%` grants, which on some computers includes an
AppContainer capability SID with full control, added by sandbox tools. The
owner then makes the home private once with `massorca profile secure`.

## Decision

- **At creation.** Each missing folder of `<home>`, `<home>/profiles` and
  `<home>/profiles/<name>` is created with `CreateDirectoryW` and a descriptor
  that names the current user as owner and carries a protected DACL with one
  entry: full access for that user, inherited by every file and folder below
  (`O:<user>D:P(A;OICI;FA;;;<user>)`). The folder is never reachable by
  others, not even briefly, and folders made later inherit from it. The
  service, `service run` and a client that autostarts the service all create
  them this way (`create_root`). On Unix they are created `0700` in one step.
  Folders above the home are created as usual.
- **At every start**, before anything in the profile is written or opened,
  `massorca_profile::secure_root` checks, in this order:
  1. **Location.** `<home>` and `<home>/profiles` must have an accepted
     owner (below), and no SID other than an accepted owner or Administrators
     may hold a right that changes
     them (add a file or folder, delete a child, delete, change the DACL or
     owner, generic write or all). `profiles` must not be a link; the home may
     be one the user chose. Otherwise the start is refused with
     `profile.unsafe_location`, naming the folder. On Unix an ancestor must be
     owned by the user or root and not group- or world-writable unless it is
     sticky. A home that is itself a link or junction is checked through its
     target, but the link lives in the home's parent, where whoever can change
     that parent could repoint it later. So such a home's parent must pass
     the same check; otherwise the home is refused with
     `profile.unsafe_location`.
  2. **Root.** A root that is a link or junction is refused with
     `profile.unsafe_location`. A root without an accepted owner is refused with
     `profile.foreign_entries`. On Unix the root itself must be owned by the
     user; a root-owned root is refused, while ancestors and entries below
     the root may be owned by root. A root whose DACL, inherited entries included,
     let another SID change it is refused with `profile.writable_by_others`
     and left as it is: files the user owns may have been rewritten, and a
     repair would lock the changed content in. On Unix a group- or
     world-writable root is refused the same way.
  3. **Repair.** Otherwise, when the root's DACL is not the protected
     owner-only one, it is replaced; Windows passes the change on to the
     inherited entries of everything below, and a warning names the path. On
     Unix a root that is not `0700` is set to `0700`. The repair comes before
     the content scan, so a refusal by the scan has already made the root
     owner-only. That change only takes access away and trusts nothing, so a
     refused start leaves the root private instead of unchanged.
  4. **Content.** The tree below the root is scanned after the repair, so
     nothing can be planted between the listing and the new access. Links
     are read themselves, never followed. On Windows, reading every
     descriptor of a 30,000-file agent home took 8 to 11 seconds, so a clean
     scan records each folder's modification time in `<root>/.access-scan`.
     While the root stays owner-only, a later start reads descriptors only in
     folders whose time changed. Adding, removing or renaming an entry
     changes its folder's time, and inside a private root only trusted
     principals can change anything else. A new or repaired root, and
     `profile secure`, always read every descriptor; that measured case took
     about 9 seconds once, and cached starts took 70 to 90 ms. Unix reads
     owner and mode with one `lstat` per entry and scans in full. An entry without an accepted owner, or whose
     owner cannot be read, is refused with `profile.foreign_entries`; an entry
     whose own DACL lets another SID change it is refused with
     `profile.writable_by_others`. On Unix the owner must be the user or root,
     and an entry must not be writable by others; group write is allowed,
     since entries have sat below a `0700` root since 0004.

  A refusal lists up to ten paths (`data.paths`, or `data.path` for a
  location). Nothing is repaired once the location or the root is refused.
  The messages of `profile.unsafe_location` and `profile.writable_by_others`
  say why in one sentence, "other programs on this computer could change
  these files", and name `massorca profile secure`.
- **Untrusted SIDs.** Only accepted owners and Administrators may hold
  write access. Capability and AppContainer SIDs (`S-1-15-2-*`,
  `S-1-15-3-*`) are untrusted: a sandboxed process that holds one runs
  separately from the user's own processes, and write access would let
  sandboxed code rewrite files the service trusts, which is the boundary the
  sandbox exists to keep.
- **`massorca profile secure`.** The owner's explicit way to make an existing
  home private. Without `--yes` it prints, for the home, `profiles` and the
  root, every access entry it would remove and the one it keeps, changes
  nothing, and exits 0. With `--yes` it sets the protected owner-only DACL on
  each of them, home first, so inherited entries below follow, then runs the
  full check, reading every descriptor. It refuses a folder another user owns
  or a link it cannot make safe. Existing files are kept and trusted as they
  are: the owner reviewed the plan and vouched for them by applying it. The
  scan still refuses entries another user owns, or that keep their own
  grant to others. A start never does this on its own.
- **A refused autostart is reported.** A client that starts the service
  passes `service run` a hidden `--start-status <file>` in a new private
  folder outside the profile. A refusal before the service accepts
  connections is written there as one `ErrorBody`, and the client returns
  `ClientError::StartRefused` with that code and message, exit code 1,
  instead of a ten-second timeout. It is an option rather than an
  environment variable, so it never reaches the terminals the service
  starts.
- **Accepted owners.** An owner is accepted when it equals the token's user
  (`TokenUser`), the token's default owner (`TokenOwner`), or SYSTEM. An
  elevated administrator token's default owner is Administrators, so what such
  a process creates is owned by Administrators and stays accepted while it
  runs elevated. Administrators is not accepted as an owner otherwise: an
  ordinary token creates nothing owned by it, so on that token such an entry
  was made by someone else. Administrators stays acceptable as a holder of
  write access in a DACL, since Windows grants it full access on the user's
  own folders. Every crate checks owners with the same functions in
  `massorca_profile` (module `windows_dir_access`, shared with agent
  credential homes): `Sid` (`parse` for `S-1-` text or an alias,
  `unsafe from_psid`), `owner_matches(&Sid, &[Sid])`,
  `owner_is_acceptable(PSID)` and `dir_owner_is_acceptable(&File)`.
- SIDs are compared as SIDs (`EqualSid`), never as text: SDDL writes some
  accounts as aliases, such as `SY`, `BA` and `LA` for the built-in
  Administrator. Every SID read from a descriptor is parsed back into its
  binary form, SYSTEM and Administrators are parsed from their SID strings,
  and the user and default owner come from the process token. "Already owner-only" is decided from the
  parsed DACL: protected, exactly one allow entry, full access inherited by
  files and folders, for the user's SID. A user running as the built-in
  Administrator is therefore neither refused nor locked out.
- Paths are passed to Win32 in their `\\?\` form, so homes past the old
  248-character limit work.

Repair keeps an ordinary existing profile working after an upgrade: only
trusted users could reach it, so its content is what the user left. Refusal
does not prove that nothing was changed; it stops the service from trusting a
profile that someone else could have changed. The owner reviews it, then runs
`massorca profile secure --yes` or recreates the profile.

## Alternatives

- **Create, then restrict.** Leaves a window in which another user can open
  the folder or plant files in it, and a handle opened then keeps its rights.
- **Repair everything, refuse nothing.** Locks in content others could have
  changed, and hides planted files behind a new access list.
- **Refuse any non-private root.** Breaks every profile created before this
  decision, although only the user could reach it.
- **Check every folder up to the drive root.** Folders above the home belong
  to whoever chose the home, and the user's own profile folders carry entries
  for system components that a strict check would refuse.
- **Hold a handle to the root while the service runs.** Would need every path
  open in the service to go through it. Refusing an unsafe location instead
  leaves no folder from the home down that another principal can swap; the
  home's own parent is checked only when the home is a link.
- **Scan the sensitive subtrees every start and the rest less often.** The
  largest subtree, agent homes, is also the most sensitive one, since agent
  settings can run hooks, so it would be scanned in full on every start
  anyway.

## Consequences

- `service run` checks the root before it creates the logs folder, so a
  refused profile receives nothing, not even a log file.
- Folders above the home are not checked. Someone who can change them can
  move the home itself; choosing a home in such a place is outside this rule.
- Entries an elevated process created belong to Administrators. They are
  accepted while the service runs elevated, and refused as foreign when it
  later runs on an ordinary token.
- The first start after a repair, and `profile secure`, read every
  descriptor; later starts read only changed folders. The cache sits in the
  private root, where only trusted principals can write it.
- A temporary folder shared with other users is an unsafe location, so tests
  put their home one level below it, where the service creates it private.
- A foreign owner cannot be set without a second account or privileges, so
  those refusals are tested with an injected owner. Real descriptors are used
  for every other case.
