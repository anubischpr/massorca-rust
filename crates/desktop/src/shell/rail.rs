//! The Activity rail: the mode button and its morph, the page entries, the theme toggle, the
//! usage rings and Settings. One tab stop, moved with the arrow keys (D-SH-09 to D-SH-16).

use super::actions::{RailFirst, RailLast, RailNext, RailPrevious};
use super::tooltip::with_tooltip;
use super::{Shell, change};
use crate::model::morph::{MorphFrame, ease, incoming, outgoing};
use crate::model::navigation::Mode;
use crate::model::rail::{RailEntry, RailItem, rail_items, theme_label};
use crate::model::roving::{FocusStep, step};
use crate::model::shell::{Agent, UsageModel, UsageStatus};
use crate::model::shell_state::ShellModel;
use gpui::{
    Animation, AnimationExt, AnyElement, App, Context, Div, FocusHandle, IntoElement, KeyDownEvent,
    Radians, Role, SharedString, Stateful, Transformation, WeakEntity, Window, div, prelude::*, px,
    size, svg,
};
use massorca_ui::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use massorca_ui::components::popover::{Popover, PopoverPlacement, PopoverVariant, PopoverWidth};
use massorca_ui::components::skeleton::{LineWidth, Skeleton};
use massorca_ui::components::usage_ring::{UsageReading, UsageRing, UsageSlot, usage_name};
use massorca_ui::style::{FocusRing, Surface};
use massorca_ui::tokens::{
    BORDER_WIDTH_THICK, DURATION_MODERATE, EASE_STANDARD, RADIUS_MD, RADIUS_XS, SIZE_ICON_MD,
    SIZE_RAIL_BUTTON, SIZE_RAIL_WIDTH, SIZE_TITLEBAR_MACOS, SPACE_1, SPACE_2, SPACE_HALF,
    TYPE_BODY, TYPE_BODY_SM,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// The rail's GPUI state: a focus handle per item in roving order, and the morph in progress.
#[derive(Debug)]
pub(super) struct RailState {
    handles: Vec<FocusHandle>,
    /// The item with the tab stop; the arrow keys move it.
    roving: usize,
    morph: Option<ModeMorph>,
    morph_count: u64,
}

/// A mode switch in progress: the mode it leaves. Its generation restarts the animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModeMorph {
    from: Mode,
    generation: u64,
}

impl RailState {
    /// The generation of the mode switch in progress, if any: the Sidebar fades in for it.
    pub(super) fn morph_generation(&self) -> Option<u64> {
        self.morph.map(|morph| morph.generation)
    }

    /// The item F6 lands on: the one with the tab stop, so the arrow keys carry on from it.
    pub(super) fn focus_target(&self) -> &FocusHandle {
        &self.handles[self.roving]
    }

    pub(super) fn new(usage_rings: usize, cx: &mut App) -> Self {
        let handles = rail_items(usage_rings)
            .iter()
            .map(|_| cx.focus_handle())
            .collect();
        Self {
            handles,
            roving: 0,
            morph: None,
            morph_count: 0,
        }
    }
}

impl Shell {
    /// The mode button (P6). From a full page the press returns home; at home it switches the
    /// mode, and the morph runs unless motion is reduced.
    pub(super) fn press_mode(&mut self, reduced_motion: bool) {
        let from = self.model.navigation().mode;
        if self.model.press_mode_switch() && !reduced_motion {
            self.rail.morph_count += 1;
            self.rail.morph = Some(ModeMorph {
                from,
                generation: self.rail.morph_count,
            });
        }
    }

    /// Moves focus and the tab stop to the neighbouring rail item, without wrapping.
    pub(super) fn move_rail_focus(
        &mut self,
        movement: FocusStep,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .rail
            .handles
            .iter()
            .position(|handle| handle.is_focused(window))
            .unwrap_or(self.rail.roving);
        let next = step(current, self.rail.handles.len(), movement);
        self.rail.roving = next;
        window.focus(&self.rail.handles[next], cx);
        cx.notify();
    }
}

/// The rail: a full-height column on the frame surface (D-SH-09).
pub(super) fn rail(
    model: &ShellModel,
    state: &RailState,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let theme = *cx.theme();
    let shell = cx.entity().downgrade();
    let mut column = div()
        .id("activity-rail")
        .key_context("ActivityRail")
        .on_action(cx.listener(|shell, _: &RailPrevious, window, cx| {
            shell.move_rail_focus(FocusStep::Previous, window, cx);
        }))
        .on_action(cx.listener(|shell, _: &RailNext, window, cx| {
            shell.move_rail_focus(FocusStep::Next, window, cx);
        }))
        .on_action(cx.listener(|shell, _: &RailFirst, window, cx| {
            shell.move_rail_focus(FocusStep::First, window, cx);
        }))
        .on_action(cx.listener(|shell, _: &RailLast, window, cx| {
            shell.move_rail_focus(FocusStep::Last, window, cx);
        }))
        .flex()
        .flex_col()
        .flex_none()
        .items_center()
        .gap(px(SPACE_1))
        .w(px(SIZE_RAIL_WIDTH))
        .h_full()
        .pt(px(top_inset() + SPACE_2))
        .pb(px(SPACE_2 + SPACE_HALF))
        .bg(theme.palette.surface.frame);
    for (index, item) in rail_items(model.snapshot().usage.len())
        .into_iter()
        .enumerate()
    {
        if item == RailItem::Theme {
            column = column.child(div().flex_1());
        }
        column = column.child(rail_item(item, index, model, state, &shell, &theme));
    }
    column
}

/// macOS keeps its traffic lights in a 40 px zone above the rail (ActivityRail spec).
fn top_inset() -> f32 {
    if cfg!(target_os = "macos") {
        SIZE_TITLEBAR_MACOS
    } else {
        0.0
    }
}

fn rail_item(
    item: RailItem,
    index: usize,
    model: &ShellModel,
    state: &RailState,
    shell: &gpui::WeakEntity<Shell>,
    theme: &Theme,
) -> AnyElement {
    let handle = &state.handles[index];
    let tab_stop = index == state.roving;
    match item {
        RailItem::Mode => {
            let mode = model.navigation().mode;
            let slot = RailSlot {
                id: "rail-mode".into(),
                handle,
                tab_stop,
                active: true,
                name: mode.label().into(),
                label: mode.switch_tooltip().into(),
            };
            let shell = shell.clone();
            rail_button(
                slot,
                mode_content(state, mode, theme),
                move |_, cx| {
                    let reduced = cx.reduce_motion();
                    change(&shell, cx, move |shell| shell.press_mode(reduced));
                },
                theme,
            )
        }
        RailItem::Entry(entry) => {
            let target = entry.page();
            let active = entry.shows(model.navigation().page);
            let slot = RailSlot {
                id: SharedString::from(format!("rail-{}", entry.label().to_lowercase())),
                handle,
                tab_stop,
                active,
                name: entry.label().into(),
                label: entry.label().into(),
            };
            let shell = shell.clone();
            rail_button(
                slot,
                rail_icon(entry_icon(entry), active, theme),
                move |_, cx| change(&shell, cx, move |shell| shell.model.open_page(target)),
                theme,
            )
        }
        RailItem::Theme => {
            let dark = theme.is_dark();
            let icon = if dark { IconName::Sun } else { IconName::Moon };
            let slot = RailSlot {
                id: "rail-theme".into(),
                handle,
                tab_stop,
                active: false,
                name: theme_label(dark).into(),
                label: theme_label(dark).into(),
            };
            rail_button(
                slot,
                rail_icon(icon, false, theme),
                |_, cx| Theme::toggle(cx),
                theme,
            )
        }
        RailItem::Usage(position) => {
            let usage = &model.snapshot().usage[position];
            let open = model.usage_popover() == Some(position);
            usage_ring(usage, position, open, handle, tab_stop, shell, theme)
        }
    }
}

/// What a rail item needs besides its content.
struct RailSlot<'a> {
    id: SharedString,
    handle: &'a FocusHandle,
    tab_stop: bool,
    active: bool,
    /// The accessible name: the current state, such as "Workspaces mode".
    name: SharedString,
    /// The tooltip: what a press does, such as "Switch to Bot Teams".
    label: SharedString,
}

/// A 36 px rail button: state fills, the active bar, keyboard focus and the tooltip.
fn rail_button(
    slot: RailSlot,
    content: AnyElement,
    activate: impl Fn(&mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    let fills = Surface::Frame.fills(theme);
    let selector = slot.id.to_string();
    let label = slot.label.clone();
    let name = slot.name.clone();
    let mut item = div()
        .id(slot.id.clone())
        .group("rail-item")
        .track_focus(slot.handle)
        .tab_stop(slot.tab_stop)
        .focus_ring(theme)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(SIZE_RAIL_BUTTON))
        .rounded(px(RADIUS_MD))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(name)
        .debug_selector(move || selector)
        .active(move |style| style.bg(fills.pressed))
        .on_click(move |_, window, cx| activate(window, cx))
        .child(content);
    if slot.active {
        item = item.bg(fills.selected).child(active_bar(theme));
    } else {
        item = item.hover(move |style| style.bg(fills.hover));
    }
    with_tooltip(item, label).into_any_element()
}

/// The 2 px accent bar on the rail's left edge, inset from the top and bottom of the item.
fn active_bar(theme: &Theme) -> impl IntoElement {
    div()
        .absolute()
        .left(px(-(SPACE_1 + SPACE_HALF)))
        .top(px(SPACE_2))
        .bottom(px(SPACE_2))
        .w(px(BORDER_WIDTH_THICK))
        .rounded(px(RADIUS_XS))
        .bg(theme.palette.accent.fg)
}

/// A static rail icon: secondary at rest, primary when active or hovered.
fn rail_icon(name: IconName, active: bool, theme: &Theme) -> AnyElement {
    let rest = if active {
        theme.palette.text.primary
    } else {
        theme.palette.text.secondary
    };
    let hover = theme.palette.text.primary;
    svg()
        .path(name.path())
        .size(px(SIZE_ICON_MD))
        .flex_none()
        .text_color(rest)
        .group_hover("rail-item", move |style| style.text_color(hover))
        .into_any_element()
}

/// The mode icon, or the two icons of a morph in progress stacked in one box.
fn mode_content(state: &RailState, mode: Mode, theme: &Theme) -> AnyElement {
    let Some(morph) = state.morph else {
        return rail_icon(mode_icon(mode), true, theme);
    };
    div()
        .relative()
        .flex_none()
        .size(px(SIZE_ICON_MD))
        .child(morph_icon(
            mode_icon(morph.from),
            morph.generation,
            "out",
            outgoing,
            theme,
        ))
        .child(morph_icon(
            mode_icon(mode),
            morph.generation,
            "in",
            incoming,
            theme,
        ))
        .into_any_element()
}

/// One icon of the morph, animated over `DURATION_MODERATE` on the standard curve. The frame
/// function maps the eased progress to scale, turn and opacity.
fn morph_icon(
    icon: IconName,
    generation: u64,
    side: &'static str,
    frame: fn(f32) -> MorphFrame,
    theme: &Theme,
) -> impl IntoElement {
    let id = SharedString::from(format!("mode-morph-{side}-{generation}"));
    svg()
        .absolute()
        .top_0()
        .left_0()
        .path(icon.path())
        .size(px(SIZE_ICON_MD))
        .text_color(theme.palette.text.primary)
        .with_animation(
            id,
            Animation::new(DURATION_MODERATE).with_easing(|t| ease(EASE_STANDARD, t)),
            move |svg, progress| {
                let f = frame(progress);
                svg.with_transformation(
                    Transformation::rotate(Radians(f.angle_deg.to_radians()))
                        .with_scaling(size(f.scale, f.scale))
                        .with_alpha(f.alpha),
                )
            },
        )
}

/// One provider's usage ring, in a focusable 36 px item. A click, Enter or Space opens its usage
/// popover, which closes on a second press, Escape or a click outside (D-SH-15).
fn usage_ring(
    usage: &UsageModel,
    position: usize,
    open: bool,
    handle: &FocusHandle,
    tab_stop: bool,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> AnyElement {
    let kind = agent_kind(usage.agent);
    let reading = usage_reading(usage.status);
    let resets = (!usage.resets_in.is_empty()).then_some(usage.resets_in.as_str());
    let name = usage_name(kind.label(), reading, resets, None);
    let mut ring = UsageRing::new(kind, reading);
    if let Some(resets) = resets {
        ring = ring.resets_in(resets.to_owned());
    }
    if let Some(slot) = UsageSlot::new(usage.slot) {
        ring = ring.slot(slot);
    }
    let press = shell.clone();
    let key_press = shell.clone();
    let trigger = div()
        .id(SharedString::from(format!("rail-usage-{position}")))
        .debug_selector(move || format!("rail-usage-{position}"))
        .group("rail-item")
        .track_focus(handle)
        .tab_stop(tab_stop)
        .focus_ring(theme)
        .role(Role::Button)
        .aria_label(name)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(SIZE_RAIL_BUTTON))
        .rounded(px(RADIUS_MD))
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            change(&press, cx, move |shell| {
                shell.model.toggle_usage_popover(position);
            });
        })
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                change(&key_press, cx, move |shell| {
                    shell.model.toggle_usage_popover(position);
                });
                cx.stop_propagation();
            }
        })
        .child(ring);
    let dismiss = shell.clone();
    Popover::new(("usage-popover", position), trigger)
        .open(open)
        .variant(PopoverVariant::Info)
        .width(PopoverWidth::Small)
        .placement(PopoverPlacement::Above)
        .title("Usage")
        .trigger_focus(handle.clone())
        .on_dismiss(move |_, cx| change(&dismiss, cx, |shell| shell.model.close_usage_popover()))
        .child(usage_popover_body(usage, theme))
        .into_any_element()
}

/// The popover's rows: the agent, the reading or the state it is in, when the window resets, and
/// the plan (D-SH-15).
fn usage_popover_body(usage: &UsageModel, theme: &Theme) -> Div {
    let kind = agent_kind(usage.agent);
    let agent = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPACE_2))
        .text_style(TYPE_BODY)
        .text_color(theme.palette.text.primary)
        .child(AgentIcon::new(kind).size(AgentIconSize::Small))
        .child(kind.label());
    let body = div().flex().flex_col().gap(px(SPACE_1)).child(agent);
    let body = match usage.status.headline() {
        Some(headline) => body.child(
            div()
                .text_style(TYPE_BODY)
                .text_color(theme.palette.text.primary)
                .child(headline),
        ),
        None => body
            .child(Skeleton::line(LineWidth::Full))
            .child(Skeleton::line(LineWidth::Partial)),
    };
    if !matches!(usage.status, UsageStatus::Percent(_)) {
        return body;
    }
    let body = if usage.resets_in.is_empty() {
        body
    } else {
        body.child(usage_row(format!("Resets in {}", usage.resets_in), theme))
    };
    // D-SH-15 lists a Plan row; the usage data does not carry the plan name yet.
    body.child(usage_row("Plan not reported", theme))
}

/// One caption row of the usage popover: "Resets in 2 days", "Plan" (D-SH-15).
fn usage_row(text: impl IntoElement, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(text)
}

/// The ring's reading for the status: a percentage draws its arc; the others draw none.
fn usage_reading(status: UsageStatus) -> UsageReading {
    match status {
        UsageStatus::Percent(percent) => UsageReading::Percent(f32::from(percent)),
        UsageStatus::Loading => UsageReading::Loading,
        UsageStatus::NotReported => UsageReading::NotReported,
        UsageStatus::Unavailable => UsageReading::Unavailable,
        UsageStatus::Failed { last } => UsageReading::Failed {
            last: last.map(f32::from),
        },
    }
}

/// The mark of each agent: its own icon, never a letter (A16, A94).
pub(super) fn agent_kind(agent: Agent) -> AgentKind {
    match agent {
        Agent::Claude => AgentKind::Claude,
        Agent::Codex => AgentKind::Codex,
        Agent::Antigravity => AgentKind::Antigravity,
        Agent::GentleShell => AgentKind::GentleShell,
        Agent::Gemini => AgentKind::Gemini,
        Agent::OpenCode => AgentKind::OpenCode,
        Agent::Cursor => AgentKind::Cursor,
    }
}

fn mode_icon(mode: Mode) -> IconName {
    match mode {
        Mode::Workspaces => IconName::LayoutGrid,
        Mode::BotTeams => IconName::Bot,
    }
}

fn entry_icon(entry: RailEntry) -> IconName {
    match entry {
        RailEntry::Tasks => IconName::ListChecks,
        RailEntry::Calendar => IconName::Calendar,
        RailEntry::Marketplace => IconName::Store,
        RailEntry::Settings => IconName::Settings,
    }
}
