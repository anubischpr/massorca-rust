//! The Skills section of the Skills category: the skills agent tools keep on the host the Marketplace shows (SKL-01).
//! It reads when the page opens, when the picked host changes and on Refresh, and shows pv96's states for each answer.

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Role, SharedString, Styled, WeakEntity,
    div, prelude::*, px,
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
    BundledAction, BundledRow, HostTarget, ItemKind, ManagedRow, RootRow, SkillRow,
    bundled_review_sentence, bundled_rows, detected_agents, managed_rows, place_review_sentence,
    plan_detail_lines, remove_review_sentence, result_sentence, root_rows, search_skills,
    skill_count_line, skill_rows,
};
use crate::model::navigation::{MarketplacePage, Page};
use crate::service::{PlacementAnswer, PlacementCall, PlacementOutcome, SkillsListing};
use crate::shell::Shell;
use massorca_protocol::methods::skill_placement::{
    PlacementAgent, PlacementState, SkillPlacementResult,
};

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
    /// Whether the shown host is This computer, the only host skills are placed on.
    pub(super) local: bool,
    /// The placement waiting for the owner's confirmation, if one is open.
    pub(super) review: Option<PlacementReview>,
    /// The newest placement answer: the preview under review, or the note of the last change.
    pub(super) answer: Option<PlacementAnswer>,
    /// The skills MassOrca placed on This computer, by name.
    pub(super) placed: Vec<ManagedRow>,
    /// The MassOrca skills this build ships, with their state on This computer; empty on a paired host.
    pub(super) bundled: Vec<BundledRow>,
}

/// A placement the owner must confirm: the preview of placing a skill (answered under `request`), or the removal of the
/// copies MassOrca placed for a skill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::shell) enum PlacementReview {
    Place {
        skill_id: String,
        name: String,
        request: u64,
        details_open: bool,
    },
    Remove {
        name: String,
    },
    /// The install or update of a MassOrca skill, for the agents the review names (SKL-04).
    Bundled {
        name: String,
        update: bool,
        agents: Vec<PlacementAgent>,
    },
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
            HostTarget::Local => {
                // Why: the placed and bundled lists answer under placement numbers, the ones a change refreshes them with.
                let listed = self.next_placement_request();
                service.read_skills(request, refresh);
                service.skill_placement(listed, PlacementCall::Managed);
                service.skill_placement(listed, PlacementCall::BundledList);
            }
            HostTarget::Paired(name) => service.read_host_skills(name.clone(), request, refresh),
        }
    }

    /// Whether the Skills category is the page on screen.
    pub(in crate::shell) fn on_skills_page(&self) -> bool {
        self.model.navigation().page
            == Page::Marketplace(MarketplacePage::Category(ItemKind::Skill))
    }

    /// The request number of the next placement call; only the newest request's answer is shown.
    pub(in crate::shell) fn next_placement_request(&mut self) -> u64 {
        self.marketplace_placement_request += 1;
        self.marketplace_placement_request
    }

    /// Asks for the removal of the copies MassOrca placed for a skill; the Skills page shows the review to confirm.
    pub(in crate::shell) fn ask_remove_placement(&mut self, name: String, cx: &mut Context<Self>) {
        self.marketplace_placement_review = Some(PlacementReview::Remove { name });
        cx.notify();
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
    let shown = panel(answer, host, state.connection.is_connected());
    // Why: the bundled rows wait for This computer's scan, so Install knows the agent folders it writes to.
    let bundled = match state.bundled.as_ref().map(|held| &held.outcome) {
        Some(PlacementOutcome::Bundled(skills))
            if host.is_none() && matches!(shown, SkillsPanel::Found { .. }) =>
        {
            bundled_rows(skills)
        }
        _ => Vec::new(),
    };
    SkillsSection {
        roots_open: shell.marketplace_roots_open,
        host: match &target {
            HostTarget::Local => "this computer".to_owned(),
            HostTarget::Paired(name) => name.clone(),
        },
        panel: shown,
        local: host.is_none(),
        review: shell.marketplace_placement_review.clone(),
        answer: state.placement.clone(),
        placed: match state.managed.as_ref().map(|held| &held.outcome) {
            Some(PlacementOutcome::Managed(skills)) => managed_rows(skills),
            _ => Vec::new(),
        },
        bundled,
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
        .children(bundled_group(frame, cx))
        .child(body)
        .child(placement_panel(frame, cx))
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
        list = list.child(skill_row(frame, row, cx));
    }
    list.into_any_element()
}

/// One skill as a list row: its name, its description, and the agents and the root it has. On This computer a home
/// skill also offers Place.
fn skill_row(frame: &MarketplaceFrame, row: &SkillRow, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let mut item = ListRow::new(
        SharedString::from(format!("marketplace-skill-{}", row.id)),
        row.name.clone(),
    );
    if let Some(description) = &row.description {
        item = item.secondary(description.clone());
    }
    let mut trailing = div().flex().items_center().gap(px(SPACE_2)).child(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.tertiary)
            .child(trailing_text(row)),
    );
    if frame.skills.local && row.placeable {
        trailing = trailing.child(place_button(frame, row));
    }
    item.trailing(trailing).into_any_element()
}

fn place_button(frame: &MarketplaceFrame, row: &SkillRow) -> Button {
    let shell = frame.shell.clone();
    let (skill_id, name) = (row.id.clone(), row.name.clone());
    Button::new(format!("marketplace-skill-place-{}", row.id), "Place")
        .variant(ButtonVariant::Secondary)
        .on_click(move |_, cx| start_place(&shell, skill_id.clone(), name.clone(), cx))
}

/// The MassOrca skills group above the discovered skills: each skill's name, description, state and one action (SKL-04).
fn bundled_group(frame: &MarketplaceFrame, cx: &App) -> Option<AnyElement> {
    if frame.skills.bundled.is_empty() {
        return None;
    }
    let theme = *cx.theme();
    let mut group = div().flex().flex_col().gap(px(SPACE_1)).child(
        div()
            .id("marketplace-bundled-heading")
            .role(Role::Heading)
            .text_style(TYPE_LABEL)
            .text_color(theme.palette.text.primary)
            .child("MassOrca skills"),
    );
    for row in &frame.skills.bundled {
        group = group.child(bundled_row(frame, row, cx));
    }
    Some(group.into_any_element())
}

/// One bundled skill as a list row: its description, its state on this computer, and the one action it offers.
fn bundled_row(frame: &MarketplaceFrame, row: &BundledRow, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let mut item = ListRow::new(
        SharedString::from(format!("marketplace-bundled-{}", row.name)),
        row.name.clone(),
    );
    if !row.description.is_empty() {
        item = item.secondary(row.description.clone());
    }
    let mut trailing = div().flex().items_center().gap(px(SPACE_2)).child(
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.tertiary)
            .child(row.state_line.clone()),
    );
    if let Some(action) = row.action {
        trailing = trailing.child(bundled_button(frame, row, action));
    }
    item.trailing(trailing).into_any_element()
}

/// The button of a bundled skill's action: Install and Update open a review, and Remove opens the placed-skill review.
fn bundled_button(frame: &MarketplaceFrame, row: &BundledRow, action: BundledAction) -> Button {
    let shell = frame.shell.clone();
    let name = row.name.clone();
    let agents = row.agents.clone();
    let (id, label) = match action {
        BundledAction::Install => (
            format!("marketplace-bundled-install-{}", row.name),
            "Install",
        ),
        BundledAction::Update => (format!("marketplace-bundled-update-{}", row.name), "Update"),
        BundledAction::Remove => (format!("marketplace-bundled-remove-{}", row.name), "Remove"),
    };
    Button::new(id, label)
        .variant(ButtonVariant::Secondary)
        .on_click(move |_, cx| match action {
            BundledAction::Install => {
                start_bundled(&shell, name.clone(), false, agents.clone(), cx)
            }
            BundledAction::Update => start_bundled(&shell, name.clone(), true, agents.clone(), cx),
            BundledAction::Remove => start_remove(&shell, name.clone(), cx),
        })
}

/// The placement area under the list: the review waiting for the owner, the note of the last change, and the skills
/// MassOrca placed on This computer.
fn placement_panel(frame: &MarketplaceFrame, cx: &App) -> AnyElement {
    let section = &frame.skills;
    let mut panel = div().flex().flex_col().gap(px(SPACE_2));
    if let Some(review) = &section.review {
        panel = panel.child(review_body(frame, review, section.answer.as_ref(), cx));
    } else if let Some(note) = answer_note(section.answer.as_ref()) {
        panel = panel.child(Banner::new(
            SharedString::from("marketplace-skills-placement-note"),
            BannerVariant::Info,
            note,
        ));
    }
    if section.local && !section.placed.is_empty() {
        panel = panel.child(placed_list(frame, &section.placed, cx));
    }
    panel.into_any_element()
}

/// The review: the one sentence to confirm, the folders under "Show details", and the buttons that answer it.
fn review_body(
    frame: &MarketplaceFrame,
    review: &PlacementReview,
    answer: Option<&PlacementAnswer>,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let shell = frame.shell.clone();
    let body = div().flex().flex_col().gap(px(SPACE_1));
    match review {
        PlacementReview::Place {
            name,
            request,
            details_open,
            ..
        } => match answer
            .filter(|held| held.request == *request)
            .map(|held| &held.outcome)
        {
            Some(PlacementOutcome::Preview(plan)) => {
                let agents = create_agents(plan);
                let mut actions = div().flex().gap(px(SPACE_2));
                if !agents.is_empty() {
                    let confirm = shell.clone();
                    actions = actions.child(
                        Button::new("marketplace-skill-place-confirm", "Place")
                            .variant(ButtonVariant::Primary)
                            .on_click(move |_, cx| confirm_place(&confirm, cx)),
                    );
                }
                let details = shell.clone();
                let details_label = if *details_open {
                    "Hide details"
                } else {
                    "Show details"
                };
                actions = actions
                    .child(
                        Button::new("marketplace-skill-place-details", details_label)
                            .variant(ButtonVariant::Ghost)
                            .on_click(move |_, cx| toggle_details(&details, cx)),
                    )
                    .child(cancel_button("marketplace-skill-place-cancel", &shell));
                let mut lines =
                    body.child(review_text(place_review_sentence(name, &agents), theme));
                lines = lines.child(actions);
                if *details_open {
                    for line in plan_detail_lines(plan) {
                        lines = lines.child(
                            div()
                                .text_style(TYPE_BODY_SM)
                                .text_color(theme.palette.text.tertiary)
                                .child(SharedString::from(line)),
                        );
                    }
                }
                lines.into_any_element()
            }
            Some(PlacementOutcome::Refused(refusal)) => body
                .child(review_text(refusal.sentence(), theme))
                .child(cancel_button("marketplace-skill-place-cancel", &shell))
                .into_any_element(),
            _ => body
                .child(review_text(
                    format!("Checking what would be placed for {name}."),
                    theme,
                ))
                .child(cancel_button("marketplace-skill-place-cancel", &shell))
                .into_any_element(),
        },
        PlacementReview::Remove { name } => {
            let confirm = shell.clone();
            body.child(review_text(remove_review_sentence(name), theme))
                .child(
                    div()
                        .flex()
                        .gap(px(SPACE_2))
                        .child(
                            Button::new("marketplace-skill-remove-confirm", "Remove")
                                .variant(ButtonVariant::Danger)
                                .on_click(move |_, cx| confirm_remove(&confirm, cx)),
                        )
                        .child(cancel_button("marketplace-skill-remove-cancel", &shell)),
                )
                .into_any_element()
        }
        PlacementReview::Bundled {
            name,
            update,
            agents,
        } => {
            let mut actions = div().flex().gap(px(SPACE_2));
            if !agents.is_empty() {
                let confirm = shell.clone();
                let label = if *update { "Update" } else { "Install" };
                actions = actions.child(
                    Button::new("marketplace-bundled-confirm", label)
                        .variant(ButtonVariant::Primary)
                        .on_click(move |_, cx| confirm_bundled(&confirm, cx)),
                );
            }
            body.child(review_text(
                bundled_review_sentence(name, *update, agents),
                theme,
            ))
            .child(actions.child(cancel_button("marketplace-bundled-cancel", &shell)))
            .into_any_element()
        }
    }
}

fn review_text(sentence: String, theme: massorca_ui::Theme) -> impl IntoElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.primary)
        .child(SharedString::from(sentence))
}

fn cancel_button(id: &'static str, shell: &WeakEntity<Shell>) -> Button {
    let shell = shell.clone();
    Button::new(id, "Cancel")
        .variant(ButtonVariant::Secondary)
        .on_click(move |_, cx| clear_review(&shell, cx))
}

/// The one-sentence note of the last change, or of a refusal that is not a review.
fn answer_note(answer: Option<&PlacementAnswer>) -> Option<String> {
    match &answer?.outcome {
        PlacementOutcome::Changed(result) => {
            let removing = result
                .agents
                .iter()
                .any(|row| row.state == PlacementState::Removed);
            Some(result_sentence(result, removing))
        }
        PlacementOutcome::Refused(refusal) => Some(refusal.sentence()),
        PlacementOutcome::Preview(_)
        | PlacementOutcome::Managed(_)
        | PlacementOutcome::Bundled(_) => None,
    }
}

/// The skills MassOrca placed, each with the agents it is placed for and a Remove action.
fn placed_list(frame: &MarketplaceFrame, placed: &[ManagedRow], cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let mut list = div().flex().flex_col().gap(px(SPACE_1));
    list = list.child(
        div()
            .id("marketplace-skills-placed-heading")
            .role(Role::Heading)
            .text_style(TYPE_LABEL)
            .text_color(theme.palette.text.primary)
            .child("Placed by MassOrca"),
    );
    for row in placed {
        let shell = frame.shell.clone();
        let name = row.name.clone();
        let remove = Button::new(format!("marketplace-placed-remove-{}", row.name), "Remove")
            .variant(ButtonVariant::Secondary)
            .on_click(move |_, cx| start_remove(&shell, name.clone(), cx));
        list = list.child(
            ListRow::new(
                SharedString::from(format!("marketplace-placed-{}", row.name)),
                row.name.clone(),
            )
            .secondary(row.agents.clone())
            .trailing(remove),
        );
    }
    list.into_any_element()
}

/// The agents a preview would create a folder for: the ones a placement goes to.
fn create_agents(plan: &SkillPlacementResult) -> Vec<PlacementAgent> {
    plan.agents
        .iter()
        .filter(|row| row.state == PlacementState::Create)
        .map(|row| row.agent)
        .collect()
}

/// The agents a placement offers on This computer: the ones whose folder exists here.
fn detected_here(state: &crate::service::StoreState) -> Vec<PlacementAgent> {
    state
        .skills
        .get(&None)
        .and_then(|read| match &read.listing {
            SkillsListing::Found(discovery) => Some(detected_agents(&discovery.sources)),
            _ => None,
        })
        .unwrap_or_default()
}

/// Opens the review of placing a skill: asks what would be placed for the agents whose folder exists here.
fn start_place(shell: &WeakEntity<Shell>, skill_id: String, name: String, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| {
        let agents = detected_here(shell.store.read(cx).state());
        let request = shell.next_placement_request();
        shell.marketplace_placement_review = Some(PlacementReview::Place {
            skill_id: skill_id.clone(),
            name,
            request,
            details_open: false,
        });
        if let Some(service) = shell.service.clone() {
            service.skill_placement(request, PlacementCall::Preview { skill_id, agents });
        }
        cx.notify();
    });
}

/// Places the agents the previewed review names, once the owner confirms.
fn confirm_place(shell: &WeakEntity<Shell>, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| {
        let Some(PlacementReview::Place {
            skill_id, request, ..
        }) = shell.marketplace_placement_review.clone()
        else {
            return;
        };
        let agents = match shell.store.read(cx).state().placement.as_ref() {
            Some(held) if held.request == request => match &held.outcome {
                PlacementOutcome::Preview(plan) => create_agents(plan),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        if agents.is_empty() {
            return;
        }
        let next = shell.next_placement_request();
        shell.marketplace_placement_review = None;
        if let Some(service) = shell.service.clone() {
            service.skill_placement(next, PlacementCall::Place { skill_id, agents });
        }
        cx.notify();
    });
}

fn toggle_details(shell: &WeakEntity<Shell>, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| {
        if let Some(PlacementReview::Place { details_open, .. }) =
            shell.marketplace_placement_review.as_mut()
        {
            *details_open = !*details_open;
        }
        cx.notify();
    });
}

fn start_remove(shell: &WeakEntity<Shell>, name: String, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| shell.ask_remove_placement(name, cx));
}

fn confirm_remove(shell: &WeakEntity<Shell>, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| {
        let Some(PlacementReview::Remove { name }) = shell.marketplace_placement_review.take()
        else {
            return;
        };
        let next = shell.next_placement_request();
        if let Some(service) = shell.service.clone() {
            service.skill_placement(next, PlacementCall::Unplace { name });
        }
        cx.notify();
    });
}

/// Opens the review of installing or updating a bundled skill: the folders it goes to, or the agents it is placed for.
fn start_bundled(
    shell: &WeakEntity<Shell>,
    name: String,
    update: bool,
    placed: Vec<PlacementAgent>,
    cx: &mut App,
) {
    let _ = shell.update(cx, |shell, cx| {
        let agents = if update {
            placed
        } else {
            detected_here(shell.store.read(cx).state())
        };
        shell.marketplace_placement_review = Some(PlacementReview::Bundled {
            name,
            update,
            agents,
        });
        cx.notify();
    });
}

/// Sends the install or update the owner confirmed, for the agents its review named; nothing is sent for none.
fn confirm_bundled(shell: &WeakEntity<Shell>, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| {
        let Some(PlacementReview::Bundled {
            name,
            update,
            agents,
        }) = shell.marketplace_placement_review.clone()
        else {
            return;
        };
        if agents.is_empty() {
            return;
        }
        shell.marketplace_placement_review = None;
        let next = shell.next_placement_request();
        let call = if update {
            PlacementCall::BundledRefresh { name }
        } else {
            PlacementCall::BundledInstall { name }
        };
        if let Some(service) = shell.service.clone() {
            service.skill_placement(next, call);
        }
        cx.notify();
    });
}

fn clear_review(shell: &WeakEntity<Shell>, cx: &mut App) {
    let _ = shell.update(cx, |shell, cx| {
        shell.marketplace_placement_review = None;
        cx.notify();
    });
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
