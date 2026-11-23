# 0070: Settings the desktop needs

Status: accepted, 2026-11-21.

## Context

The Settings page (D-ST-01 to D-ST-12) draws most of its rows disabled with
"Not available yet". The service declares one setting,
`workspaces.default_root`. The store already keeps values by owner and key, and
`config.get`, `config.set` and `config.list` already validate against
declarations. Three things are missing:

- **Kinds.** The kinds are string, bool, integer and path. They cannot express
  "System, Dark or Light", "a number from 1 to 5" or a keybinding table.
- **Change events.** `config.set` publishes nothing, so a second window or a
  phone never learns of a change.
- **Commands.** No command restores a default, and the CLI command is
  `massorca config`, while the desktop and the inventory say "settings".

## Decision

### Keys

The service declares only the keys below. Each one is a row the design shows
with a value the service can hold. All of them belong to the owner `core`.

| Key | Kind | Default | Design |
|---|---|---|---|
| `conversations.folder` | path | `<home>/MassOrca/Conversations` | D-ST-01 |
| `updates.channel` | choice `stable`, `preview` | `stable` | D-ST-01 |
| `agents.default_kind` | name (alias `agents.default`) | `claude` (assumed) | D-ST-02 |
| `agents.delegation_depth` | integer 1 to 5 | `3` | D-ST-02, decision 11 |
| `agents.computer_use` | bool | `false` (assumed) | D-ST-02 |
| `agents.browser_access` | bool | `false` (assumed) | D-ST-02 |
| `agents.keep_awake` | bool | `false` (assumed) | D-ST-02 |
| `sessions.search` | bool | `true` (assumed) | D-ST-02 |
| `appearance.theme` | choice `system`, `dark`, `light` | `system` | D-ST-03, A8 |
| `appearance.accent` | choice `lavender`, `blue`, `teal`, `rose`, `orange`, `graphite`, or a `#rrggbb` colour | `lavender` | D-ST-03, A15, design system 6.4 |
| `appearance.density` | choice `compact`, `default`, `comfortable` | `default` | D-ST-03 |
| `appearance.text_size` | integer 90 to 130 (percent) | `100` | D-ST-03 |
| `appearance.font.interface` | string | `Geist` | D-ST-03, pending review 9 |
| `appearance.font.monospace` | string | `JetBrains Mono` | D-ST-03, pending review 9 |
| `appearance.session_summary` | bool | `true` (assumed) | D-ST-03 |
| `appearance.status_bar` | set of `agent_status`, `memory`, `terminals`, `ports`, `hosts` | all five | D-ST-03 |
| `input.keybindings` | keybinding overrides | `{}` | D-ST-04, SET-12 |
| `editor.tab_width` | integer 1 to 16 | `4` | D-ST-04 |
| `terminal.font` | string | `JetBrains Mono` | D-ST-05, fidelity N24 |
| `terminal.scrollback` | integer 100 to 100000 | `1000` | D-ST-05, fidelity N24 |
| `terminal.shell` | string, empty for the host default | `""` | D-ST-05 |
| `browser.home` | string | `about:blank` | D-ST-05 |
| `git.fetch_on_focus` | bool | `false` (assumed) | D-ST-06 |
| `notifications.agent_finished` | bool | `true` (assumed) | D-ST-08 |
| `notifications.waiting` | bool | `true` (assumed) | D-ST-08 |
| `notifications.routine_ran` | bool | `true` (assumed) | D-ST-08 |
| `notifications.extension_notices` | bool | `true` (assumed) | D-ST-08 |
| `voice.dictation` | bool | `false` (assumed) | D-ST-08 |
| `privacy.telemetry` | bool | `false` | D-ST-10: "off unless the owner turns it on" |
| `network.proxy` | string, empty for none | `""` | D-ST-10 |
| `advanced.enabled` | bool | `false` | D-ST-10: "off by default" |

"(assumed)" marks a default the design does not state. Defaults for agent
powers (computer use, browser access, keep-awake) start off, because turning
them on gives an agent more reach.

Rows that stay out, and why:

- **Owned by another domain:**
  - account and updates status;
  - agent availability and accounts;
  - git and task providers;
  - hosts and phones;
  - extensions;
  - the vault (0071);
  - routing history;
  - layout presets and theme import.
- **The design gives no values yet:**
  - quick commands;
  - notification sounds;
  - the speech model;
  - terminal palette and rendering;
  - search engine, zoom, browser profiles and link routing;
  - default branch handling;
  - API budget;
  - cache timers;
  - experimental flags.
- **A credential:** the transcription key goes to the vault (decision 12), never
  to a setting.

### Kinds

Four kinds are added to `SettingKind`:

- **choice:** one of a fixed list of strings;
- **ranged integer:** a whole number from a minimum to a maximum, both
  inclusive;
- **set:** a list of distinct strings drawn from a fixed list;
- **keybinding overrides:** a JSON object from action id to a shortcut string,
  or to `null` for "unbound".

`appearance.accent` takes either a preset name or `#` followed by six hex
digits. The desktop derives the accent set from the value and checks its
contrast (design system 6.4). The service checks only the shape.

The service validates keybindings without the default catalogue, which the
desktop owns (SET-11):

- An action id follows the setting name rule.
- A shortcut is modifiers from `Mod`, `Ctrl`, `Alt`, `Shift` and `Meta`, in any
  order and each at most once, joined by `+`, followed by one key. `Mod` means
  Ctrl on Windows and Linux and Cmd on macOS.
- Two actions bound to the same shortcut are refused with
  `config.invalid_value`, and the message names both actions. A reassignment
  writes the whole object in one `config.set`, so the move and the unbinding
  land together.

### Methods and events

- `config.get`, `config.set` and `config.list` keep their names and shapes.
  Existing clients keep working.
- **`config.reset`** (HostAdmin). It takes `{ owner?, key }`, deletes the stored
  value, and returns the `ConfigValue` that now applies. Resetting a key that
  already holds its default succeeds and publishes nothing.
- **`config.changed`** is a new event topic. `config.set` and `config.reset`
  publish it after the store confirms, and only when the stored value actually
  changed. Its `id` is `<owner>/<key>`; the key name rule excludes `/`, so the
  pair is unambiguous. The event carries no value. A client re-reads with
  `config.get`, so a value never travels to a subscriber that did not ask for
  it.
- **`config.get` and `config.list` gain optional fields.** Each value gets
  `kind`; a choice, set or colour gets `choices`; a ranged integer gets `min`
  and `max`. The desktop can then build its controls from the
  service. Older clients ignore the new fields.
- `config/1` stays the capability. Older services reject `config.reset` as an
  unknown method, which a client can tell apart from a refusal.

### CLI

`massorca settings get|set|list|reset` is the command group.
`massorca config` stays as a hidden alias, so scripts that use it keep
working. `settings set` parses its value as JSON and falls back to plain text,
as `config set` did. `settings list` prints the key, the value and `(default)`
when the value is the default.

## Alternatives

- **New `settings.*` methods.** These would rename a wire surface that clients
  already call, for no behaviour gain. The CLI name changes; the wire does not.
- **One settings record with partial updates, as pv96 had.** The store is
  already keyed by owner and key. Extensions declare keys under their own
  owner, and a single record would need merge rules for every nested value.
- **Keybindings in a user file (SET-12).** A file on the service host cannot be
  edited from a phone. It also needs its own watcher and parse-error handling.
  One validated setting gives the same result.

## Consequences

- Every key above has a default, so the desktop can turn its rows on as soon as
  the service lists them.
- Settings live on the host the client talks to. A desktop connected to a
  remote host sees that host's theme. Per-client overrides are not designed;
  a client may keep a local copy for its first paint.
- The service never checks shortcut conflicts against the default catalogue.
  The desktop shows those conflicts (D-ST-04).
