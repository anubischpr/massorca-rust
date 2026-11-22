//! The Marketplace pages: the catalogue and its views (D-EX-01, 7.4). Installs and agent accounts go
//! through the source trait, and the snapshot always follows it.

mod catalogue;

use super::Shell;
use crate::model::marketplace::{
    MarketplaceItemId, MarketplaceSnapshot, MarketplaceSource, SourceError,
};
use crate::model::navigation::{MarketplacePage, Page};
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_ui::components::search_field::SearchField;
use massorca_ui::tokens::SPACE_6;

/// What the Marketplace pages draw for one frame, gathered by the shell.
pub(super) struct MarketplaceFrame {
    snapshot: MarketplaceSnapshot,
    page: MarketplacePage,
    query: String,
    search: Entity<SearchField>,
    shell: WeakEntity<Shell>,
}

/// The frame of a Marketplace page, read from the shell.
pub(super) fn frame(
    shell: &Shell,
    page: MarketplacePage,
    weak: WeakEntity<Shell>,
) -> MarketplaceFrame {
    MarketplaceFrame {
        snapshot: shell.model.marketplace().clone(),
        page,
        query: shell.model.marketplace_query().to_owned(),
        search: shell.marketplace_search.clone(),
        shell: weak,
    }
}

/// The Marketplace page: its left navigation, and the view beside it.
pub(super) fn page(frame: MarketplaceFrame, window: &mut Window, cx: &mut App) -> AnyElement {
    let nav = catalogue::nav_column(&frame, window, cx);
    let content = catalogue::catalogue_view(&frame, frame.page, cx);
    div()
        .size_full()
        .flex()
        .flex_row()
        .min_h_0()
        .child(nav)
        .child(
            div()
                .id("marketplace-content")
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(SPACE_6))
                .child(content),
        )
        .into_any_element()
}

impl Shell {
    /// Opens a Marketplace view from anywhere in the shell.
    pub(super) fn open_marketplace(&mut self, page: MarketplacePage, cx: &mut Context<Self>) {
        self.model.open_page(Page::Marketplace(page));
        cx.notify();
    }

    /// Applies one change to the source, then shows its snapshot again. The refusal is returned.
    fn change_marketplace(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut dyn MarketplaceSource) -> Result<(), SourceError>,
    ) -> Result<(), SourceError> {
        let result = change(self.marketplace.as_mut());
        self.model.set_marketplace(self.marketplace.snapshot());
        cx.notify();
        result
    }

    pub(super) fn set_marketplace_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.model.set_marketplace_query(query);
        cx.notify();
    }

    /// The Switch of an installed item. Only the sample changes; the service is not connected.
    pub(super) fn set_marketplace_enabled(
        &mut self,
        item: MarketplaceItemId,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let _ = self.change_marketplace(cx, |source| source.set_enabled(item, enabled));
    }
}
