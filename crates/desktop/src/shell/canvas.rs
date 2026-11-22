//! The Canvas: the Conversation, the regions beside it and the overlay region (D-SH-01, A5).

use super::Shell;
use super::settings::{self, SettingsFrame};
use super::setup;
use super::start_page::{self, StartFrame};
use super::teams;
use super::zones::{ZoneFocus, region_focus_ring};
use crate::model::canvas::Canvas;
use crate::model::navigation::{Mode, Page};
use crate::model::region_cycle::Zone;
use crate::model::regions::Region;
use crate::model::shell::{ServiceNotice, ServiceTone};
use crate::model::shell_state::ShellModel;
use crate::model::teams::{BoardOwner, TeamPage};
use gpui::{
    AnyElement, App, Context, Div, FocusHandle, IntoElement, MouseButton, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_ui::components::error_state::{ErrorState, ErrorStateVariant};
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::theme::box_shadows;
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_MIN, LAYOUT_CONVERSATION_READING, RADIUS_CANVAS, RADIUS_LG, SIZE_TOOLBAR,
    SPACE_1, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_HEADING, TYPE_TITLE_SM,
};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

/// The Canvas: the Conversation and the inline regions, with an open overlay on top (D-SH-01).
/// What the full pages and the start page draw from, gathered by the shell for one frame.
pub(super) struct PageFrames<'a> {
    pub settings: SettingsFrame<'a>,
    pub start: Option<StartFrame>,
    pub setup_rows: Vec<crate::model::setup::SetupRow>,
}

pub(super) fn canvas(
    model: &ShellModel,
    zones: &ZoneFocus,
    pages: PageFrames<'_>,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Div {
    let theme = *cx.theme();
    let overlay = model.overlay().filter(|region| *region != Region::Sidebar);
    let shell = cx.entity().downgrade();
    let surface = div()
        .debug_selector(|| "canvas".to_owned())
        .track_focus(zones.get(Zone::Conversation))
        .focus_visible(|style| style.shadow(region_focus_ring(&theme)))
        .relative()
        .flex()
        .flex_row()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .bg(theme.palette.surface.canvas)
        .rounded_tl(px(RADIUS_CANVAS))
        .overflow_hidden()
        .child(conversation(model, &shell, pages, window, cx, &theme))
        .children(model.inline_regions().into_iter().map(|region| {
            region_column(region, zones.get(Zone::of_region(region)).clone(), &theme)
        }));
    surface.children(
        overlay.map(|region| {
            overlay_panel(region, zones.get(Zone::of_region(region)).clone(), &theme)
        }),
    )
}

fn conversation(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    pages: PageFrames<'_>,
    window: &mut Window,
    cx: &mut App,
    theme: &Theme,
) -> AnyElement {
    // The Settings page fills the Canvas, so it is not centred in a box.
    if model.canvas() == Canvas::Page(Page::Settings) {
        return settings::settings_page(pages.settings, window, cx);
    }
    if model.canvas() == Canvas::Page(Page::Setup) {
        return setup::setup_page(&pages.setup_rows, cx);
    }
    // A service that cannot be reached takes the Canvas: its page, not a banner over an empty one.
    if let Some(notice) = model
        .snapshot()
        .service_notice
        .as_ref()
        .filter(|notice| notice.tone == ServiceTone::Unreachable)
    {
        return service_down_page(notice, shell);
    }
    // The start page is laid out from the top, so it is not centred in a box either.
    if let Canvas::Workspace(_) = model.canvas() {
        return match pages.start {
            Some(frame) => start_page::start_page(frame, cx),
            None => placeholder(
                "Workspace",
                "This workspace is not listed by the service.",
                theme,
            )
            .into_any_element(),
        };
    }
    let body: AnyElement = match model.canvas() {
        Canvas::Home(Mode::Workspaces) => start_page(theme).into_any_element(),
        Canvas::Home(Mode::BotTeams) => teams::bot_teams_home(model, shell, cx),
        Canvas::Team(TeamPage::Board(team)) => {
            teams::board_page(model, shell, BoardOwner::Team(team), window, cx)
        }
        Canvas::Team(TeamPage::Project(team, project)) => {
            teams::project_page(model, shell, team, project, window, cx)
        }
        Canvas::Team(TeamPage::Routines(team)) => teams::routines_page(model, shell, team, cx),
        Canvas::Team(TeamPage::Memory(team)) => teams::memory_page(model, shell, team, cx),
        Canvas::Team(TeamPage::Files(team)) => match model.teams().team(team) {
            Some(team) => teams::files_page(team, cx),
            None => placeholder("Files", "This team is gone.", theme).into_any_element(),
        },
        Canvas::Team(TeamPage::Conference(_)) => placeholder(
            "Conference room",
            "The Conference room needs live agents and sessions, which are not built yet.",
            theme,
        )
        .into_any_element(),
        Canvas::Session(title) => {
            placeholder(&title, "The conversation is not built yet.", theme).into_any_element()
        }
        Canvas::Team(page) => {
            placeholder(page.title(), "This page is not built yet.", theme).into_any_element()
        }
        // Drawn above: the start page is not centred in the box below.
        Canvas::Workspace(_) => placeholder(
            "Workspace",
            "This workspace is not listed by the service.",
            theme,
        )
        .into_any_element(),
        Canvas::Page(page) => {
            placeholder(page.title(), "This page is not built yet.", theme).into_any_element()
        }
    };
    div()
        .flex_1()
        .min_w(px(LAYOUT_CONVERSATION_MIN))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .child(body)
        .into_any_element()
}

/// The start page with no session open: the rose logo and the question (A40).
fn start_page(theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(SPACE_3))
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .child(rose_logo())
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("What should we work on?"),
        )
}

/// A centred title and a note for a Canvas content that is not built yet.
fn placeholder(title: &str, note: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(SPACE_1))
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(note.to_owned()),
        )
}

/// The page for a service that cannot be reached: what it is, Retry, and the start error behind
/// Show details (cross-cutting "Service not running").
fn service_down_page(notice: &ServiceNotice, shell: &WeakEntity<Shell>) -> AnyElement {
    let retry = shell.clone();
    let mut page = ErrorState::new("service-down", "MassOrca service is not running")
        .variant(ErrorStateVariant::Page)
        .explanation(
            "Projects, agents and terminals run in the local service on this PC. Retry starts it. Show details has the last start error.",
        )
        .on_retry(move |_, cx| {
            let _ = retry.update(cx, |shell, _| shell.retry_service_now());
        });
    if let Some(detail) = &notice.detail {
        page = page.details(detail.clone());
    }
    div()
        .debug_selector(|| "service-down".to_owned())
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .p(px(SPACE_4))
        .child(page)
        .into_any_element()
}

/// A region docked beside the Conversation, at its default width (D-SH-01).
fn region_column(region: Region, focus: FocusHandle, theme: &Theme) -> impl IntoElement {
    div()
        .track_focus(&focus)
        .focus_visible(|style| style.shadow(region_focus_ring(theme)))
        .flex_none()
        .w(px(region.default_width()))
        .min_w(px(region.min_width()))
        .h_full()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(theme.palette.border.subtle)
        .child(region_header(region, theme))
}

/// A region open over the Canvas when the window is too narrow to fit it inline (X1).
fn overlay_panel(region: Region, focus: FocusHandle, theme: &Theme) -> impl IntoElement {
    div()
        .track_focus(&focus)
        .focus_visible(|style| {
            style.shadow([region_focus_ring(theme), box_shadows(theme.shadows.e2)].concat())
        })
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(region.default_width()))
        .flex()
        .flex_col()
        .bg(theme.palette.surface.overlay)
        .border_1()
        .border_color(theme.palette.border.default)
        .rounded_l(px(RADIUS_LG))
        .shadow(box_shadows(theme.shadows.e2))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(region_header(region, theme))
}

fn region_header(region: Region, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(SIZE_TOOLBAR))
        .px(px(SPACE_3))
        .text_style(TYPE_TITLE_SM)
        .text_color(theme.palette.text.primary)
        .truncate()
        .child(region.name())
}
