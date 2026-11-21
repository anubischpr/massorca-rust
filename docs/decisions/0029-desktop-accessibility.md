# 0029: Accessibility roles in the desktop components

Status: accepted, 2026-11-21.

## Context

The pinned GPUI (0020) integrates AccessKit. An element is reported to
assistive technology when it has an id and a role. `role()` and the `aria_*`
setters exist only on stateful elements. There are setters for the label,
description, selected, expanded, toggled, numeric value with its range, and
orientation. There are none for disabled, busy, read-only, invalid, modal or
live politeness.

GPUI's test platform never builds the AccessKit tree:
`Window::debug_a11y_tree_json()` stays `None` headlessly.
`AnimationElement`, from `with_animation`, forwards neither the role nor the
extra information, so an animated root loses its role in real frames.

## Decision

- **Every component exposes a role, a name and its state.** Roles by kind:

  | Kind | Role |
  |---|---|
  | Button | Button |
  | Toggle button | Button with toggled |
  | Checkbox | CheckBox, Mixed when indeterminate |
  | Switch | Switch, on the track |
  | Segmented control | RadioGroup, with a RadioButton per segment |
  | Progress, usage ring | ProgressIndicator, value 0–100 |
  | List row | ListItem, or TreeItem when expandable |
  | Tree | Tree |
  | Table | Table, Row, Cell, ColumnHeader |
  | Tabs | TabList, Tab |
  | Menu | Menu, with MenuItem and MenuItemCheckBox |
  | Select | ComboBox trigger and a ListBox of ListBoxOption |
  | Text field | TextInput, or SearchInput |
  | Popover | Dialog, non-modal |
  | Dialog | Dialog, or AlertDialog when destructive |
  | Tooltip | Tooltip |
  | Banner | Alert for danger, Status otherwise |
  | Error panel | Alert |
  | Toast | Alert for errors, Status otherwise |
  | Empty state, static card | Group |
  | Agent mark and status glyph | Image, named by the agent or the state word |
  | Kbd | Label, named by its spoken words |
  | Splitter | Splitter, with orientation and value |
  | Terminal | Terminal |

  Badges, skeleton blocks and other decorative parts have no id and no role,
  so they stay out of the tree.
- **Names.** A control with visible text is named by that text.
  - An icon-only control takes its name in its constructor, so it cannot be
    built without one.
  - The tooltip of a control with a visible name becomes its description. An
    icon-only button's tooltip is its name only, so it is not announced
    twice.
  - A search field with no label is named by its placeholder. A select is
    named by its label, then its value, then its placeholder.
- **State.**
  - Built-in setters cover toggled, selected, expanded and value.
  - Disabled, busy, read-only, invalid, modal and live politeness are written
    in the `write_a11y_info` hook through `a11y::annotate`, because GPUI has
    no setter for them.
  - Sort state rides in the column header's name ("sorted ascending").
- **Animated roots.** The role goes on a plain wrapper around the animated
  element, never on the animated element itself.
- **Tests.** The `a11y` probe makes the same two calls GPUI makes for each
  reported element, `a11y_role` and `write_a11y_info`. Component tests use it
  to assert role, name and state without a window. Render-view components
  are probed inside a host view.

## Consequences

- A new component is not done until it has a role, a name, its states and a
  probe test.
- The probe reaches only the root of a component. Children are covered by
  their own component's tests.
- Screen-reader behaviour in a real window is not tested automatically.
  Narrator on Windows, Orca on Linux and VoiceOver on macOS need a manual
  pass.
- Open gaps:
  - Live regions are approximated with Status and Alert roles.
  - The tooltip trigger has no description.
  - The popover trigger has no expanded state.
  - The dialog body is not described.
  - The text field's focus target may have no node of its own.

## Alternatives considered

- **Asserting on the real AccessKit tree.** The test platform never builds
  it.
- **Our own accessibility layer beside GPUI.** It would duplicate what
  AccessKit already sends to every platform.
