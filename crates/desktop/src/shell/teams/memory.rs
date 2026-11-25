//! The team memory (D-AG-26, screens 4.8): what the team remembers, under a Bot and a Team tab. A
//! memory entry is deleted after a confirm, and the page says who can read it.

use super::super::Shell;
use super::team_pill;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{MemoryEntry, MemoryTab, Team, TeamId, TeamPage};
use gpui::{
    AnyElement, App, Div, IntoElement, ParentElement, Styled, WeakEntity, Window, div, prelude::*,
    px,
};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::list_row::ListRow;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::tokens::{SPACE_2, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_HEADING};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// The Memory page of one team (4.8).
pub(in crate::shell) fn memory_page(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    team: TeamId,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let Some(team_data) = model.teams().team(team) else {
        return note("This team is gone.", &theme).into_any_element();
    };
    let tab = model.teams().memory_tab();
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("Memory"),
        )
        .child(team_pill(
            TeamPage::Memory(team),
            &team_data.name,
            team_data.character,
            shell,
            window,
            cx,
        ));
    let entries: Vec<&MemoryEntry> = team_data
        .memory
        .iter()
        .filter(|entry| entry.tab == tab)
        .collect();
    let body = if entries.is_empty() {
        note(
            "What the team remembers shows here. Bots add notes when they learn something that helps later work.",
            &theme,
        )
        .debug_selector(|| "memory-empty".to_owned())
        .into_any_element()
    } else {
        let mut list = div().flex().flex_col().gap(px(SPACE_2));
        for entry in entries {
            list = list.child(entry_row(entry, team, team_data, shell));
        }
        list.into_any_element()
    };
    div()
        .id("memory-page")
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .p(px(SPACE_6))
        .size_full()
        .overflow_y_scroll()
        .child(header)
        .child(tabs(shell, tab))
        .child(body)
        .child(note(
            "Only the team's bots and you can read this memory.",
            &theme,
        ))
        .into_any_element()
}

/// The Bot and Team tabs; the choice stays while the page is open.
fn tabs(shell: &WeakEntity<Shell>, tab: MemoryTab) -> impl IntoElement {
    let shell = shell.clone();
    let selected = match tab {
        MemoryTab::Bot => 0,
        MemoryTab::Team => 1,
    };
    SegmentedControl::new(
        "memory-tabs",
        "Memory",
        vec![Segment::text("Bot"), Segment::text("Team")],
    )
    .selected(selected)
    .on_change(move |index, _, cx| {
        let tab = if index == 0 {
            MemoryTab::Bot
        } else {
            MemoryTab::Team
        };
        let _ = shell.update(cx, |shell, cx| shell.set_memory_tab(tab, cx));
    })
}

/// One entry: its sentence, who wrote it and when, and a More menu whose Delete asks first.
fn entry_row(
    entry: &MemoryEntry,
    team: TeamId,
    team_data: &Team,
    shell: &WeakEntity<Shell>,
) -> impl IntoElement {
    let author = entry
        .author
        .and_then(|id| team_data.bot(id))
        .map_or_else(|| "the team".to_owned(), |bot| bot.name.clone());
    let more = {
        let shell = shell.clone();
        let memory = entry.id;
        let trigger = format!("memory-more-{}", entry.id.0);
        let selector = trigger.clone();
        div().debug_selector(move || selector).child(
            IconButton::new(trigger, IconName::Ellipsis, "More").on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.open_memory_menu(team, memory, window, cx)
                });
            }),
        )
    };
    ListRow::new(format!("memory-{}", entry.id.0), entry.text.clone())
        .secondary(format!("From {author}, {}", entry.date))
        .trailing(more)
}

fn note(text: &str, theme: &Theme) -> Div {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text.to_owned())
}

impl Shell {
    /// Shows the Bot or the Team tab of the memory.
    pub(super) fn set_memory_tab(&mut self, tab: MemoryTab, cx: &mut gpui::Context<Self>) {
        self.model.teams_mut().set_memory_tab(tab);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::super::dialogs::TeamConfirm;
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::teams::{MemoryId, SampleTeamsSource, TeamPage};
    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
    use massorca_ui::{Appearance, Theme as UiTheme};

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| {
            UiTheme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        })
    }

    #[gpui::test]
    fn a_memory_delete_waits_for_its_confirm_and_then_forgets_the_entry(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Memory(TeamId(1)));
            shell.set_memory_tab(MemoryTab::Bot, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let count = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                shell.model().teams().team(TeamId(1)).unwrap().memory.len()
            })
        };
        assert_eq!(count(&shell, cx), 2);
        shell.update(cx, |shell, cx| {
            shell.open_team_confirm(
                TeamConfirm::DeleteMemory {
                    team: TeamId(1),
                    memory: MemoryId(2),
                },
                cx,
            );
        });
        assert_eq!(
            count(&shell, cx),
            2,
            "nothing is deleted before the confirm"
        );
        shell.update(cx, |shell, cx| shell.confirm_team_delete(cx));
        assert_eq!(count(&shell, cx), 1);
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn memory_count(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> usize {
        shell.read_with(cx, |shell, _| {
            shell.model().teams().team(TeamId(1)).unwrap().memory.len()
        })
    }

    #[gpui::test]
    fn a_memory_more_menu_delete_opens_the_same_confirm_the_trash_button_did(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Memory(TeamId(1)));
            shell.set_memory_tab(MemoryTab::Bot, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let more = cx
            .debug_bounds("memory-more-2")
            .expect("the entry draws its More button");
        cx.simulate_click(more.center(), Modifiers::default());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.team_confirm,
                Some(TeamConfirm::DeleteMemory {
                    team: TeamId(1),
                    memory: MemoryId(2),
                }),
                "Delete in the More menu asks the confirm first"
            );
        });
        assert_eq!(
            memory_count(&shell, cx),
            2,
            "nothing is deleted before the confirm"
        );
        shell.update(cx, |shell, cx| shell.confirm_team_delete(cx));
        assert_eq!(memory_count(&shell, cx), 1);
    }

    #[gpui::test]
    fn an_empty_memory_tab_says_one_sentence_with_no_panel_or_title(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Memory(TeamId(1)));
            shell.set_memory_tab(MemoryTab::Team, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.update(cx, |shell, cx| {
            shell.open_team_confirm(
                TeamConfirm::DeleteMemory {
                    team: TeamId(1),
                    memory: MemoryId(1),
                },
                cx,
            );
            shell.confirm_team_delete(cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            cx.debug_bounds("memory-empty").is_some(),
            "the empty Team tab says its sentence"
        );
        assert!(
            cx.debug_bounds("empty-panel").is_none(),
            "the empty state draws no panel and no title"
        );
    }
}
