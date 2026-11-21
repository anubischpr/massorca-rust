//! Gallery stories for the overlays group.

use super::{Story, variant};
use crate::components::button::{Button, ButtonVariant};
use crate::components::dialog::{Dialog, DialogVariant, DialogWidth};
use crate::components::menu::{MenuEntry, MenuItem, MenuView};
use crate::components::popover::OverlayLayer;
use crate::components::popover::{Popover, PopoverVariant, PopoverWidth};
use crate::components::select::SelectView;
use crate::components::skeleton::{LineWidth, Skeleton};
use crate::components::text_field::TextField;
use crate::components::tooltip::{Tooltip, TooltipTrigger};
use crate::icon::IconName;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{SPACE_2, TYPE_BODY_SM};
use gpui::{
    AnyElement, App, Focusable, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Popover",
            spec: "Popover.md",
            render: popover_story,
        },
        Story {
            name: "Tooltip",
            spec: "Tooltip.md",
            render: tooltip_story,
        },
        Story {
            name: "Menu",
            spec: "Menu.md",
            render: menu_story,
        },
        Story {
            name: "Select",
            spec: "Select.md",
            render: select_story,
        },
        Story {
            name: "Dialog",
            spec: "Dialog.md",
            render: dialog_story,
        },
    ]
}

/// Room under each trigger for its open panel, which floats over the gallery.
const SLOT_HEIGHT: f32 = 240.0;

fn popover_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let trigger = window
        .use_keyed_state("popover-story/trigger", cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();
    let field = window.use_keyed_state("popover-story/field", cx, |_, cx| {
        TextField::new(cx).placeholder("Project name")
    });
    let field_focus = field.read(cx).focus_handle(cx);
    let secondary = cx.theme().palette.text.secondary;

    let line = move |text: &'static str| div().text_body().text_color(secondary).child(text);

    let info = Popover::new(
        "popover-info",
        Button::new("popover-info-trigger", "Claude usage").focus_handle(trigger.clone()),
    )
    .open(true)
    .variant(PopoverVariant::Info)
    .width(PopoverWidth::Small)
    .title("Claude usage")
    .trigger_focus(trigger.clone())
    .child(line("5-hour window: 62 percent used"))
    .child(line("Resets in 2 days"));

    let form = Popover::new(
        "popover-form",
        Button::new("popover-form-trigger", "Rename project").focus_handle(trigger.clone()),
    )
    .open(true)
    .variant(PopoverVariant::Form)
    .title("Rename project")
    .trigger_focus(trigger.clone())
    .initial_focus(field_focus)
    .child(field.clone())
    .footer(
        div()
            .flex()
            .gap(px(SPACE_2))
            .child(Button::new("popover-form-cancel", "Cancel").variant(ButtonVariant::Secondary))
            .child(Button::new("popover-form-apply", "Apply").variant(ButtonVariant::Primary)),
    );

    let action = Popover::new(
        "popover-action",
        Button::new("popover-action-trigger", "Attach").focus_handle(trigger.clone()),
    )
    .open(true)
    .variant(PopoverVariant::Action)
    .width(PopoverWidth::Medium)
    .child(line("Attach an image or a file from disk"));

    let loading = Popover::new(
        "popover-loading",
        Button::new("popover-loading-trigger", "Activity").focus_handle(trigger.clone()),
    )
    .open(true)
    .variant(PopoverVariant::Info)
    .title("Activity")
    .child(Skeleton::line(LineWidth::Full))
    .child(Skeleton::line(LineWidth::Most))
    .child(Skeleton::line(LineWidth::Partial));

    let empty = Popover::new(
        "popover-empty",
        Button::new("popover-empty-trigger", "Usage").focus_handle(trigger.clone()),
    )
    .open(true)
    .variant(PopoverVariant::Info)
    .width(PopoverWidth::Small)
    .title("Claude usage")
    .child(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(secondary)
            .child("No usage reported yet"),
    );

    let closed = Popover::new(
        "popover-closed",
        Button::new("popover-closed-trigger", "Claude usage").focus_handle(trigger),
    )
    .open(false)
    .title("Claude usage")
    .child(line("Not rendered while closed"));

    div()
        .flex()
        .flex_wrap()
        .gap(px(SPACE_2))
        .child(variant("Closed (trigger only)", slot(closed), cx))
        .child(variant("Info, open", slot(info), cx))
        .child(variant("Form, focus in first field", slot(form), cx))
        .child(variant("Action, 320 px", slot(action), cx))
        .child(variant("Loading", slot(loading), cx))
        .child(variant("Empty", slot(empty), cx))
        .into_any_element()
}

/// A fixed-height slot so the open panel has room to float below its trigger.
fn slot(content: impl IntoElement) -> impl IntoElement {
    div().h(px(SLOT_HEIGHT)).child(content)
}

fn tooltip_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let label = window.use_keyed_state("tooltip-story/label", cx, |_, _| {
        Tooltip::new("New terminal")
    });
    let shortcut = window.use_keyed_state("tooltip-story/shortcut", cx, |_, _| {
        Tooltip::new("Command palette").shortcut("Mod+K")
    });
    let reason = window.use_keyed_state("tooltip-story/reason", cx, |_, _| {
        Tooltip::new("Host unreachable. Last known state, 5 min ago.")
    });
    let full = window.use_keyed_state("tooltip-story/full", cx, |_, _| {
        Tooltip::new("~/projects/NexusCare/oppm-api")
    });
    let long = window.use_keyed_state("tooltip-story/long", cx, |_, _| {
        Tooltip::new(
            "Opens the Board, Routines and Memory for this team, with every project the team uses on this host",
        )
    });
    let focus = window
        .use_keyed_state("tooltip-story/focus", cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();

    let trigger = TooltipTrigger::new(
        "tooltip-story/trigger",
        "Command palette",
        Button::new("tooltip-story/button", "Command palette")
            .variant(ButtonVariant::Secondary)
            .focus_handle(focus.clone()),
    )
    .shortcut("Mod+K")
    .focus_handle(focus);

    div()
        .flex()
        .flex_wrap()
        .gap(px(SPACE_2))
        .child(variant("Label", label, cx))
        .child(variant("Label and shortcut", shortcut, cx))
        .child(variant("Reason (disabled control)", reason, cx))
        .child(variant("Full text of a truncated value", full, cx))
        .child(variant("Long content, three lines", long, cx))
        .child(variant("Trigger: hover, or Tab to it", trigger, cx))
        .into_any_element()
}

fn menu_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let actions = window.use_keyed_state("menu-story/actions", cx, |window, cx| {
        MenuView::new(action_entries(), window, cx)
    });
    let submenu = window.use_keyed_state("menu-story/submenu", cx, |window, cx| {
        let mut menu = MenuView::new(action_entries(), window, cx);
        menu.open_submenu(SUBMENU_ROW, cx);
        menu
    });
    let checkable = window.use_keyed_state("menu-story/checkable", cx, |window, cx| {
        MenuView::new(checkable_entries(), window, cx)
    });
    let empty = window.use_keyed_state("menu-story/empty", cx, |window, cx| {
        MenuView::new(Vec::new(), window, cx)
    });

    div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap(px(SPACE_2))
        .child(variant(
            "Actions, icons, shortcut, disabled and destructive",
            actions,
            cx,
        ))
        .child(variant("Submenu open (Right or hover)", submenu, cx))
        .child(variant(
            "Checkable options and a group label",
            checkable,
            cx,
        ))
        .child(variant("Empty", empty, cx))
        .into_any_element()
}

/// Row of the action menu that opens a submenu in the gallery.
const SUBMENU_ROW: usize = 6;

fn action_entries() -> Vec<MenuEntry> {
    vec![
        MenuItem::new("Rename")
            .icon(IconName::Pencil)
            .shortcut("Mod+R")
            .into(),
        MenuItem::new("Duplicate").icon(IconName::Copy).into(),
        MenuItem::new("Open in editor")
            .icon(IconName::ExternalLink)
            .into(),
        MenuEntry::Separator,
        MenuItem::new("Stop agent")
            .icon(IconName::Zap)
            .disabled("Host unreachable")
            .into(),
        MenuEntry::Separator,
        MenuItem::new("Open with")
            .icon(IconName::SquareTerminal)
            .submenu(vec![
                MenuItem::new("Terminal").icon(IconName::Terminal).into(),
                MenuItem::new("Editor").icon(IconName::FileText).into(),
            ])
            .into(),
        MenuEntry::Separator,
        MenuItem::new("Delete workspace (uncommitted changes lost)")
            .icon(IconName::Trash2)
            .destructive()
            .into(),
    ]
}

fn checkable_entries() -> Vec<MenuEntry> {
    vec![
        MenuEntry::Group("Layout".into()),
        MenuItem::new("Activity rail").checked(true).into(),
        MenuItem::new("Sidebar").checked(true).into(),
        MenuItem::new("Status bar").checked(false).into(),
        MenuEntry::Separator,
        MenuItem::new("Wrap long lines").checked(false).into(),
        MenuItem::new("Reset layout").into(),
    ]
}

fn select_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let chosen = window.use_keyed_state("select-story/chosen", cx, |_, cx| {
        SelectView::new(branch_options(), cx).selected(Some(0))
    });
    let placeholder = window.use_keyed_state("select-story/placeholder", cx, |_, cx| {
        SelectView::new(branch_options(), cx).placeholder("Choose a branch")
    });
    let disabled = window.use_keyed_state("select-story/disabled", cx, |_, cx| {
        SelectView::new(branch_options(), cx)
            .selected(Some(1))
            .disabled(true)
    });
    let error = window.use_keyed_state("select-story/error", cx, |_, cx| {
        SelectView::new(branch_options(), cx)
            .placeholder("Choose a branch")
            .error("Choose a branch before you run the agent.")
    });
    let open = window.use_keyed_state("select-story/open", cx, |window, cx| {
        let mut select = SelectView::new(branch_options(), cx).selected(Some(1));
        select.open(window, cx);
        select
    });
    let empty = window.use_keyed_state("select-story/empty", cx, |window, cx| {
        let mut select = SelectView::new(Vec::<SharedString>::new(), cx)
            .placeholder("Choose a branch")
            .empty_text("No branches in this repo");
        select.open(window, cx);
        select
    });

    div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap(px(SPACE_2))
        .child(variant("Chosen value", chosen, cx))
        .child(variant("Placeholder", placeholder, cx))
        .child(variant("Disabled", disabled, cx))
        .child(variant("Error", error, cx))
        .child(variant("Open, value highlighted", slot(open), cx))
        .child(variant("Open, no options", slot(empty), cx))
        .into_any_element()
}

fn branch_options() -> Vec<SharedString> {
    ["main", "feature/overlays", "release/2026.11"]
        .into_iter()
        .map(SharedString::from)
        .collect()
}

fn dialog_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let field = window.use_keyed_state("dialog-story/field", cx, |_, cx| {
        TextField::new(cx).placeholder("Project name")
    });
    let field_focus = field.read(cx).focus_handle(cx);
    let secondary = cx.theme().palette.text.secondary;
    let body = move |text: &'static str| div().text_body().text_color(secondary).child(text);

    let confirm = Dialog::new("dialog-story/confirm", "Archive this workspace?")
        .open(true)
        .width(DialogWidth::Small)
        .child(body("Its sessions stop, and its worktree stays on disk."))
        .action("Archive")
        .inline();
    let destructive = Dialog::new("dialog-story/destructive", "Delete secret DB_PASSWORD?")
        .open(true)
        .variant(DialogVariant::Destructive)
        .child(body(
            "This removes the secret from 3 projects and 1 team. Agents that use it lose access. This cannot be undone.",
        ))
        .loss(["3 projects", "1 team", "2 routines stop"])
        .action("Delete secret (3 projects use it)")
        .inline();
    let long_loss = Dialog::new("dialog-story/long-loss", "Remove project NexusCare?")
        .open(true)
        .variant(DialogVariant::Destructive)
        .child(body(
            "Every session and file in these places is removed. This cannot be undone.",
        ))
        .loss([
            "8 workspaces",
            "21 sessions",
            "4 routines stop",
            "2 teams lose their board",
            "1 design system",
            "3 secrets lose a project",
            "5 saved searches",
        ])
        .action("Remove project")
        .inline();
    let error = Dialog::new("dialog-story/error", "Could not delete")
        .open(true)
        .variant(DialogVariant::Error)
        .child(body("Could not delete. The secret is still there."))
        .error("Host unreachable. Nothing was deleted.")
        .action("Retry")
        .inline();
    let one_field = Dialog::new("dialog-story/one-field", "Rename project")
        .open(true)
        .variant(DialogVariant::OneField)
        .field(field_focus)
        .child(field.clone())
        .action("Rename")
        .inline();
    let dialog_select = window.use_keyed_state("dialog-story/select", cx, |window, cx| {
        let mut select = SelectView::new(branch_options(), cx)
            .selected(Some(1))
            .layer(OverlayLayer::Dialog);
        select.open(window, cx);
        select
    });
    let select_dialog = Dialog::new("dialog-story/select-dialog", "Create workspace")
        .open(true)
        .child(body("Pick the branch the workspace starts from."))
        .child(dialog_select.clone())
        .action("Create")
        .inline();
    let busy = Dialog::new("dialog-story/busy", "Delete secret DB_PASSWORD?")
        .open(true)
        .variant(DialogVariant::Destructive)
        .child(body("Deleting the secret from every project."))
        .action("Deleting")
        .busy(true)
        .inline();

    div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap(px(SPACE_2))
        .child(variant("Confirm", confirm, cx))
        .child(variant("Destructive, names the loss", destructive, cx))
        .child(variant("Destructive, long loss list", long_loss, cx))
        .child(variant("Error", error, cx))
        .child(variant("One field", one_field, cx))
        .child(variant(
            "Select in a dialog, menu on the dialog layer",
            slot(select_dialog),
            cx,
        ))
        .child(variant("Busy", busy, cx))
        .into_any_element()
}
