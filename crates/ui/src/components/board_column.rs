//! One column of a team Board and its cards (D-WS-30, BoardColumn.md). The column is a list named
//! by its title and count; each card is a list item. Moving cards is the caller's job: it passes a
//! key handler to each card and draws the lifted card through `lifted`.

use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::badge::{Badge, BadgeTone};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled, Theme, box_shadows};
use crate::tokens::{
    LAYOUT_BOARD_COLUMN_WIDTH, RADIUS_LG, RADIUS_SM, SIZE_CONTROL_COMFORTABLE, SPACE_1, SPACE_2,
    SPACE_HALF, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION, TYPE_TITLE_SM,
};
use std::fmt;

use gpui::{
    AnyElement, App, ElementId, FocusHandle, IntoElement, KeyDownEvent, ParentElement, RenderOnce,
    Role, SharedString, Styled, Window, div, prelude::*, px,
};

/// The text of an empty column (BoardColumn.md, copy).
pub const EMPTY_COLUMN_TEXT: &str = "No tasks";

/// 10 px: no token gives it, so it is SPACE_2 + SPACE_HALF (bt-board `.bcard`).
const CARD_PADDING: f32 = SPACE_2 + SPACE_HALF;

/// 6 px: no token gives it, so it is SPACE_1 + SPACE_HALF (bt-board `.bcard`).
const CARD_GAP: f32 = SPACE_1 + SPACE_HALF;

/// A card's key handler, which the Board uses to move cards.
type CardKeys = Box<dyn Fn(&KeyDownEvent, &mut Window, &mut App)>;

/// One card: its title, the bot working on it, a status with its word, the time and a note.
#[derive(IntoElement)]
pub struct BoardCard {
    id: ElementId,
    title: SharedString,
    focus: FocusHandle,
    bot: Option<SharedString>,
    /// No bot works on the card: its meta row shows the Unassigned chip.
    unassigned: bool,
    status: Option<(AgentState, SharedString)>,
    time: Option<SharedString>,
    note: Option<SharedString>,
    unavailable: Option<SharedString>,
    unreachable: Option<SharedString>,
    lifted: bool,
    /// Drawn as the dashed slot the lifted card left behind.
    slot: bool,
    on_key: Option<CardKeys>,
}

impl fmt::Debug for BoardCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoardCard")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("lifted", &self.lifted)
            .finish_non_exhaustive()
    }
}

impl BoardCard {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        focus: FocusHandle,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            focus,
            bot: None,
            unassigned: false,
            status: None,
            time: None,
            note: None,
            unavailable: None,
            unreachable: None,
            lifted: false,
            slot: false,
            on_key: None,
        }
    }

    /// The bot's name, after the status.
    pub fn bot(mut self, name: impl Into<SharedString>) -> Self {
        self.bot = Some(name.into());
        self
    }

    /// No bot works on the card: its meta row shows an "Unassigned" chip.
    pub fn unassigned(mut self) -> Self {
        self.unassigned = true;
        self
    }

    /// The agent state glyph and its word ("Running", "Waiting for you").
    pub fn status(mut self, state: AgentState, word: impl Into<SharedString>) -> Self {
        self.status = Some((state, word.into()));
        self
    }

    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// A line under the meta row, such as "Needs your reply in the Conference room".
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The bot's agent cannot run on this host: the card says so under the bot.
    pub fn unavailable(mut self, text: impl Into<SharedString>) -> Self {
        self.unavailable = Some(text.into());
        self
    }

    /// The bot's host is unreachable: the card shows its last known state, with its age.
    pub fn unreachable(mut self, last_known: impl Into<SharedString>) -> Self {
        self.unreachable = Some(last_known.into());
        self
    }

    /// The card is picked up for a move: it draws raised and over the column.
    pub fn lifted(mut self, lifted: bool) -> Self {
        self.lifted = lifted;
        self
    }

    /// Keys pressed while the card has focus; the Board reads them to move cards.
    pub fn on_key_down(
        mut self,
        listener: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_key = Some(Box::new(listener));
        self
    }

    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// The lines under the card's border: the title, one meta row, then the note and the
    /// unavailable line. The source slot draws the same lines, so it is as tall as the card.
    fn lines(&self, theme: &Theme) -> Vec<AnyElement> {
        let p = &theme.palette;
        let mut lines = vec![
            div()
                .debug_selector(|| "board-card-title".into())
                .text_style(TYPE_BODY_STRONG)
                .text_color(p.text.primary)
                .line_clamp(2)
                .child(self.title.clone())
                .into_any_element(),
        ];
        let mut meta_items = Vec::new();
        // An unreachable card shows only its last known state, never a live status.
        match (&self.status, &self.unreachable) {
            (_, Some(last_known)) => {
                meta_items.push(unreachable_line(last_known.clone(), theme).into_any_element());
            }
            (Some((state, word)), None) => {
                meta_items.push(status_line(*state, word.clone(), theme).into_any_element());
            }
            (None, None) => {}
        }
        if let Some(bot) = &self.bot {
            meta_items.push(
                div()
                    .debug_selector(|| "board-card-bot".into())
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child(bot.clone())
                    .into_any_element(),
            );
        } else if self.unassigned {
            meta_items.push(unassigned_chip(theme).into_any_element());
        }
        if let Some(time) = &self.time {
            meta_items.push(
                div()
                    .debug_selector(|| "board-card-time".into())
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.tertiary)
                    .child(time.clone())
                    .into_any_element(),
            );
        }
        // The status, the bot and the time share one row that wraps (bt-board `.bmeta`).
        if !meta_items.is_empty() {
            lines.push(
                div()
                    .debug_selector(|| "board-card-meta".into())
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(CARD_GAP))
                    .children(meta_items)
                    .into_any_element(),
            );
        }
        if let Some(note) = &self.note {
            lines.push(meta(note.clone(), theme).into_any_element());
        }
        if let Some(text) = &self.unavailable {
            lines.push(
                div()
                    .debug_selector(|| "board-card-unavailable".into())
                    .child(meta(text.clone(), theme))
                    .into_any_element(),
            );
        }
        lines
    }
}

impl RenderOnce for BoardCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let hover = p.fill.hover_raised;
        if self.slot {
            return source_slot(self, &theme).into_any_element();
        }
        // An unreachable card has a dashed `border.strong` outline (bt-board `.bcard.lost`).
        let unreachable = self.unreachable.is_some();
        let lines = self.lines(&theme);
        let mut card = div()
            .id(self.id.clone())
            .track_focus(&self.focus)
            .tab_stop(true)
            .focus_ring(&theme)
            .role(Role::ListItem)
            .aria_label(self.title.clone())
            .debug_selector(|| "board-card".into())
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .p(px(CARD_PADDING))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(if unreachable {
                p.border.strong
            } else {
                p.border.default
            })
            .bg(if self.lifted {
                p.surface.overlay
            } else {
                p.surface.raised
            })
            .shadow(box_shadows(if self.lifted {
                theme.shadows.e2
            } else {
                theme.shadows.e1
            }))
            .hover(move |style| style.bg(hover));
        if unreachable {
            card = card.border_dashed();
        }
        if let Some(listener) = self.on_key {
            card = card.on_key_down(move |event, window, cx| listener(event, window, cx));
        }
        card.children(lines).into_any_element()
    }
}

/// The source slot of a lifted card: a dashed `border.strong` outline the card's height, with
/// its lines kept for size and hidden (BoardColumn.md States, selected).
fn source_slot(card: BoardCard, theme: &Theme) -> impl IntoElement {
    let p = &theme.palette;
    let content = div()
        .invisible()
        .flex()
        .flex_col()
        .gap(px(CARD_GAP))
        .children(card.lines(theme));
    div()
        .id(card.id)
        .debug_selector(|| "board-source-slot".into())
        .p(px(CARD_PADDING))
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_dashed()
        .border_color(p.border.strong)
        .child(content)
}

fn meta(text: SharedString, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(text)
}

/// The agent's glyph and its word, the word in `caption` (bt-board `.stat`).
fn status_line(state: AgentState, word: SharedString, theme: &Theme) -> impl IntoElement {
    div()
        .debug_selector(|| "board-card-status".into())
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .child(AgentStatus::new(state).size(AgentStatusSize::XSmall))
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.secondary)
                .child(word),
        )
}

/// The "Unassigned" chip: the Badge's neutral tone in a `border.subtle` frame, because Badge has
/// no bordered neutral tone (bt-board `.chip`).
fn unassigned_chip(theme: &Theme) -> impl IntoElement {
    div()
        .debug_selector(|| "board-card-unassigned".into())
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(theme.palette.border.subtle)
        .child(Badge::text("Unassigned").tone(BadgeTone::Neutral))
}

/// The unreachable glyph and the last known state of a card whose host cannot be reached.
fn unreachable_line(last_known: SharedString, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.tertiary)
        .child(AgentStatus::new(AgentState::Unreachable).size(AgentStatusSize::XSmall))
        .child(last_known)
}

/// One Board column: its header with the title and the card count, then the cards, or
/// "No tasks" when there are none.
#[derive(Debug, IntoElement)]
pub struct BoardColumn {
    id: ElementId,
    title: SharedString,
    cards: Vec<BoardCard>,
    slot: Option<(usize, BoardCard)>,
}

impl BoardColumn {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            cards: Vec::new(),
            slot: None,
        }
    }

    pub fn card(mut self, card: BoardCard) -> Self {
        self.cards.push(card);
        self
    }

    pub fn cards(mut self, cards: impl IntoIterator<Item = BoardCard>) -> Self {
        self.cards.extend(cards);
        self
    }

    /// The slot a lifted card left at `index`, drawn dashed; it is not counted as a task.
    pub fn source_slot(mut self, index: usize, card: BoardCard) -> Self {
        self.slot = Some((index, card));
        self
    }

    /// The name a screen reader reads: "Running, 2 tasks".
    pub fn accessible_name(title: &str, count: usize) -> String {
        format!(
            "{title}, {count} {}",
            if count == 1 { "task" } else { "tasks" }
        )
    }
}

impl RenderOnce for BoardColumn {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let count = self.cards.len();
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(SIZE_CONTROL_COMFORTABLE))
            .px(px(SPACE_1))
            .child(
                div()
                    .text_style(TYPE_TITLE_SM)
                    .text_color(p.text.primary)
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.secondary)
                    .child(count.to_string()),
            );
        let mut cards = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .child(header);
        if self.cards.is_empty() && self.slot.is_none() {
            cards = cards.child(
                div()
                    .debug_selector(|| "board-empty".into())
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .py(px(SPACE_2))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(EMPTY_COLUMN_TEXT),
            );
        }
        let mut slot = self.slot;
        for (index, card) in self.cards.into_iter().enumerate() {
            if let Some((_, ghost)) = slot.take_if(|(at, _)| *at == index) {
                cards = cards.child(BoardCard {
                    slot: true,
                    ..ghost
                });
            }
            cards = cards.child(card);
        }
        if let Some((_, ghost)) = slot {
            cards = cards.child(BoardCard {
                slot: true,
                ..ghost
            });
        }
        div()
            .id(self.id.clone())
            .role(Role::List)
            .aria_label(Self::accessible_name(&self.title, count))
            .debug_selector(|| "board-column".into())
            .flex()
            .flex_col()
            .flex_none()
            .w(px(LAYOUT_BOARD_COLUMN_WIDTH))
            .h_full()
            .p(px(SPACE_2))
            .gap(px(SPACE_2))
            .rounded(px(RADIUS_LG))
            .bg(p.surface.sunken)
            .child(cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::SIZE_BADGE_HEIGHT;

    #[test]
    fn the_accessible_name_counts_tasks_in_the_plural_except_for_one() {
        assert_eq!(
            BoardColumn::accessible_name("Running", 2),
            "Running, 2 tasks"
        );
        assert_eq!(BoardColumn::accessible_name("Open", 1), "Open, 1 task");
        assert_eq!(BoardColumn::accessible_name("Done", 0), "Done, 0 tasks");
    }

    /// One column: empty, or holding only the dashed slot a lifted card left.
    struct Harness {
        slot: bool,
    }

    impl gpui::Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let mut column = BoardColumn::new("open", "Open");
            if self.slot {
                let card = BoardCard::new("slot", "Write release notes", cx.focus_handle());
                column = column.source_slot(0, card);
            }
            div().h(gpui::px(400.)).child(column)
        }
    }

    #[gpui::test]
    fn an_empty_column_centres_no_tasks(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Harness { slot: false });
        cx.run_until_parked();
        let empty = cx.debug_bounds("board-empty").expect("No tasks is drawn");
        assert!(empty.size.height > gpui::px(SIZE_CONTROL_COMFORTABLE * 2.));
    }

    /// One card on an open column, for the card's own lines.
    struct CardHarness;

    impl gpui::Render for CardHarness {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let card = BoardCard::new("card", "Write release notes", cx.focus_handle())
                .bot("Nora")
                .unavailable("Not available on this host");
            div()
                .h(gpui::px(400.))
                .child(BoardColumn::new("open", "Open").card(card))
        }
    }

    #[gpui::test]
    fn a_card_whose_bot_cannot_run_here_says_so(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| CardHarness);
        cx.run_until_parked();
        assert!(cx.debug_bounds("board-card-unavailable").is_some());
    }

    #[gpui::test]
    fn a_source_slot_replaces_the_empty_text(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Harness { slot: true });
        cx.run_until_parked();
        assert!(cx.debug_bounds("board-source-slot").is_some());
        assert!(cx.debug_bounds("board-empty").is_none());
    }

    /// A window with one open column, built by `column` from the window's own focus handles.
    struct Stage {
        column: fn(&mut gpui::App) -> BoardColumn,
    }

    impl gpui::Render for Stage {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            div().h(gpui::px(400.)).child((self.column)(cx))
        }
    }

    /// A running card with a bot and a time, whose status, bot and time share one row.
    fn running_card(cx: &mut gpui::App) -> BoardColumn {
        let card = BoardCard::new("card", "Write release notes", cx.focus_handle())
            .status(AgentState::Running, "Running")
            .bot("Nora")
            .time("09:42");
        BoardColumn::new("open", "Open").card(card)
    }

    /// An open card that no bot works on.
    fn unassigned_card(cx: &mut gpui::App) -> BoardColumn {
        let card = BoardCard::new("card", "Write release notes", cx.focus_handle()).unassigned();
        BoardColumn::new("open", "Open").card(card)
    }

    /// A lifted card's source slot, beside the same card drawn in its place.
    fn slot_beside_its_card(cx: &mut gpui::App) -> BoardColumn {
        let slot = BoardCard::new("slot", "Write release notes", cx.focus_handle())
            .status(AgentState::Running, "Running")
            .bot("Nora")
            .time("09:42");
        let card = BoardCard::new("card", "Write release notes", cx.focus_handle())
            .status(AgentState::Running, "Running")
            .bot("Nora")
            .time("09:42");
        BoardColumn::new("open", "Open")
            .source_slot(0, slot)
            .card(card)
    }

    /// The edges and size of a measured box, in pixels.
    fn left(bounds: gpui::Bounds<gpui::Pixels>) -> f32 {
        f32::from(bounds.origin.x)
    }

    fn top(bounds: gpui::Bounds<gpui::Pixels>) -> f32 {
        f32::from(bounds.origin.y)
    }

    fn width(bounds: gpui::Bounds<gpui::Pixels>) -> f32 {
        f32::from(bounds.size.width)
    }

    fn height(bounds: gpui::Bounds<gpui::Pixels>) -> f32 {
        f32::from(bounds.size.height)
    }

    /// The height of the middle of a measured box, in pixels.
    fn middle(bounds: gpui::Bounds<gpui::Pixels>) -> f32 {
        top(bounds) + height(bounds) / 2.
    }

    /// Asserts a measured length, in pixels, against the expected one within layout rounding.
    fn assert_px(measured: f32, expected: f32, what: &str) {
        assert!(
            (measured - expected).abs() < 0.01,
            "{what}: measured {measured} px, expected {expected} px"
        );
    }

    #[gpui::test]
    fn a_card_is_padded_ten_px_and_its_title_sits_six_px_above_the_meta_row(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_, cx) = cx.add_window_view(|_, _| Stage {
            column: running_card,
        });
        cx.run_until_parked();
        let card = cx.debug_bounds("board-card").expect("the card is drawn");
        let title = cx
            .debug_bounds("board-card-title")
            .expect("the title is drawn");
        let meta = cx
            .debug_bounds("board-card-meta")
            .expect("the meta row is drawn");
        // The 1 px border and the 10 px padding inset the title from the card's left edge.
        assert_px(
            left(title) - left(card),
            11.,
            "the title's inset in the card",
        );
        assert_px(
            top(meta) - (top(title) + height(title)),
            6.,
            "the gap under the title",
        );
    }

    #[gpui::test]
    fn a_column_pads_its_cards_eight_px(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Stage {
            column: running_card,
        });
        cx.run_until_parked();
        let column = cx
            .debug_bounds("board-column")
            .expect("the column is drawn");
        let card = cx.debug_bounds("board-card").expect("the card is drawn");
        assert_px(
            left(card) - left(column),
            8.,
            "the card's inset in its column",
        );
    }

    #[gpui::test]
    fn the_status_bot_and_time_share_one_row_six_px_apart(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Stage {
            column: running_card,
        });
        cx.run_until_parked();
        let status = cx
            .debug_bounds("board-card-status")
            .expect("the status is drawn");
        let bot = cx.debug_bounds("board-card-bot").expect("the bot is drawn");
        let time = cx
            .debug_bounds("board-card-time")
            .expect("the time is drawn");
        assert_px(
            middle(bot),
            middle(status),
            "the bot is on the status's line",
        );
        assert_px(
            middle(time),
            middle(status),
            "the time is on the status's line",
        );
        assert_px(
            left(bot) - (left(status) + width(status)),
            6.,
            "the gap after the status",
        );
        assert_px(
            left(time) - (left(bot) + width(bot)),
            6.,
            "the gap after the bot",
        );
    }

    #[gpui::test]
    fn a_card_no_bot_works_on_says_unassigned_in_a_framed_chip(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Stage {
            column: unassigned_card,
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("board-card-bot").is_none(),
            "no bot is named as plain text"
        );
        let chip = cx
            .debug_bounds("board-card-unassigned")
            .expect("the chip is drawn");
        // The Badge is SIZE_BADGE_HEIGHT tall; the frame adds a 1 px border above and below.
        assert_px(height(chip), SIZE_BADGE_HEIGHT + 2., "the chip's frame");
    }

    #[gpui::test]
    fn a_source_slot_is_as_tall_as_the_card_it_stands_for(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Stage {
            column: slot_beside_its_card,
        });
        cx.run_until_parked();
        let slot = cx
            .debug_bounds("board-source-slot")
            .expect("the slot is drawn");
        let card = cx.debug_bounds("board-card").expect("the card is drawn");
        assert_px(height(slot), height(card), "the slot's height");
    }
}
