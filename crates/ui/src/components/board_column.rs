//! One column of a team Board and its cards (D-WS-30, BoardColumn.md). The column is a list named
//! by its title and count; each card is a list item. Moving cards is the caller's job: it passes a
//! key handler to each card and draws the lifted card through `lifted`.

use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled, Theme, box_shadows};
use crate::tokens::{
    LAYOUT_BOARD_COLUMN_WIDTH, RADIUS_LG, SIZE_CONTROL_COMFORTABLE, SPACE_1, SPACE_2, SPACE_HALF,
    TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION, TYPE_TITLE_SM,
};
use std::fmt;

use gpui::{
    App, ElementId, FocusHandle, IntoElement, KeyDownEvent, ParentElement, RenderOnce, Role,
    SharedString, Styled, Window, div, prelude::*, px,
};

/// The text of an empty column (BoardColumn.md, copy).
pub const EMPTY_COLUMN_TEXT: &str = "No tasks";

/// A card's key handler, which the Board uses to move cards.
type CardKeys = Box<dyn Fn(&KeyDownEvent, &mut Window, &mut App)>;

/// One card: its title, the bot working on it, a status with its word, the time and a note.
#[derive(IntoElement)]
pub struct BoardCard {
    id: ElementId,
    title: SharedString,
    focus: FocusHandle,
    bot: Option<SharedString>,
    status: Option<(AgentState, SharedString)>,
    time: Option<SharedString>,
    note: Option<SharedString>,
    unreachable: Option<SharedString>,
    lifted: bool,
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
            status: None,
            time: None,
            note: None,
            unreachable: None,
            lifted: false,
            on_key: None,
        }
    }

    /// The bot's name, after the status.
    pub fn bot(mut self, name: impl Into<SharedString>) -> Self {
        self.bot = Some(name.into());
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
}

impl RenderOnce for BoardCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let hover = p.fill.hover_raised;
        let mut card = div()
            .id(self.id.clone())
            .track_focus(&self.focus)
            .tab_stop(true)
            .focus_ring(&theme)
            .role(Role::ListItem)
            .aria_label(self.title.clone())
            .flex()
            .flex_col()
            .gap(px(SPACE_HALF))
            .p(px(SPACE_2))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
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
        if let Some(listener) = self.on_key {
            card = card.on_key_down(move |event, window, cx| listener(event, window, cx));
        }
        card = card.child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(p.text.primary)
                .line_clamp(2)
                .child(self.title),
        );
        if let Some((state, word)) = self.status {
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .child(AgentStatus::new(state).size(AgentStatusSize::XSmall))
                    .child(
                        div()
                            .text_style(TYPE_CAPTION)
                            .text_color(p.text.secondary)
                            .child(word),
                    ),
            );
        }
        if let Some(bot) = self.bot {
            card = card.child(meta(bot, &theme));
        }
        if let Some(note) = self.note {
            card = card.child(meta(note, &theme));
        }
        if let Some(last_known) = self.unreachable {
            card = card.child(unreachable_line(last_known, &theme));
        }
        if let Some(time) = self.time {
            card = card.child(
                div()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.tertiary)
                    .child(time),
            );
        }
        card
    }
}

fn meta(text: SharedString, theme: &Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(text)
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
}

impl BoardColumn {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            cards: Vec::new(),
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
        let mut cards = div().flex().flex_col().gap(px(SPACE_2)).child(header);
        if self.cards.is_empty() {
            cards = cards.child(
                div()
                    .py(px(SPACE_2))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(EMPTY_COLUMN_TEXT),
            );
        }
        for card in self.cards {
            cards = cards.child(card);
        }
        div()
            .id(self.id.clone())
            .role(Role::List)
            .aria_label(Self::accessible_name(&self.title, count))
            .flex()
            .flex_col()
            .flex_none()
            .w(px(LAYOUT_BOARD_COLUMN_WIDTH))
            .h_full()
            .p(px(SPACE_1))
            .gap(px(SPACE_2))
            .rounded(px(RADIUS_LG))
            .bg(p.surface.sunken)
            .child(cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_accessible_name_counts_tasks_in_the_plural_except_for_one() {
        assert_eq!(
            BoardColumn::accessible_name("Running", 2),
            "Running, 2 tasks"
        );
        assert_eq!(BoardColumn::accessible_name("Open", 1), "Open, 1 task");
        assert_eq!(BoardColumn::accessible_name("Done", 0), "Done, 0 tasks");
    }
}
