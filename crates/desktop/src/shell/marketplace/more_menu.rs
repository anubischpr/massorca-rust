//! The menu a Marketplace control opens at the pointer. Its state lives in the control's keyed state, so the Shell keeps no
//! field for it: an installed row's More button and the Add extension split button both open one of these menus.

use crate::shell::Shell;
use gpui::{AnyElement, App, AppContext, Entity, Pixels, Point, Subscription, WeakEntity, Window};
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuOrigin, MenuView, menu_overlay};
use massorca_ui::components::popover::OverlayLayer;

/// The menu open on one control: the menu, where it opened, and the subscription that answers its choice.
#[derive(Debug)]
struct OpenMenu {
    menu: Entity<MenuView>,
    at: Point<Pixels>,
    _subscription: Subscription,
}

/// The menu state of one control, kept in that control's keyed state.
#[derive(Debug, Default)]
pub(super) struct MoreMenu {
    open: Option<OpenMenu>,
}

impl MoreMenu {
    /// The overlay of the menu open on this control, if one is open.
    pub(super) fn overlay(state: &Entity<Self>, cx: &mut App) -> Option<AnyElement> {
        let (view, at) = state
            .read(cx)
            .open
            .as_ref()
            .map(|open| (open.menu.clone(), open.at))?;
        Some(menu_overlay(&view, MenuOrigin::At(at), cx))
    }
}

/// Opens `entries` at the pointer. A choice closes the menu and then runs `on_choose` with its row; a dismissal only
/// closes it. Either way the Shell redraws.
pub(super) fn open(
    state: &Entity<MoreMenu>,
    shell: WeakEntity<Shell>,
    entries: Vec<MenuEntry>,
    on_choose: impl Fn(usize, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let view = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
    let at = window.mouse_position();
    let on_answer = shell.clone();
    state.update(cx, |state, cx| {
        let subscription = cx.subscribe_in(
            &view,
            window,
            move |state: &mut MoreMenu, _, event: &MenuEvent, window, cx| {
                // Taking the menu closes it, on a choice or a dismissal alike.
                let _closed = state.open.take();
                if let MenuEvent::Chosen(choice) = event {
                    on_choose(choice.row, window, cx);
                }
                let _ = on_answer.update(cx, |_, cx| cx.notify());
            },
        );
        state.open = Some(OpenMenu {
            menu: view,
            at,
            _subscription: subscription,
        });
        cx.notify();
    });
    let _ = shell.update(cx, |_, cx| cx.notify());
}
