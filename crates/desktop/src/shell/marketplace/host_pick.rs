//! Which host the Marketplace is aimed at, and the answer it waits for. A read or change for a paired host carries a
//! request number; only the answer to the newest request for the picked host is shown, so a late answer never
//! replaces a newer pick.

use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, Subscription, div,
    prelude::*, px,
};
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::tokens::{SPACE_2, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use super::MarketplaceFrame;
use crate::model::marketplace::{
    HostTarget, MarketplaceItemId, MarketplaceSnapshot, MarketplaceSource, ServiceMarketplace,
};
use crate::model::navigation::{MarketplacePage, Page};
use crate::service::{HostListing, HostLists, HostMarketplaceRead, StoreState};
use crate::shell::Shell;
use crate::ui_state;

/// The width of the host picker's select.
const HOST_PICKER_WIDTH: f32 = 240.0;

/// The host the Marketplace is aimed at, and the number of the newest request sent for it.
#[derive(Debug, Default)]
pub(in crate::shell) struct HostPick {
    target: HostTarget,
    requested: u64,
    issued: u64,
}

impl HostPick {
    /// The pick a start begins with: the host saved last, or This computer when none was saved.
    pub(in crate::shell) fn restored(target: HostTarget) -> Self {
        Self {
            target,
            ..Self::default()
        }
    }

    pub(in crate::shell) fn target(&self) -> &HostTarget {
        &self.target
    }

    /// Aims the Marketplace at `target`; returns the number the read for it carries.
    pub(in crate::shell) fn aim(&mut self, target: HostTarget) -> u64 {
        self.target = target;
        self.request()
    }

    /// A new request for the picked host, such as a refresh after a change; the older answer is no longer awaited.
    pub(in crate::shell) fn request(&mut self) -> u64 {
        self.issued += 1;
        self.requested = self.issued;
        self.requested
    }

    /// Whether an answer is the one the picked host is waiting for.
    pub(in crate::shell) fn awaits(&self, answer: &HostMarketplaceRead) -> bool {
        matches!(&self.target, HostTarget::Paired(name) if *name == answer.host)
            && answer.request == self.requested
    }
}

/// The answer the picked paired host is waiting for, once it is in.
fn awaited<'a>(state: &'a StoreState, pick: &HostPick) -> Option<&'a HostMarketplaceRead> {
    match pick.target() {
        HostTarget::Paired(name) => state
            .host_listings
            .get(name)
            .filter(|read| pick.awaits(read)),
        HostTarget::Local => None,
    }
}

/// The note above a paired host's items while its answer is not in: checking, update first, or the refusal.
pub(in crate::shell) fn host_note(name: &str, listing: Option<&HostListing>) -> Option<String> {
    match listing {
        None => Some(format!("Checking {name}…")),
        Some(HostListing::Lists(_)) => None,
        Some(HostListing::UpdateNeeded) => Some(format!("Update MassOrca on {name} first.")),
        Some(HostListing::Refused(refusal)) => Some(refusal.sentence()),
    }
}

/// The target the Marketplace shows: the pick while its host is offered, otherwise This computer. The pick is kept, so it
/// returns when its host connects again.
pub(in crate::shell) fn effective_target(pick: &HostTarget, offers: &[HostTarget]) -> HostTarget {
    if offers.contains(pick) {
        pick.clone()
    } else {
        HostTarget::Local
    }
}

/// The labels of the picker's options, in the order of the offers: This computer, then each paired host by its name.
pub(in crate::shell) fn offer_labels(offers: &[HostTarget]) -> Vec<String> {
    offers
        .iter()
        .map(|target| match target {
            HostTarget::Local => "This computer".to_owned(),
            HostTarget::Paired(name) => name.clone(),
        })
        .collect()
}

/// The status word beside the picker, worded as pv96 words the picked host: Connected, Checking or Could not check.
pub(in crate::shell) fn pick_status(
    target: &HostTarget,
    listing: Option<&HostListing>,
) -> &'static str {
    match (target, listing) {
        (HostTarget::Local, _) | (HostTarget::Paired(_), Some(HostListing::Lists(_))) => {
            "Connected"
        }
        (HostTarget::Paired(_), None) => "Checking",
        (HostTarget::Paired(_), Some(_)) => "Could not check",
    }
}

/// The picker over the offers: its options are the offers, its selection the host shown, and a choice picks that host.
pub(in crate::shell) fn host_picker_for(
    offers: &[HostTarget],
    shown: &HostTarget,
    cx: &mut Context<Shell>,
) -> (Entity<SelectView>, Subscription) {
    let selected = offers.iter().position(|target| target == shown);
    let select = cx.new(|cx| SelectView::new(offer_labels(offers), cx).selected(selected));
    let changes = cx.subscribe(&select, |shell, _, event: &SelectEvent, cx| {
        let SelectEvent::Changed(index) = event;
        if let Some(target) = shell.marketplace_offers.get(*index).cloned() {
            shell.pick_marketplace_host(target, cx);
        }
    });
    (select, changes)
}

/// The picker row: the select, and beside it the status word of the host it shows.
pub(super) fn host_picker_row(frame: &MarketplaceFrame, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    div()
        .debug_selector(|| "marketplace-host".to_owned())
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .child(
            div()
                .w(px(HOST_PICKER_WIDTH))
                .child(frame.host_picker.clone()),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(frame.host_status),
        )
        .into_any_element()
}

/// The snapshot of a paired host's answered lists, shown under the host's name.
fn paired_snapshot(name: &str, lists: &HostLists) -> MarketplaceSnapshot {
    let mut source = ServiceMarketplace::of_paired_host(name);
    source.follow_agent_kinds(&lists.kinds);
    source.follow_extensions(&lists.installed, &lists.catalogue);
    source.snapshot()
}

impl Shell {
    /// Shows the shown host's snapshot: This computer's, or a paired host's lists once its answer is in.
    pub(in crate::shell) fn show_marketplace(&mut self, cx: &App) {
        let snapshot =
            match effective_target(self.marketplace_pick.target(), &self.marketplace_offers) {
                HostTarget::Local => self.marketplace.snapshot(),
                HostTarget::Paired(name) => {
                    match awaited(self.store.read(cx).state(), &self.marketplace_pick) {
                        Some(HostMarketplaceRead {
                            listing: HostListing::Lists(lists),
                            ..
                        }) => paired_snapshot(&name, lists),
                        _ => MarketplaceSnapshot::default(),
                    }
                }
            };
        self.model.set_marketplace(snapshot);
    }

    /// The note above the shown paired host's items while its answer is pending or refused.
    pub(in crate::shell) fn picked_note(&self, cx: &App) -> Option<String> {
        match effective_target(self.marketplace_pick.target(), &self.marketplace_offers) {
            HostTarget::Local => None,
            HostTarget::Paired(name) => host_note(
                &name,
                awaited(self.store.read(cx).state(), &self.marketplace_pick)
                    .map(|read| &read.listing),
            ),
        }
    }

    /// The paired host an item runs on; None for an item of This computer.
    pub(super) fn paired_host_of(&self, item: MarketplaceItemId) -> Option<String> {
        let found = self.model.marketplace().item(item)?;
        match HostTarget::from_key(&found.host.0)? {
            HostTarget::Paired(name) => Some(name),
            HostTarget::Local => None,
        }
    }

    /// The status word beside the picker, for the host it shows.
    pub(in crate::shell) fn picker_status(&self, cx: &App) -> &'static str {
        let shown = effective_target(self.marketplace_pick.target(), &self.marketplace_offers);
        let listing =
            awaited(self.store.read(cx).state(), &self.marketplace_pick).map(|read| &read.listing);
        pick_status(&shown, listing)
    }

    /// Takes the hosts the service offers. The picker lists them; a pick of a paired host that is offered again is read.
    pub(in crate::shell) fn follow_host_offers(
        &mut self,
        offers: Vec<HostTarget>,
        cx: &mut Context<Self>,
    ) {
        if offers == self.marketplace_offers {
            return;
        }
        let pick = self.marketplace_pick.target().clone();
        if matches!(pick, HostTarget::Paired(_))
            && !self.marketplace_offers.contains(&pick)
            && offers.contains(&pick)
        {
            self.read_picked_host(pick);
        }
        self.marketplace_offers = offers;
        let shown = effective_target(self.marketplace_pick.target(), &self.marketplace_offers);
        let (select, changes) = host_picker_for(&self.marketplace_offers, &shown, cx);
        self.marketplace_host_select = select;
        self._marketplace_host_changes = changes;
        self.show_marketplace(cx);
        cx.notify();
    }

    /// Aims the Marketplace at `target`. A paired host is read under a new request number, so an answer to an earlier one
    /// is no longer awaited.
    fn read_picked_host(&mut self, target: HostTarget) {
        let request = self.marketplace_pick.aim(target.clone());
        if let (HostTarget::Paired(name), Some(service)) = (target, self.service.clone()) {
            service.read_host_marketplace(name, request);
        }
    }

    /// Selects the option of the host shown, so the picker matches the Marketplace.
    fn sync_picker(&mut self, cx: &mut Context<Self>) {
        let shown = effective_target(self.marketplace_pick.target(), &self.marketplace_offers);
        let index = self
            .marketplace_offers
            .iter()
            .position(|target| target == &shown);
        self.marketplace_host_select
            .update(cx, |select, cx| select.set_selected(index, cx));
    }

    /// Aims the Marketplace at This computer or a paired host. A paired host is read at once and its answer is shown when
    /// it comes; an item page of the host left behind goes back to Discover, since its items are another host's.
    pub(in crate::shell) fn pick_marketplace_host(
        &mut self,
        target: HostTarget,
        cx: &mut Context<Self>,
    ) {
        // Why: only a pick is saved; an offer change never overwrites the saved host, so a host that is gone returns.
        let saved = target.clone();
        ui_state::remember(cx, |state| state.marketplace_host = Some(saved));
        self.read_picked_host(target);
        if matches!(
            self.model.navigation().page,
            Page::Marketplace(MarketplacePage::Item(_))
        ) {
            self.open_marketplace(MarketplacePage::Discover, cx);
        }
        self.marketplace_error = None;
        self.marketplace_item_error = None;
        self.marketplace_review = None;
        self.marketplace_uninstall = None;
        self.show_marketplace(cx);
        self.sync_picker(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{HostLists, Refusal};

    fn paired(name: &str) -> HostTarget {
        HostTarget::Paired(name.to_owned())
    }

    fn answer(host: &str, request: u64) -> HostMarketplaceRead {
        HostMarketplaceRead {
            host: host.to_owned(),
            request,
            listing: HostListing::UpdateNeeded,
        }
    }

    fn no_lists() -> HostLists {
        HostLists {
            kinds: Vec::new(),
            installed: Vec::new(),
            catalogue: Vec::new(),
        }
    }

    #[test]
    fn a_pick_awaits_only_the_answer_to_its_newest_request() {
        let mut pick = HostPick::default();
        let first = pick.aim(paired("vps"));
        assert!(pick.awaits(&answer("vps", first)));
        let second = pick.aim(paired("nas"));
        assert!(
            !pick.awaits(&answer("vps", first)),
            "an answer for a host no longer picked is dropped"
        );
        assert!(pick.awaits(&answer("nas", second)));
    }

    #[test]
    fn an_answer_to_an_older_request_for_the_same_host_is_dropped() {
        let mut pick = HostPick::default();
        let read = pick.aim(paired("vps"));
        let refresh = pick.request();
        assert!(!pick.awaits(&answer("vps", read)));
        assert!(pick.awaits(&answer("vps", refresh)));
    }

    #[test]
    fn a_saved_host_is_the_pick_at_start_and_shows_this_computer_until_offered() {
        let pick = HostPick::restored(paired("vps"));
        assert_eq!(pick.target(), &paired("vps"));
        assert_eq!(
            effective_target(pick.target(), &[HostTarget::Local]),
            HostTarget::Local,
            "a saved host that is not connected is not overwritten"
        );
        assert_eq!(
            effective_target(pick.target(), &[HostTarget::Local, paired("vps")]),
            paired("vps")
        );
    }

    #[test]
    fn this_computer_awaits_no_host_answer() {
        let mut pick = HostPick::default();
        let number = pick.aim(HostTarget::Local);
        assert!(!pick.awaits(&answer("vps", number)));
        assert_eq!(pick.target(), &HostTarget::Local);
    }

    #[test]
    fn the_note_says_checking_until_the_answer_is_in() {
        assert_eq!(host_note("vps", None).as_deref(), Some("Checking vps…"));
    }

    #[test]
    fn a_host_that_predates_the_marketplace_is_asked_to_update_first() {
        assert_eq!(
            host_note("vps", Some(&HostListing::UpdateNeeded)).as_deref(),
            Some("Update MassOrca on vps first.")
        );
    }

    #[test]
    fn a_pick_that_is_not_offered_shows_this_computer_and_returns_when_offered_again() {
        assert_eq!(
            effective_target(&paired("vps"), &[HostTarget::Local]),
            HostTarget::Local
        );
        assert_eq!(
            effective_target(&paired("vps"), &[HostTarget::Local, paired("vps")]),
            paired("vps")
        );
    }

    #[test]
    fn the_picker_names_this_computer_and_each_offered_host() {
        assert_eq!(
            offer_labels(&[HostTarget::Local, paired("vps")]),
            ["This computer", "vps"]
        );
    }

    #[test]
    fn the_status_word_says_connected_checking_or_could_not_check() {
        assert_eq!(pick_status(&HostTarget::Local, None), "Connected");
        assert_eq!(pick_status(&paired("vps"), None), "Checking");
        assert_eq!(
            pick_status(&paired("vps"), Some(&HostListing::Lists(no_lists()))),
            "Connected"
        );
        assert_eq!(
            pick_status(&paired("vps"), Some(&HostListing::UpdateNeeded)),
            "Could not check"
        );
    }

    #[test]
    fn a_host_with_its_lists_has_no_note_and_a_refused_host_gives_its_sentence() {
        assert_eq!(
            host_note("vps", Some(&HostListing::Lists(no_lists()))),
            None
        );
        let refusal = Refusal::unavailable("The host is unreachable: timed out");
        let sentence = refusal.sentence();
        assert_eq!(
            host_note("vps", Some(&HostListing::Refused(refusal))),
            Some(sentence)
        );
    }
}
