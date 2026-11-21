//! The GPUI view of one terminal: keys, pointer, scrollback and resize, with the
//! grid painted by `grid_paint` (docs/design/components/TerminalSurface.md).

use std::fmt;
use std::rc::Rc;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::Side;
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::term::TermMode;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, ClipboardItem, Context, Div, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, Role, ScrollEvent, StatefulInteractiveElement, Styled,
    Window, canvas, div, px,
};

use super::feed::{Exit, FeedError, FeedEvent, FeedOutcome, TerminalFeed};
use super::geometry::{self, CellHit, CellSize};
use super::grid_paint::{self, Frame};
use super::keys::{self, KeyPress};
use crate::assets::FONT_MONO;
use crate::components::button::Button;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{Palette, RADIUS_MD, SPACE_2};

/// Receives the bytes a key press or a paste sends to the program.
pub type InputHandler = Rc<dyn Fn(&[u8], &mut Window, &mut App)>;
/// Receives the grid the pane shows, in columns and lines, whenever it changes.
pub type ResizeHandler = Rc<dyn Fn(u16, u16, &mut Window, &mut App)>;
/// Restarts the process once it has exited.
pub type RestartHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The pane's accessibility role: an application, as TerminalSurface.md names it.
const ACCESSIBILITY_ROLE: Role = Role::Application;

/// The pane's accessible name: "Terminal: <title>", or "Terminal" until the program sets one.
fn accessible_name(title: Option<&str>) -> String {
    match title {
        Some(title) if !title.is_empty() => format!("Terminal: {title}"),
        _ => "Terminal".to_owned(),
    }
}

/// One terminal drawn as a grid of text cells. Feed it with [`TerminalSurface::apply`];
/// keys, pastes and resizes leave through the callbacks it was given.
pub struct TerminalSurface {
    feed: TerminalFeed,
    focus: FocusHandle,
    /// Where the grid was painted last, for pointer hits.
    place: Option<GridPlace>,
    /// The grid last reported to `on_resize`.
    grid_size: Option<(u16, u16)>,
    selecting: bool,
    /// Scroll distance that is not yet a whole line.
    scroll_remainder: f32,
    on_input: Option<InputHandler>,
    on_resize: Option<ResizeHandler>,
    on_restart: Option<RestartHandler>,
}

/// Where the grid was painted: its top-left corner in the window and the cell size.
#[derive(Clone, Copy, Debug)]
struct GridPlace {
    origin: Point<Pixels>,
    cell: CellSize,
}

impl TerminalSurface {
    /// A surface for a terminal of `cols` x `rows` that has not attached yet.
    pub fn new(cols: u16, rows: u16, cx: &mut App) -> Self {
        Self {
            feed: TerminalFeed::new(cols, rows),
            focus: cx.focus_handle(),
            place: None,
            grid_size: None,
            selecting: false,
            scroll_remainder: 0.0,
            on_input: None,
            on_resize: None,
            on_restart: None,
        }
    }

    /// Receives the bytes of key presses and pastes.
    pub fn on_input(mut self, handler: impl Fn(&[u8], &mut Window, &mut App) + 'static) -> Self {
        self.on_input = Some(Rc::new(handler));
        self
    }

    /// Receives the grid the pane shows, after every change of its size.
    pub fn on_resize(
        mut self,
        handler: impl Fn(u16, u16, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }

    /// Adds a Restart button to the exit row, shown once the process has exited.
    pub fn on_restart(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_restart = Some(Rc::new(handler));
        self
    }

    /// The emulator behind the surface.
    pub fn feed(&self) -> &TerminalFeed {
        &self.feed
    }

    /// The focus handle; focusing it sends keys to the terminal.
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// Applies one item of the attach stream, then repaints.
    pub fn apply(
        &mut self,
        event: FeedEvent<'_>,
        cx: &mut Context<Self>,
    ) -> Result<FeedOutcome, FeedError> {
        let outcome = self.feed.apply(event);
        cx.notify();
        outcome
    }

    /// Records where the grid is painted and the size it fits. When the size changed,
    /// returns the resize handler with the new size, to be called outside this view.
    fn fit(&mut self, place: GridPlace, cols: u16, rows: u16) -> Option<(ResizeHandler, u16, u16)> {
        self.place = Some(place);
        if self.grid_size == Some((cols, rows)) {
            return None;
        }
        self.grid_size = Some((cols, rows));
        let _ = self.feed.apply(FeedEvent::Resized { cols, rows });
        self.on_resize.clone().map(|handler| (handler, cols, rows))
    }

    /// Sends `bytes` to the program. Typing returns the view to the live screen.
    fn send(&mut self, bytes: &[u8], window: &mut Window, cx: &mut Context<Self>) {
        self.feed.scroll(Scroll::Bottom);
        if let Some(handler) = self.on_input.clone() {
            handler(bytes, window, cx);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let mods = keystroke.modifiers;
        if is_copy(key, mods) {
            self.copy(cx);
            cx.stop_propagation();
            return;
        }
        if self.feed.exit().is_some() || app_key(key, mods) {
            cx.propagate();
            return;
        }
        if is_paste(key, mods) {
            self.paste(window, cx);
            cx.stop_propagation();
            return;
        }
        if let Some(scroll) = scroll_key(key, mods) {
            self.feed.scroll(scroll);
            cx.notify();
            cx.stop_propagation();
            return;
        }
        let press = KeyPress {
            key,
            text: keystroke.key_char.as_deref(),
            mods: encoded_mods(mods),
        };
        let app_cursor = self.feed.modes().contains(TermMode::APP_CURSOR);
        match keys::encode_key(press, app_cursor) {
            Some(bytes) => {
                self.send(&bytes, window, cx);
                cx.stop_propagation();
            }
            None => cx.propagate(),
        }
    }

    /// Copies the selection to the clipboard, keeping the selection.
    fn copy(&self, cx: &mut Context<Self>) {
        if let Some(text) = self.feed.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Pastes the clipboard's text, bracketed when the program asked for bracketed paste.
    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let bracketed = self.feed.modes().contains(TermMode::BRACKETED_PASTE);
        let bytes = keys::encode_paste(&text, bracketed);
        self.send(&bytes, window, cx);
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if let Some(hit) = self.cell_at(event.position) {
            let kind = match event.click_count {
                0 | 1 => SelectionType::Simple,
                2 => SelectionType::Semantic,
                _ => SelectionType::Lines,
            };
            self.feed.begin_selection(kind, hit.point, side_of(hit));
            self.selecting = true;
        }
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.selecting = false;
            return;
        }
        if !self.selecting {
            return;
        }
        if let Some(hit) = self.cell_at(event.position) {
            self.feed.extend_selection(hit.point, side_of(hit));
            cx.notify();
        }
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.selecting = false;
        cx.notify();
    }

    fn scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(place) = self.place else {
            return;
        };
        let pixels = event.delta.pixel_delta(px(place.cell.height));
        let lines = f32::from(pixels.y) / place.cell.height + self.scroll_remainder;
        let whole = lines.trunc();
        self.scroll_remainder = lines - whole;
        if whole != 0.0 {
            self.feed.scroll(Scroll::Delta(whole as i32));
            cx.notify();
        }
    }

    /// The cell under a window position, once the grid has been painted.
    fn cell_at(&self, position: Point<Pixels>) -> Option<CellHit> {
        let place = self.place?;
        let x = f32::from(position.x - place.origin.x);
        let y = f32::from(position.y - place.origin.y);
        Some(geometry::cell_at(
            x,
            y,
            place.cell,
            self.feed.columns(),
            self.feed.lines(),
        ))
    }
}

impl fmt::Debug for TerminalSurface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalSurface")
            .field("feed", &self.feed)
            .field("grid_size", &self.grid_size)
            .finish_non_exhaustive()
    }
}

impl Render for TerminalSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.theme().palette;
        let focused = self.focus.is_focused(window);
        let label = accessible_name(self.feed.title().as_deref());
        let exit = self.feed.exit();
        let surface = cx.entity();
        let grid = canvas(
            move |bounds, window, cx| prepare(&surface, focused, bounds, window, cx),
            grid_paint::paint_grid,
        )
        .size_full();

        div()
            .id("terminal-surface")
            .relative()
            .size_full()
            .p(px(SPACE_2))
            .bg(palette.terminal.bg)
            .font_family(FONT_MONO)
            .track_focus(&self.focus)
            .role(ACCESSIBILITY_ROLE)
            .aria_label(label)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_scroll(cx.listener(Self::scroll))
            .child(grid)
            .when(focused, |pane| pane.child(focus_ring(&palette)))
            .when_some(exit, |pane, exit| {
                pane.child(exit_row(exit, &palette, self.on_restart.clone()))
            })
    }
}

/// Measures the grid, reports its size and takes the snapshot the paint pass draws.
fn prepare(
    surface: &Entity<TerminalSurface>,
    focused: bool,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Frame> {
    let cell = grid_paint::cell_metrics(cx)?;
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let (cols, rows) = geometry::grid_fit(width, height, cell);
    let place = GridPlace {
        origin: bounds.origin,
        cell,
    };
    let resized = surface.update(cx, |view, _| view.fit(place, cols, rows));
    if let Some((handler, cols, rows)) = resized {
        handler(cols, rows, window, cx);
    }
    let palette = cx.theme().palette;
    let view = surface.read(cx);
    let snapshot = view.feed.snapshot(&palette.terminal);
    let cursor = match view.feed.exit() {
        Some(_) => None,
        None => snapshot.cursor,
    };
    Some(Frame {
        snapshot,
        cell,
        cursor,
        focused,
        palette: palette.terminal,
    })
}

/// The focus ring: an inset 2 px ring at the pane edge, drawn while the terminal has focus.
fn focus_ring(palette: &Palette) -> Div {
    div()
        .absolute()
        .inset_0()
        .border_2()
        .border_color(palette.border.focus)
}

/// The exit row at the bottom of the pane. A non-zero code is drawn in the danger colour.
fn exit_row(exit: Exit, palette: &Palette, restart: Option<RestartHandler>) -> Div {
    let (text, colour) = match exit.code {
        Some(0) => ("Process finished".to_owned(), palette.text.primary),
        Some(code) => (
            format!("Process exited with code {code}"),
            palette.status.danger_fg,
        ),
        None => ("Process exited".to_owned(), palette.text.primary),
    };
    div()
        .absolute()
        .left(px(SPACE_2))
        .right(px(SPACE_2))
        .bottom(px(SPACE_2))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(SPACE_2))
        .p(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(palette.border.default)
        .bg(palette.surface.raised)
        .text_body()
        .text_color(colour)
        .child(text)
        .when_some(restart, |row, restart| {
            row.child(
                Button::new("terminal-restart", "Restart")
                    .on_click(move |window, cx| restart(window, cx)),
            )
        })
}

/// The side of a cell a pointer on its right half selects from.
fn side_of(hit: CellHit) -> Side {
    if hit.right_half {
        Side::Right
    } else {
        Side::Left
    }
}

/// The modifiers the terminal encodes: Shift, Alt and Control, never the platform key.
fn encoded_mods(modifiers: Modifiers) -> keys::Mods {
    keys::Mods {
        shift: modifiers.shift,
        alt: modifiers.alt,
        control: modifiers.control,
    }
}

/// Copy: Cmd+C on macOS, Ctrl+Shift+C elsewhere.
fn is_copy(key: &str, mods: Modifiers) -> bool {
    key.eq_ignore_ascii_case("c") && copy_paste_chord(mods)
}

/// Paste: Cmd+V on macOS, Ctrl+Shift+V elsewhere.
fn is_paste(key: &str, mods: Modifiers) -> bool {
    key.eq_ignore_ascii_case("v") && copy_paste_chord(mods)
}

fn copy_paste_chord(mods: Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        mods.platform && !mods.control && !mods.alt && !mods.shift
    } else {
        mods.control && mods.shift && !mods.alt && !mods.platform
    }
}

/// Keys the app keeps for itself: the platform key (paste excepted), F6, Ctrl+Tab and the
/// command palette's Ctrl+Shift+P.
fn app_key(key: &str, mods: Modifiers) -> bool {
    (mods.platform && !is_paste(key, mods))
        || key == "f6"
        || (key == "tab" && mods.control)
        || (mods.control && mods.shift && key.eq_ignore_ascii_case("p"))
}

/// Shift with Page Up, Page Down, Home or End moves through the scrollback.
fn scroll_key(key: &str, mods: Modifiers) -> Option<Scroll> {
    if !mods.shift || mods.control || mods.alt || mods.platform {
        return None;
    }
    match key {
        "pageup" => Some(Scroll::PageUp),
        "pagedown" => Some(Scroll::PageDown),
        "home" => Some(Scroll::Top),
        "end" => Some(Scroll::Bottom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pane_is_an_application_named_after_its_title() {
        assert_eq!(ACCESSIBILITY_ROLE, Role::Application);
        assert_eq!(accessible_name(Some("zsh, oppm")), "Terminal: zsh, oppm");
        assert_eq!(accessible_name(Some("")), "Terminal");
        assert_eq!(accessible_name(None), "Terminal");
    }
}
