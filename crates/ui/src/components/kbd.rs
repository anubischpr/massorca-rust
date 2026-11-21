//! Kbd: a keyboard shortcut badge (docs/design/components/Kbd.md).
//!
//! A binding is stored as `Mod+K` and drawn for the OS: `Ctrl+K` on Windows and
//! Linux, `⌘K` on macOS (handoff 2.3). Kbd is the only way a shortcut is shown.

use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, RADIUS_XS, SIZE_KBD_HEIGHT, SPACE_0, SPACE_1, SPACE_HALF, TYPE_MONO_SM,
};
use gpui::{
    App, Empty, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

/// Transparent: a disabled cap has no fill.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// The platform a shortcut is drawn for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutOs {
    /// Windows and Linux: caps joined with "+".
    WindowsLinux,
    /// macOS: glyphs with no separator.
    MacOs,
}

impl ShortcutOs {
    /// The OS this build runs on.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::WindowsLinux
        }
    }
}

/// A modifier of a stored binding. `Mod` is the primary one: Ctrl on Windows
/// and Linux, Command on macOS. `Ctrl` is always the Control key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Modifier {
    Mod,
    Ctrl,
    Alt,
    Shift,
}

impl Modifier {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "Mod" => Some(Self::Mod),
            "Ctrl" | "Control" => Some(Self::Ctrl),
            "Alt" | "Option" => Some(Self::Alt),
            "Shift" => Some(Self::Shift),
            _ => None,
        }
    }

    /// Order inside a chord: Ctrl, Alt, Shift, then Command on macOS (design-system 8.1).
    fn rank(self, os: ShortcutOs) -> u8 {
        match (self, os) {
            (Self::Mod, ShortcutOs::MacOs) => 3,
            (Self::Mod | Self::Ctrl, _) => 0,
            (Self::Alt, _) => 1,
            (Self::Shift, _) => 2,
        }
    }

    fn glyph(self, os: ShortcutOs) -> &'static str {
        match (self, os) {
            (Self::Mod, ShortcutOs::MacOs) => "⌘",
            (Self::Ctrl, ShortcutOs::MacOs) => "⌃",
            (Self::Alt, ShortcutOs::MacOs) => "⌥",
            (Self::Shift, ShortcutOs::MacOs) => "⇧",
            (Self::Mod | Self::Ctrl, ShortcutOs::WindowsLinux) => "Ctrl",
            (Self::Alt, ShortcutOs::WindowsLinux) => "Alt",
            (Self::Shift, ShortcutOs::WindowsLinux) => "Shift",
        }
    }

    fn spoken(self, os: ShortcutOs) -> &'static str {
        match (self, os) {
            (Self::Mod, ShortcutOs::MacOs) => "Command",
            (Self::Mod | Self::Ctrl, _) => "Control",
            (Self::Alt, ShortcutOs::MacOs) => "Option",
            (Self::Alt, _) => "Alt",
            (Self::Shift, _) => "Shift",
        }
    }
}

/// The key as drawn on `os`: named keys get their glyph, the rest keep their text.
fn key_glyph(key: &str, os: ShortcutOs) -> String {
    let mac = os == ShortcutOs::MacOs;
    let named = match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => Some(if mac { "↩" } else { "Enter" }),
        "esc" | "escape" => Some(if mac { "⎋" } else { "Esc" }),
        "backspace" => Some(if mac { "⌫" } else { "Backspace" }),
        "tab" => Some(if mac { "⇥" } else { "Tab" }),
        _ => None,
    };
    named.map_or_else(|| capitalise(key), str::to_string)
}

/// The key read aloud: "Escape" for the Esc cap, letters as capitals.
fn key_spoken(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "Enter".to_string(),
        "esc" | "escape" => "Escape".to_string(),
        _ => capitalise(key),
    }
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A binding read from its stored form, such as `Mod+Shift+K` or `F6`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Shortcut {
    modifiers: Vec<Modifier>,
    key: Option<String>,
}

impl Shortcut {
    fn parse(binding: &str) -> Self {
        let mut shortcut = Self {
            modifiers: Vec::new(),
            key: None,
        };
        for token in binding
            .split('+')
            .map(str::trim)
            .filter(|token| !token.is_empty())
        {
            match Modifier::parse(token) {
                Some(modifier) => shortcut.modifiers.push(modifier),
                None => shortcut.key = Some(token.to_string()),
            }
        }
        shortcut
    }

    fn ordered_modifiers(&self, os: ShortcutOs) -> Vec<Modifier> {
        let mut modifiers = self.modifiers.clone();
        modifiers.sort_by_key(|modifier| modifier.rank(os));
        modifiers
    }

    /// One entry per key cap, in the order it is drawn.
    fn caps(&self, os: ShortcutOs) -> Vec<String> {
        let mut caps: Vec<String> = self
            .ordered_modifiers(os)
            .into_iter()
            .map(|modifier| modifier.glyph(os).to_string())
            .collect();
        caps.dedup();
        if let Some(key) = &self.key {
            caps.push(key_glyph(key, os));
        }
        caps
    }

    fn spoken(&self, os: ShortcutOs) -> String {
        let mut words: Vec<String> = self
            .ordered_modifiers(os)
            .into_iter()
            .map(|modifier| modifier.spoken(os).to_string())
            .collect();
        words.dedup();
        if let Some(key) = &self.key {
            words.push(key_spoken(key));
        }
        match os {
            ShortcutOs::WindowsLinux => words.join(" plus "),
            ShortcutOs::MacOs => words.join(" "),
        }
    }
}

/// A keyboard shortcut drawn as key caps, for example `Ctrl+K` or `⌘K`.
#[derive(Debug, IntoElement)]
pub struct Kbd {
    binding: SharedString,
    os: ShortcutOs,
    disabled: bool,
    selected: bool,
}

impl Kbd {
    /// Draws `binding` (stored as `Mod+K`) for the running OS.
    pub fn new(binding: impl Into<SharedString>) -> Self {
        Self {
            binding: binding.into(),
            os: ShortcutOs::current(),
            disabled: false,
            selected: false,
        }
    }

    /// Draws the binding for `os` instead of the running OS.
    pub fn os(mut self, os: ShortcutOs) -> Self {
        self.os = os;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Inside a selected row the caps take the primary text colour.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The key caps in drawing order, for example `["Ctrl", "K"]`.
    pub fn caps(&self) -> Vec<String> {
        Shortcut::parse(&self.binding).caps(self.os)
    }

    /// The binding read aloud, for example "Control plus K".
    pub fn spoken(&self) -> String {
        Shortcut::parse(&self.binding).spoken(self.os)
    }
}

impl RenderOnce for Kbd {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let caps = self.caps();
        if caps.is_empty() {
            return Empty.into_any_element();
        }
        let theme = *cx.theme();
        let p = &theme.palette;
        let text = if self.disabled {
            p.text.disabled
        } else if self.selected {
            p.text.primary
        } else {
            p.text.secondary
        };
        let fill = if self.disabled {
            NONE
        } else {
            p.surface.sunken
        };
        let mac = self.os == ShortcutOs::MacOs;
        let mut chord = div()
            .flex()
            .items_center()
            .gap(px(if mac { SPACE_HALF } else { SPACE_0 }))
            .text_style(TYPE_MONO_SM)
            .text_color(text);
        for (index, cap) in caps.into_iter().enumerate() {
            if index > 0 && !mac {
                chord = chord.child("+");
            }
            chord = chord.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(SIZE_KBD_HEIGHT))
                    .min_w(px(SIZE_KBD_HEIGHT))
                    .px(px(SPACE_1))
                    .rounded(px(RADIUS_XS))
                    .border_1()
                    .border_color(p.border.subtle)
                    .bg(fill)
                    .child(cap),
            );
        }
        chord.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_chord_reads_ctrl_plus_the_key() {
        let kbd = Kbd::new("Mod+K").os(ShortcutOs::WindowsLinux);
        assert_eq!(kbd.caps(), vec!["Ctrl", "K"]);
        assert_eq!(kbd.spoken(), "Control plus K");
    }

    #[test]
    fn macos_chord_uses_glyphs_with_no_separator_and_command_last() {
        let kbd = Kbd::new("Mod+Shift+F").os(ShortcutOs::MacOs);
        assert_eq!(kbd.caps(), vec!["⇧", "⌘", "F"]);
        assert_eq!(kbd.spoken(), "Shift Command F");
        assert_eq!(
            Kbd::new("Mod+K").os(ShortcutOs::MacOs).caps(),
            vec!["⌘", "K"]
        );
    }

    #[test]
    fn ctrl_stays_the_control_key_on_macos() {
        assert_eq!(
            Kbd::new("Ctrl+K").os(ShortcutOs::MacOs).caps(),
            vec!["⌃", "K"]
        );
        assert_eq!(
            Kbd::new("Ctrl+K").os(ShortcutOs::WindowsLinux).caps(),
            vec!["Ctrl", "K"]
        );
    }

    #[test]
    fn modifiers_follow_ctrl_alt_shift_order_on_windows() {
        let kbd = Kbd::new("Shift+Alt+Mod+K").os(ShortcutOs::WindowsLinux);
        assert_eq!(kbd.caps(), vec!["Ctrl", "Alt", "Shift", "K"]);
    }

    #[test]
    fn named_keys_take_the_glyph_of_each_platform() {
        assert_eq!(
            Kbd::new("Enter").os(ShortcutOs::WindowsLinux).caps(),
            vec!["Enter"]
        );
        assert_eq!(Kbd::new("Enter").os(ShortcutOs::MacOs).caps(), vec!["↩"]);
        assert_eq!(
            Kbd::new("Esc").os(ShortcutOs::WindowsLinux).caps(),
            vec!["Esc"]
        );
        assert_eq!(Kbd::new("Escape").os(ShortcutOs::MacOs).caps(), vec!["⎋"]);
        assert_eq!(
            Kbd::new("Backspace").os(ShortcutOs::MacOs).caps(),
            vec!["⌫"]
        );
        assert_eq!(Kbd::new("Tab").os(ShortcutOs::MacOs).caps(), vec!["⇥"]);
    }

    #[test]
    fn function_keys_are_the_same_on_every_os() {
        for os in [ShortcutOs::WindowsLinux, ShortcutOs::MacOs] {
            assert_eq!(Kbd::new("F6").os(os).caps(), vec!["F6"]);
            assert_eq!(Kbd::new("F6").os(os).spoken(), "F6");
        }
        assert_eq!(
            Kbd::new("Shift+F6").os(ShortcutOs::MacOs).caps(),
            vec!["⇧", "F6"]
        );
    }

    #[test]
    fn escape_is_read_as_a_word_and_a_bare_modifier_draws_one_cap() {
        assert_eq!(Kbd::new("Esc").spoken(), "Escape");
        assert_eq!(
            Kbd::new("Mod").os(ShortcutOs::WindowsLinux).caps(),
            vec!["Ctrl"]
        );
    }

    #[test]
    fn an_empty_binding_draws_nothing() {
        assert!(Kbd::new("").caps().is_empty());
    }

    #[test]
    fn duplicate_modifiers_collapse_into_one_cap() {
        assert_eq!(
            Kbd::new("Mod+Ctrl+K").os(ShortcutOs::WindowsLinux).caps(),
            vec!["Ctrl", "K"]
        );
    }
}
