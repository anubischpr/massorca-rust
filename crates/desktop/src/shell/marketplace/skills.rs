//! The Skills section of the Skills category: the skills agent tools keep on the host the Marketplace shows (SKL-01).
//! It reads when the page opens, when the picked host changes and on Refresh, and shows pv96's states for each answer.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, Role, SharedString, Styled, WeakEntity, div,
    prelude::*, px,
};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::{EmptyState, EmptyStateVariant};
use massorca_ui::components::error_state::{ErrorState, ErrorStateVariant};
use massorca_ui::components::list_row::ListRow;
use massorca_ui::tokens::{SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_LABEL};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

use super::MarketplaceFrame;
use super::host_pick::effective_target;
use crate::model::marketplace::{
    HostTarget, ItemKind, RootRow, SkillRow, root_rows, search_skills, skill_count_line, skill_rows,
};
use crate::model::navigation::{MarketplacePage, Page};
use crate::service::SkillsListing;
use crate::shell::Shell;

/// What the section says while the service it reads through is not connected.
const NOT_CONNECTED: &str = "The service is not connected.";

/// What the section shows for the host it is aimed at.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SkillsPanel {
    /// The newest read has not been answered yet.
    Scanning,
    /// The host predates skill discovery; the sentence asks for an update.
    UpdateNeeded(String),
    /// The read failed; the sentence says why.
    Refused(String),
    /// The skills the host found, and the roots it read or skipped.
    Found {
        rows: Vec<SkillRow>,
        roots: Vec<RootRow>,
    },
}

/// The host the section lists, as it reads in a sentence, and what the section shows for it.
pub(super) struct SkillsSection {
    pub(super) host: String,
    pub(super) panel: SkillsPanel,
    pub(super) roots_open: bool,
}

impl Shell {
    /// The host the Marketplace shows: the pick while it is offered, otherwise This computer.
    pub(in crate::shell) fn shown_target(&self) -> HostTarget {
        effective_target(self.marketplace_pick.target(), &self.marketplace_offers)
    }

    /// Reads the skills of `target` under a new request; only the newest request is awaited, so an older answer never shows.
    pub(in crate::shell) fn read_skills(&mut self, target: &HostTarget, refresh: bool) {
        self.marketplace_skills_request += 1;
        let request = self.marketplace_skills_request;
        let Some(service) = self.service.clone() else {
            return;
        };
        match target {
            HostTarget::Local => service.read_skills(request, refresh),
            HostTarget::Paired(name) => service.read_host_skills(name.clone(), request, refresh),
        }
    }

    /// Whether the Skills category is the page on screen.
    pub(in crate::shell) fn on_skills_page(&self) -> bool {
        self.model.navigation().page
            == Page::Marketplace(MarketplacePage::Category(ItemKind::Skill))
    }
}

/// What the section shows for an answer; `host` is the paired host's name, or None for This computer.
pub(super) fn panel(
    listing: Option<&SkillsListing>,
    host: Option<&str>,
    connected: bool,
) -> SkillsPanel {
    match listing {
        Some(SkillsListing::Found(found)) => SkillsPanel::Found {
            rows: skill_rows(found),
            roots: root_rows(found),
        },
        Some(SkillsListing::UpdateNeeded) => SkillsPanel::UpdateNeeded(match host {
            Some(name) => format!("Update MassOrca on {name} first."),
            None => "Update MassOrca first.".to_owned(),
        }),
        Some(SkillsListing::Refused(refusal)) => SkillsPanel::Refused(refusal.sentence()),
        None if host.is_none() && !connected => SkillsPanel::Refused(NOT_CONNECTED.to_owned()),
        None => SkillsPanel::Scanning,
    }
}

/// The section for the host the Marketplace shows, from the answer to that host's newest read.
pub(super) fn section_of(shell: &Shell, cx: &App) -> SkillsSection {
    let state = shell.store.read(cx).state();
    let target = shell.shown_target();
    let host = match &target {
        HostTarget::Local => None,
        HostTarget::Paired(name) => Some(name.as_str()),
    };
    let answer = state
        .skills
        .get(&host.map(str::to_owned))
        .filter(|read| read.request == shell.marketplace_skills_request)
        .map(|read| &read.listing);
    SkillsSection {
        roots_open: shell.marketplace_roots_open,
        host: match &target {
            HostTarget::Local => "this computer".to_owned(),
            HostTarget::Paired(name) => name.clone(),
        },
        panel: panel(answer, host, state.connection.is_connected()),
    }
}

/// The Skills section under the toolbar: the host's skills with Refresh, and pv96's states (SKL-01).
pub(super) fn skills_section(frame: &MarketplaceFrame, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let section = &frame.skills;
    let title = format!("Found on {}", section.host);
    let count = match &section.panel {
        SkillsPanel::Found { rows, roots } => format!(
            "{} · {}",
            skill_count_line(rows.len()),
            source_count_line(roots)
        ),
        _ => String::new(),
    };
    let report = match &section.panel {
        SkillsPanel::Found { roots, .. } => {
            Some(roots_report(frame, roots, section.roots_open, cx))
        }
        _ => None,
    };
    let body = match &section.panel {
        SkillsPanel::Scanning => div()
            .text_style(TYPE_BODY_SM)
            .text_color(p.text.tertiary)
            .child("Scanning skills")
            .into_any_element(),
        SkillsPanel::UpdateNeeded(sentence) => Banner::new(
            SharedString::from("marketplace-skills-update"),
            BannerVariant::Info,
            sentence.clone(),
        )
        .into_any_element(),
        SkillsPanel::Refused(sentence) => {
            let shell = frame.shell.clone();
            ErrorState::new("marketplace-skills-error", "Could not scan skills")
                .variant(ErrorStateVariant::Section)
                .explanation(sentence.clone())
                .on_retry(move |_, cx| refresh_skills(&shell, cx))
                .into_any_element()
        }
        SkillsPanel::Found { rows, .. } => found_body(frame, rows, cx),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(SPACE_2))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(SPACE_1))
                        .child(
                            div()
                                .id("marketplace-skills-heading")
                                .role(Role::Heading)
                                .aria_label(SharedString::from(title.clone()))
                                .text_style(TYPE_LABEL)
                                .text_color(p.text.primary)
                                .child(SharedString::from(title)),
                        )
                        .child(
                            div()
                                .text_style(TYPE_BODY_SM)
                                .text_color(p.text.tertiary)
                                .child(SharedString::from(count)),
                        ),
                )
                .child(refresh_button(frame)),
        )
        .child(body)
        .children(report)
        .into_any_element()
}

/// The skills of the host as list rows, or the state that says why there are none to list.
fn found_body(frame: &MarketplaceFrame, rows: &[SkillRow], cx: &App) -> AnyElement {
    if rows.is_empty() {
        return EmptyState::new("marketplace-skills-empty", "No skills found")
            .variant(EmptyStateVariant::Section)
            .explanation("The scanned skill folders are empty. Refresh after adding a skill.")
            .action(refresh_button(frame))
            .into_any_element();
    }
    let matches = search_skills(rows, &frame.query);
    if matches.is_empty() {
        return EmptyState::new("marketplace-skills-empty", "No matches")
            .variant(EmptyStateVariant::Section)
            .explanation("No skill matches the current search and filters.")
            .into_any_element();
    }
    let mut list = div().flex().flex_col();
    for row in matches {
        list = list.child(skill_row(row, cx));
    }
    list.into_any_element()
}

/// One skill as a list row: its name, its description, and the agents and the root it has.
fn skill_row(row: &SkillRow, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let mut item = ListRow::new(
        SharedString::from(format!("marketplace-skill-{}", row.id)),
        row.name.clone(),
    );
    if let Some(description) = &row.description {
        item = item.secondary(description.clone());
    }
    item.trailing(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.tertiary)
            .child(trailing_text(row)),
    )
    .into_any_element()
}

/// The agents that have a skill and the root it was read from, such as "Claude · Claude home".
fn trailing_text(row: &SkillRow) -> String {
    if row.agents.is_empty() {
        row.source.clone()
    } else {
        format!("{} · {}", row.agents.join(", "), row.source)
    }
}

/// "1 source" or "3 sources": the roots that were read in full.
fn source_count_line(roots: &[RootRow]) -> String {
    let read = roots
        .iter()
        .filter(|root| root.exists && root.skip_reason.is_none())
        .count();
    if read == 1 {
        "1 source".to_owned()
    } else {
        format!("{read} sources")
    }
}

/// The Refresh action in the section header, which reads the shown host's skills again.
fn refresh_button(frame: &MarketplaceFrame) -> Button {
    let shell = frame.shell.clone();
    Button::new("marketplace-skills-refresh", "Refresh")
        .variant(ButtonVariant::Secondary)
        .leading_icon(IconName::RefreshCw)
        .on_click(move |_, cx| refresh_skills(&shell, cx))
}

/// Reads the shown host's skills again, from Refresh or Retry.
fn refresh_skills(shell: &WeakEntity<Shell>, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| {
        let target = shell.shown_target();
        shell.read_skills(&target, true);
        cx.notify();
    });
}

/// The "Where MassOrca looked" report: each root the scan read or skipped, and why it was skipped (SKL-01).
fn roots_report(frame: &MarketplaceFrame, roots: &[RootRow], open: bool, cx: &App) -> AnyElement {
    let shell = frame.shell.clone();
    let mut report = div().flex().flex_col().child(
        ListRow::new("marketplace-skills-roots", "Where MassOrca looked")
            .expanded(open)
            .on_toggle(move |expanded, _, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.marketplace_roots_open = expanded;
                    cx.notify();
                });
            }),
    );
    if open {
        for (index, root) in roots.iter().enumerate() {
            report = report.child(root_row(index, root, cx));
        }
    }
    report.into_any_element()
}

/// One root of the report: its label, its agents and kind, and why it was skipped when it was.
fn root_row(index: usize, root: &RootRow, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let mut row = ListRow::new(
        SharedString::from(format!("marketplace-root-{index}")),
        root.label.clone(),
    );
    if let Some(reason) = root.skip_reason {
        row = row.secondary(reason);
    }
    row.trailing(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.tertiary)
            .child(root_trailing(root)),
    )
    .into_any_element()
}

/// The agents of a root and its kind, such as "Claude · Home".
fn root_trailing(root: &RootRow) -> String {
    let mut parts = Vec::new();
    if !root.agents.is_empty() {
        parts.push(root.agents.join(", "));
    }
    if let Some(kind) = root.kind {
        parts.push(kind.to_owned());
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::Refusal;

    #[test]
    fn a_root_names_its_agents_and_kind_and_leaves_out_what_it_lacks() {
        let root = RootRow {
            label: "Claude home".to_owned(),
            agents: vec!["Claude"],
            kind: Some("Home"),
            exists: true,
            skip_reason: None,
        };
        assert_eq!(root_trailing(&root), "Claude · Home");
        let shared = RootRow {
            label: "Shared folder".to_owned(),
            agents: Vec::new(),
            kind: None,
            exists: true,
            skip_reason: None,
        };
        assert_eq!(root_trailing(&shared), "");
    }
    use massorca_protocol::methods::skills::SkillDiscovery;

    fn empty_discovery() -> SkillDiscovery {
        SkillDiscovery {
            ..crate::service::fixtures::base_skill_discovery()
        }
    }

    #[test]
    fn a_host_that_predates_skill_discovery_is_asked_to_update_it() {
        let listing = SkillsListing::UpdateNeeded;
        assert_eq!(
            panel(Some(&listing), Some("vps"), true),
            SkillsPanel::UpdateNeeded("Update MassOrca on vps first.".to_owned())
        );
        assert_eq!(
            panel(Some(&listing), None, true),
            SkillsPanel::UpdateNeeded("Update MassOrca first.".to_owned())
        );
    }

    #[test]
    fn a_refused_read_keeps_the_sentence_that_says_why() {
        let listing = SkillsListing::Refused(Refusal::unavailable("the host went away"));
        assert_eq!(
            panel(Some(&listing), Some("vps"), true),
            SkillsPanel::Refused("The host went away".to_owned())
        );
    }

    #[test]
    fn an_unanswered_read_scans_and_this_computer_says_when_its_service_is_not_connected() {
        assert_eq!(panel(None, Some("vps"), false), SkillsPanel::Scanning);
        assert_eq!(panel(None, None, true), SkillsPanel::Scanning);
        assert_eq!(
            panel(None, None, false),
            SkillsPanel::Refused(NOT_CONNECTED.to_owned())
        );
    }

    #[test]
    fn an_answer_that_lists_skills_shows_them_as_found() {
        let listing = SkillsListing::Found(empty_discovery());
        assert_eq!(
            panel(Some(&listing), None, true),
            SkillsPanel::Found {
                rows: Vec::new(),
                roots: Vec::new(),
            }
        );
    }
}
