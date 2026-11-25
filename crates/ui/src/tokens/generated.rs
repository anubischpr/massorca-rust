// @generated from docs/design/tokens.json by tests/tokens_are_generated.rs.
// Do not edit by hand (decision 0022).

#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use super::{Color, Easing, FontFamily, Measure, ShadowToken, TextStyleToken};
use std::time::Duration;

/// Colour tokens named `surface.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceColors {
    /// `surface.frame`
    pub frame: Color,
    /// `surface.canvas`
    pub canvas: Color,
    /// `surface.sunken`
    pub sunken: Color,
    /// `surface.raised`
    pub raised: Color,
    /// `surface.overlay`
    pub overlay: Color,
}

/// Colour tokens named `progress.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressColors {
    /// `progress.track`
    pub track: Color,
}

/// Colour tokens named `text.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextColors {
    /// `text.primary`
    pub primary: Color,
    /// `text.secondary`
    pub secondary: Color,
    /// `text.tertiary`
    pub tertiary: Color,
    /// `text.disabled`
    pub disabled: Color,
    /// `text.on-accent`
    pub on_accent: Color,
}

/// Colour tokens named `border.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderColors {
    /// `border.subtle`
    pub subtle: Color,
    /// `border.default`
    pub r#default: Color,
    /// `border.strong`
    pub strong: Color,
    /// `border.focus`
    pub focus: Color,
    /// `border.disabled`
    pub disabled: Color,
}

/// Colour tokens named `accent.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccentColors {
    /// `accent.default`
    pub r#default: Color,
    /// `accent.hover`
    pub hover: Color,
    /// `accent.pressed`
    pub pressed: Color,
    /// `accent.fg`
    pub fg: Color,
    /// `accent.on`
    pub on: Color,
    /// `accent.subtle.frame`
    pub subtle_frame: Color,
    /// `accent.subtle.canvas`
    pub subtle_canvas: Color,
    /// `accent.subtle.raised`
    pub subtle_raised: Color,
    /// `accent.subtle.overlay`
    pub subtle_overlay: Color,
}

/// Colour tokens named `status.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusColors {
    /// `status.success.fg`
    pub success_fg: Color,
    /// `status.success.solid`
    pub success_solid: Color,
    /// `status.success.on`
    pub success_on: Color,
    /// `status.success.bg`
    pub success_bg: Color,
    /// `status.success.border`
    pub success_border: Color,
    /// `status.warning.fg`
    pub warning_fg: Color,
    /// `status.warning.solid`
    pub warning_solid: Color,
    /// `status.warning.on`
    pub warning_on: Color,
    /// `status.warning.bg`
    pub warning_bg: Color,
    /// `status.warning.border`
    pub warning_border: Color,
    /// `status.danger.fg`
    pub danger_fg: Color,
    /// `status.danger.solid`
    pub danger_solid: Color,
    /// `status.danger.on`
    pub danger_on: Color,
    /// `status.danger.bg`
    pub danger_bg: Color,
    /// `status.danger.border`
    pub danger_border: Color,
    /// `status.danger.hover`
    pub danger_hover: Color,
    /// `status.danger.pressed`
    pub danger_pressed: Color,
    /// `status.info.fg`
    pub info_fg: Color,
    /// `status.info.solid`
    pub info_solid: Color,
    /// `status.info.on`
    pub info_on: Color,
    /// `status.info.bg`
    pub info_bg: Color,
    /// `status.info.border`
    pub info_border: Color,
}

/// Colour tokens named `agent.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgentColors {
    /// `agent.running`
    pub running: Color,
    /// `agent.waiting`
    pub waiting: Color,
    /// `agent.idle`
    pub idle: Color,
    /// `agent.error`
    pub error: Color,
    /// `agent.unreachable`
    pub unreachable: Color,
    /// `agent.done`
    pub done: Color,
    /// `agent.waiting.bg`
    pub waiting_bg: Color,
    /// `agent.waiting.bg.overlay`
    pub waiting_bg_overlay: Color,
    /// `agent.error.bg`
    pub error_bg: Color,
    /// `agent.error.bg.overlay`
    pub error_bg_overlay: Color,
    /// `agent.unreachable.bg`
    pub unreachable_bg: Color,
    /// `agent.unreachable.bg.overlay`
    pub unreachable_bg_overlay: Color,
    /// `agent.unreachable.border`
    pub unreachable_border: Color,
}

/// Colour tokens named `diff.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiffColors {
    /// `diff.add.bg`
    pub add_bg: Color,
    /// `diff.add.gutter`
    pub add_gutter: Color,
    /// `diff.add.word`
    pub add_word: Color,
    /// `diff.add.fg`
    pub add_fg: Color,
    /// `diff.remove.bg`
    pub remove_bg: Color,
    /// `diff.remove.gutter`
    pub remove_gutter: Color,
    /// `diff.remove.word`
    pub remove_word: Color,
    /// `diff.remove.fg`
    pub remove_fg: Color,
}

/// Colour tokens named `terminal.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalColors {
    /// `terminal.bg`
    pub bg: Color,
    /// `terminal.fg`
    pub fg: Color,
    /// `terminal.cursor`
    pub cursor: Color,
    /// `terminal.selection`
    pub selection: Color,
    /// `terminal.ansi.black`
    pub ansi_black: Color,
    /// `terminal.ansi.red`
    pub ansi_red: Color,
    /// `terminal.ansi.green`
    pub ansi_green: Color,
    /// `terminal.ansi.yellow`
    pub ansi_yellow: Color,
    /// `terminal.ansi.blue`
    pub ansi_blue: Color,
    /// `terminal.ansi.magenta`
    pub ansi_magenta: Color,
    /// `terminal.ansi.cyan`
    pub ansi_cyan: Color,
    /// `terminal.ansi.white`
    pub ansi_white: Color,
    /// `terminal.ansi.bright-black`
    pub ansi_bright_black: Color,
    /// `terminal.ansi.bright-red`
    pub ansi_bright_red: Color,
    /// `terminal.ansi.bright-green`
    pub ansi_bright_green: Color,
    /// `terminal.ansi.bright-yellow`
    pub ansi_bright_yellow: Color,
    /// `terminal.ansi.bright-blue`
    pub ansi_bright_blue: Color,
    /// `terminal.ansi.bright-magenta`
    pub ansi_bright_magenta: Color,
    /// `terminal.ansi.bright-cyan`
    pub ansi_bright_cyan: Color,
    /// `terminal.ansi.bright-white`
    pub ansi_bright_white: Color,
}

/// Colour tokens named `syntax.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyntaxColors {
    /// `syntax.comment`
    pub comment: Color,
    /// `syntax.keyword`
    pub keyword: Color,
    /// `syntax.string`
    pub string: Color,
    /// `syntax.number`
    pub number: Color,
    /// `syntax.type`
    pub r#type: Color,
    /// `syntax.function`
    pub function: Color,
    /// `syntax.variable`
    pub variable: Color,
    /// `syntax.property`
    pub property: Color,
    /// `syntax.constant`
    pub constant: Color,
    /// `syntax.operator`
    pub operator: Color,
    /// `syntax.punctuation`
    pub punctuation: Color,
    /// `syntax.tag`
    pub tag: Color,
    /// `syntax.attribute`
    pub attribute: Color,
    /// `syntax.invalid`
    pub invalid: Color,
}

/// Colour tokens named `usage.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsageColors {
    /// `usage.ring.1`
    pub ring_1: Color,
    /// `usage.ring.2`
    pub ring_2: Color,
    /// `usage.ring.3`
    pub ring_3: Color,
    /// `usage.ring.4`
    pub ring_4: Color,
    /// `usage.ring.5`
    pub ring_5: Color,
    /// `usage.ring.6`
    pub ring_6: Color,
    /// `usage.ring.track`
    pub ring_track: Color,
    /// `usage.ring.warn`
    pub ring_warn: Color,
    /// `usage.ring.limit`
    pub ring_limit: Color,
}

/// Colour tokens named `selection.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionColors {
    /// `selection.bg`
    pub bg: Color,
}

/// Colour tokens named `match.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchColors {
    /// `match.bg`
    pub bg: Color,
    /// `match.active.bg`
    pub active_bg: Color,
}

/// Colour tokens named `fill.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillColors {
    /// `fill.skeleton`
    pub skeleton: Color,
    /// `fill.disabled`
    pub disabled: Color,
    /// `fill.ripple`
    pub ripple: Color,
    /// `fill.hover.frame`
    pub hover_frame: Color,
    /// `fill.selected.frame`
    pub selected_frame: Color,
    /// `fill.pressed.frame`
    pub pressed_frame: Color,
    /// `fill.hover.canvas`
    pub hover_canvas: Color,
    /// `fill.selected.canvas`
    pub selected_canvas: Color,
    /// `fill.pressed.canvas`
    pub pressed_canvas: Color,
    /// `fill.hover.raised`
    pub hover_raised: Color,
    /// `fill.selected.raised`
    pub selected_raised: Color,
    /// `fill.pressed.raised`
    pub pressed_raised: Color,
    /// `fill.hover.overlay`
    pub hover_overlay: Color,
    /// `fill.selected.overlay`
    pub selected_overlay: Color,
    /// `fill.pressed.overlay`
    pub pressed_overlay: Color,
}

/// Colour tokens named `badge.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadgeColors {
    /// `badge.neutral.bg`
    pub neutral_bg: Color,
    /// `badge.neutral.fg`
    pub neutral_fg: Color,
}

/// Colour tokens named `switch.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchColors {
    /// `switch.knob.off`
    pub knob_off: Color,
}

/// Every colour token of one theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub surface: SurfaceColors,
    pub progress: ProgressColors,
    pub text: TextColors,
    pub border: BorderColors,
    pub accent: AccentColors,
    pub status: StatusColors,
    pub agent: AgentColors,
    pub diff: DiffColors,
    pub terminal: TerminalColors,
    pub syntax: SyntaxColors,
    pub usage: UsageColors,
    pub selection: SelectionColors,
    pub r#match: MatchColors,
    pub fill: FillColors,
    pub badge: BadgeColors,
    pub switch: SwitchColors,
    /// `scrim`
    pub scrim: Color,
}

impl Palette {
    pub const DARK: Palette = Palette {
        surface: SurfaceColors {
            frame: Color::rgba8(27, 27, 28, 1.0),
            canvas: Color::rgba8(21, 21, 23, 1.0),
            sunken: Color::rgba8(16, 16, 18, 1.0),
            raised: Color::rgba8(30, 30, 33, 1.0),
            overlay: Color::rgba8(37, 37, 40, 1.0),
        },
        progress: ProgressColors {
            track: Color::rgba8(16, 16, 18, 1.0),
        },
        text: TextColors {
            primary: Color::rgba8(236, 236, 239, 1.0),
            secondary: Color::rgba8(180, 180, 189, 1.0),
            tertiary: Color::rgba8(157, 157, 167, 1.0),
            disabled: Color::rgba8(95, 95, 103, 1.0),
            on_accent: Color::rgba8(255, 255, 255, 1.0),
        },
        border: BorderColors {
            subtle: Color::rgba8(42, 42, 46, 1.0),
            r#default: Color::rgba8(54, 54, 59, 1.0),
            strong: Color::rgba8(120, 120, 128, 1.0),
            focus: Color::rgba8(184, 168, 248, 1.0),
            disabled: Color::rgba8(47, 47, 52, 1.0),
        },
        accent: AccentColors {
            r#default: Color::rgba8(116, 97, 216, 1.0),
            hover: Color::rgba8(108, 89, 210, 1.0),
            pressed: Color::rgba8(101, 82, 196, 1.0),
            fg: Color::rgba8(184, 168, 248, 1.0),
            on: Color::rgba8(255, 255, 255, 1.0),
            subtle_frame: Color::rgba8(38, 34, 58, 1.0),
            subtle_canvas: Color::rgba8(33, 29, 51, 1.0),
            subtle_raised: Color::rgba8(43, 38, 66, 1.0),
            subtle_overlay: Color::rgba8(48, 42, 74, 1.0),
        },
        status: StatusColors {
            success_fg: Color::rgba8(95, 209, 138, 1.0),
            success_solid: Color::rgba8(26, 128, 64, 1.0),
            success_on: Color::rgba8(255, 255, 255, 1.0),
            success_bg: Color::rgba8(17, 39, 26, 1.0),
            success_border: Color::rgba8(31, 90, 54, 1.0),
            warning_fg: Color::rgba8(242, 189, 74, 1.0),
            warning_solid: Color::rgba8(233, 161, 27, 1.0),
            warning_on: Color::rgba8(26, 17, 0, 1.0),
            warning_bg: Color::rgba8(42, 34, 16, 1.0),
            warning_border: Color::rgba8(107, 83, 22, 1.0),
            danger_fg: Color::rgba8(255, 123, 123, 1.0),
            danger_solid: Color::rgba8(201, 58, 58, 1.0),
            danger_on: Color::rgba8(255, 255, 255, 1.0),
            danger_bg: Color::rgba8(45, 21, 23, 1.0),
            danger_border: Color::rgba8(109, 42, 46, 1.0),
            danger_hover: Color::rgba8(207, 65, 65, 1.0),
            danger_pressed: Color::rgba8(180, 48, 48, 1.0),
            info_fg: Color::rgba8(99, 184, 240, 1.0),
            info_solid: Color::rgba8(28, 117, 176, 1.0),
            info_on: Color::rgba8(255, 255, 255, 1.0),
            info_bg: Color::rgba8(16, 34, 47, 1.0),
            info_border: Color::rgba8(31, 77, 107, 1.0),
        },
        agent: AgentColors {
            running: Color::rgba8(79, 195, 214, 1.0),
            waiting: Color::rgba8(255, 157, 77, 1.0),
            idle: Color::rgba8(157, 157, 167, 1.0),
            error: Color::rgba8(255, 123, 123, 1.0),
            unreachable: Color::rgba8(179, 168, 214, 1.0),
            done: Color::rgba8(95, 209, 138, 1.0),
            waiting_bg: Color::rgba8(46, 33, 19, 1.0),
            waiting_bg_overlay: Color::rgba8(62, 49, 36, 1.0),
            error_bg: Color::rgba8(45, 21, 23, 1.0),
            error_bg_overlay: Color::rgba8(61, 37, 40, 1.0),
            unreachable_bg: Color::rgba8(34, 31, 48, 1.0),
            unreachable_bg_overlay: Color::rgba8(50, 47, 65, 1.0),
            unreachable_border: Color::rgba8(54, 54, 59, 1.0),
        },
        diff: DiffColors {
            add_bg: Color::rgba8(18, 48, 31, 1.0),
            add_gutter: Color::rgba8(26, 74, 45, 1.0),
            add_word: Color::rgba8(31, 96, 57, 1.0),
            add_fg: Color::rgba8(139, 224, 164, 1.0),
            remove_bg: Color::rgba8(58, 23, 25, 1.0),
            remove_gutter: Color::rgba8(90, 34, 41, 1.0),
            remove_word: Color::rgba8(114, 43, 52, 1.0),
            remove_fg: Color::rgba8(255, 158, 164, 1.0),
        },
        terminal: TerminalColors {
            bg: Color::rgba8(16, 16, 18, 1.0),
            fg: Color::rgba8(220, 220, 224, 1.0),
            cursor: Color::rgba8(232, 232, 236, 1.0),
            selection: Color::rgba8(47, 79, 133, 1.0),
            ansi_black: Color::rgba8(61, 61, 70, 1.0),
            ansi_red: Color::rgba8(240, 113, 122, 1.0),
            ansi_green: Color::rgba8(127, 212, 145, 1.0),
            ansi_yellow: Color::rgba8(230, 195, 106, 1.0),
            ansi_blue: Color::rgba8(110, 166, 245, 1.0),
            ansi_magenta: Color::rgba8(199, 146, 232, 1.0),
            ansi_cyan: Color::rgba8(92, 200, 208, 1.0),
            ansi_white: Color::rgba8(201, 201, 209, 1.0),
            ansi_bright_black: Color::rgba8(134, 134, 146, 1.0),
            ansi_bright_red: Color::rgba8(255, 143, 150, 1.0),
            ansi_bright_green: Color::rgba8(155, 232, 168, 1.0),
            ansi_bright_yellow: Color::rgba8(245, 217, 138, 1.0),
            ansi_bright_blue: Color::rgba8(139, 188, 255, 1.0),
            ansi_bright_magenta: Color::rgba8(219, 170, 245, 1.0),
            ansi_bright_cyan: Color::rgba8(127, 224, 230, 1.0),
            ansi_bright_white: Color::rgba8(242, 242, 245, 1.0),
        },
        syntax: SyntaxColors {
            comment: Color::rgba8(139, 147, 161, 1.0),
            keyword: Color::rgba8(199, 146, 234, 1.0),
            string: Color::rgba8(227, 165, 111, 1.0),
            number: Color::rgba8(184, 213, 154, 1.0),
            r#type: Color::rgba8(86, 201, 182, 1.0),
            function: Color::rgba8(227, 213, 143, 1.0),
            variable: Color::rgba8(159, 216, 255, 1.0),
            property: Color::rgba8(140, 194, 255, 1.0),
            constant: Color::rgba8(127, 180, 255, 1.0),
            operator: Color::rgba8(207, 207, 216, 1.0),
            punctuation: Color::rgba8(154, 154, 166, 1.0),
            tag: Color::rgba8(242, 119, 127, 1.0),
            attribute: Color::rgba8(232, 185, 108, 1.0),
            invalid: Color::rgba8(255, 107, 107, 1.0),
        },
        usage: UsageColors {
            ring_1: Color::rgba8(90, 162, 255, 1.0),
            ring_2: Color::rgba8(255, 159, 90, 1.0),
            ring_3: Color::rgba8(79, 209, 165, 1.0),
            ring_4: Color::rgba8(229, 138, 192, 1.0),
            ring_5: Color::rgba8(230, 212, 90, 1.0),
            ring_6: Color::rgba8(169, 139, 255, 1.0),
            ring_track: Color::rgba8(58, 58, 64, 1.0),
            ring_warn: Color::rgba8(242, 189, 74, 1.0),
            ring_limit: Color::rgba8(255, 123, 123, 1.0),
        },
        selection: SelectionColors {
            bg: Color::rgba8(44, 74, 122, 1.0),
        },
        r#match: MatchColors {
            bg: Color::rgba8(90, 74, 20, 1.0),
            active_bg: Color::rgba8(110, 84, 18, 1.0),
        },
        fill: FillColors {
            skeleton: Color::rgba8(38, 38, 42, 1.0),
            disabled: Color::rgba8(26, 26, 29, 1.0),
            ripple: Color::rgba8(255, 255, 255, 0.122),
            hover_frame: Color::rgba8(36, 36, 37, 1.0),
            selected_frame: Color::rgba8(43, 43, 44, 1.0),
            pressed_frame: Color::rgba8(50, 50, 51, 1.0),
            hover_canvas: Color::rgba8(30, 30, 32, 1.0),
            selected_canvas: Color::rgba8(37, 37, 39, 1.0),
            pressed_canvas: Color::rgba8(44, 44, 46, 1.0),
            hover_raised: Color::rgba8(39, 39, 42, 1.0),
            selected_raised: Color::rgba8(46, 46, 49, 1.0),
            pressed_raised: Color::rgba8(53, 53, 55, 1.0),
            hover_overlay: Color::rgba8(46, 46, 49, 1.0),
            selected_overlay: Color::rgba8(52, 52, 55, 1.0),
            pressed_overlay: Color::rgba8(59, 59, 62, 1.0),
        },
        badge: BadgeColors {
            neutral_bg: Color::rgba8(52, 52, 58, 1.0),
            neutral_fg: Color::rgba8(236, 236, 239, 1.0),
        },
        switch: SwitchColors {
            knob_off: Color::rgba8(169, 169, 179, 1.0),
        },
        scrim: Color::rgba8(0, 0, 0, 0.651),
    };
    pub const LIGHT: Palette = Palette {
        surface: SurfaceColors {
            frame: Color::rgba8(238, 238, 242, 1.0),
            canvas: Color::rgba8(251, 251, 252, 1.0),
            sunken: Color::rgba8(244, 244, 247, 1.0),
            raised: Color::rgba8(255, 255, 255, 1.0),
            overlay: Color::rgba8(255, 255, 255, 1.0),
        },
        progress: ProgressColors {
            track: Color::rgba8(244, 244, 247, 1.0),
        },
        text: TextColors {
            primary: Color::rgba8(23, 23, 28, 1.0),
            secondary: Color::rgba8(74, 74, 84, 1.0),
            tertiary: Color::rgba8(95, 95, 107, 1.0),
            disabled: Color::rgba8(162, 162, 172, 1.0),
            on_accent: Color::rgba8(255, 255, 255, 1.0),
        },
        border: BorderColors {
            subtle: Color::rgba8(228, 228, 233, 1.0),
            r#default: Color::rgba8(211, 211, 219, 1.0),
            strong: Color::rgba8(124, 124, 136, 1.0),
            focus: Color::rgba8(91, 63, 196, 1.0),
            disabled: Color::rgba8(220, 220, 227, 1.0),
        },
        accent: AccentColors {
            r#default: Color::rgba8(100, 71, 201, 1.0),
            hover: Color::rgba8(90, 63, 189, 1.0),
            pressed: Color::rgba8(79, 54, 168, 1.0),
            fg: Color::rgba8(91, 63, 196, 1.0),
            on: Color::rgba8(255, 255, 255, 1.0),
            subtle_frame: Color::rgba8(226, 220, 245, 1.0),
            subtle_canvas: Color::rgba8(236, 232, 251, 1.0),
            subtle_raised: Color::rgba8(240, 236, 253, 1.0),
            subtle_overlay: Color::rgba8(240, 236, 253, 1.0),
        },
        status: StatusColors {
            success_fg: Color::rgba8(20, 108, 55, 1.0),
            success_solid: Color::rgba8(21, 128, 61, 1.0),
            success_on: Color::rgba8(255, 255, 255, 1.0),
            success_bg: Color::rgba8(230, 245, 234, 1.0),
            success_border: Color::rgba8(159, 213, 179, 1.0),
            warning_fg: Color::rgba8(138, 87, 0, 1.0),
            warning_solid: Color::rgba8(166, 95, 0, 1.0),
            warning_on: Color::rgba8(255, 255, 255, 1.0),
            warning_bg: Color::rgba8(253, 243, 217, 1.0),
            warning_border: Color::rgba8(229, 195, 102, 1.0),
            danger_fg: Color::rgba8(180, 35, 35, 1.0),
            danger_solid: Color::rgba8(198, 40, 40, 1.0),
            danger_on: Color::rgba8(255, 255, 255, 1.0),
            danger_bg: Color::rgba8(253, 234, 234, 1.0),
            danger_border: Color::rgba8(235, 164, 164, 1.0),
            danger_hover: Color::rgba8(181, 34, 34, 1.0),
            danger_pressed: Color::rgba8(160, 28, 28, 1.0),
            info_fg: Color::rgba8(11, 106, 158, 1.0),
            info_solid: Color::rgba8(11, 106, 158, 1.0),
            info_on: Color::rgba8(255, 255, 255, 1.0),
            info_bg: Color::rgba8(229, 242, 250, 1.0),
            info_border: Color::rgba8(156, 203, 230, 1.0),
        },
        agent: AgentColors {
            running: Color::rgba8(10, 114, 133, 1.0),
            waiting: Color::rgba8(168, 79, 0, 1.0),
            idle: Color::rgba8(95, 95, 107, 1.0),
            error: Color::rgba8(180, 35, 35, 1.0),
            unreachable: Color::rgba8(106, 93, 154, 1.0),
            done: Color::rgba8(20, 108, 55, 1.0),
            waiting_bg: Color::rgba8(253, 238, 221, 1.0),
            waiting_bg_overlay: Color::rgba8(255, 242, 224, 1.0),
            error_bg: Color::rgba8(253, 234, 234, 1.0),
            error_bg_overlay: Color::rgba8(255, 238, 237, 1.0),
            unreachable_bg: Color::rgba8(239, 236, 248, 1.0),
            unreachable_bg_overlay: Color::rgba8(243, 240, 251, 1.0),
            unreachable_border: Color::rgba8(211, 211, 219, 1.0),
        },
        diff: DiffColors {
            add_bg: Color::rgba8(227, 245, 232, 1.0),
            add_gutter: Color::rgba8(197, 235, 208, 1.0),
            add_word: Color::rgba8(169, 224, 186, 1.0),
            add_fg: Color::rgba8(17, 107, 52, 1.0),
            remove_bg: Color::rgba8(253, 234, 234, 1.0),
            remove_gutter: Color::rgba8(248, 205, 209, 1.0),
            remove_word: Color::rgba8(242, 173, 180, 1.0),
            remove_fg: Color::rgba8(164, 38, 44, 1.0),
        },
        terminal: TerminalColors {
            bg: Color::rgba8(248, 248, 250, 1.0),
            fg: Color::rgba8(31, 31, 38, 1.0),
            cursor: Color::rgba8(31, 31, 38, 1.0),
            selection: Color::rgba8(188, 211, 255, 1.0),
            ansi_black: Color::rgba8(31, 31, 38, 1.0),
            ansi_red: Color::rgba8(179, 38, 30, 1.0),
            ansi_green: Color::rgba8(27, 122, 62, 1.0),
            ansi_yellow: Color::rgba8(138, 97, 0, 1.0),
            ansi_blue: Color::rgba8(31, 85, 199, 1.0),
            ansi_magenta: Color::rgba8(142, 63, 181, 1.0),
            ansi_cyan: Color::rgba8(10, 117, 128, 1.0),
            ansi_white: Color::rgba8(118, 118, 131, 1.0),
            ansi_bright_black: Color::rgba8(82, 82, 92, 1.0),
            ansi_bright_red: Color::rgba8(196, 43, 43, 1.0),
            ansi_bright_green: Color::rgba8(22, 117, 58, 1.0),
            ansi_bright_yellow: Color::rgba8(122, 86, 0, 1.0),
            ansi_bright_blue: Color::rgba8(26, 75, 184, 1.0),
            ansi_bright_magenta: Color::rgba8(122, 47, 166, 1.0),
            ansi_bright_cyan: Color::rgba8(6, 104, 114, 1.0),
            ansi_bright_white: Color::rgba8(74, 74, 84, 1.0),
        },
        syntax: SyntaxColors {
            comment: Color::rgba8(102, 112, 130, 1.0),
            keyword: Color::rgba8(122, 63, 196, 1.0),
            string: Color::rgba8(161, 74, 20, 1.0),
            number: Color::rgba8(45, 122, 47, 1.0),
            r#type: Color::rgba8(10, 122, 107, 1.0),
            function: Color::rgba8(122, 90, 0, 1.0),
            variable: Color::rgba8(11, 106, 165, 1.0),
            property: Color::rgba8(26, 84, 181, 1.0),
            constant: Color::rgba8(26, 77, 176, 1.0),
            operator: Color::rgba8(60, 60, 70, 1.0),
            punctuation: Color::rgba8(92, 92, 102, 1.0),
            tag: Color::rgba8(181, 36, 58, 1.0),
            attribute: Color::rgba8(138, 82, 0, 1.0),
            invalid: Color::rgba8(180, 35, 35, 1.0),
        },
        usage: UsageColors {
            ring_1: Color::rgba8(31, 95, 208, 1.0),
            ring_2: Color::rgba8(184, 90, 0, 1.0),
            ring_3: Color::rgba8(11, 125, 92, 1.0),
            ring_4: Color::rgba8(181, 61, 136, 1.0),
            ring_5: Color::rgba8(138, 116, 0, 1.0),
            ring_6: Color::rgba8(106, 70, 216, 1.0),
            ring_track: Color::rgba8(207, 207, 216, 1.0),
            ring_warn: Color::rgba8(138, 87, 0, 1.0),
            ring_limit: Color::rgba8(180, 35, 35, 1.0),
        },
        selection: SelectionColors {
            bg: Color::rgba8(191, 212, 251, 1.0),
        },
        r#match: MatchColors {
            bg: Color::rgba8(251, 229, 138, 1.0),
            active_bg: Color::rgba8(246, 196, 67, 1.0),
        },
        fill: FillColors {
            skeleton: Color::rgba8(230, 230, 235, 1.0),
            disabled: Color::rgba8(240, 240, 244, 1.0),
            ripple: Color::rgba8(20, 20, 26, 0.122),
            hover_frame: Color::rgba8(227, 227, 231, 1.0),
            selected_frame: Color::rgba8(218, 218, 223, 1.0),
            pressed_frame: Color::rgba8(210, 210, 214, 1.0),
            hover_canvas: Color::rgba8(239, 239, 241, 1.0),
            selected_canvas: Color::rgba8(230, 230, 232, 1.0),
            pressed_canvas: Color::rgba8(221, 221, 223, 1.0),
            hover_raised: Color::rgba8(243, 243, 244, 1.0),
            selected_raised: Color::rgba8(234, 234, 234, 1.0),
            pressed_raised: Color::rgba8(224, 224, 225, 1.0),
            hover_overlay: Color::rgba8(243, 243, 244, 1.0),
            selected_overlay: Color::rgba8(234, 234, 234, 1.0),
            pressed_overlay: Color::rgba8(224, 224, 225, 1.0),
        },
        badge: BadgeColors {
            neutral_bg: Color::rgba8(213, 213, 221, 1.0),
            neutral_fg: Color::rgba8(23, 23, 28, 1.0),
        },
        switch: SwitchColors {
            knob_off: Color::rgba8(91, 91, 103, 1.0),
        },
        scrim: Color::rgba8(20, 20, 30, 0.322),
    };

    /// Looks a colour up by its token name, as theme files write it.
    pub fn get(&self, name: &str) -> Option<Color> {
        Some(match name {
            "surface.frame" => self.surface.frame,
            "surface.canvas" => self.surface.canvas,
            "surface.sunken" => self.surface.sunken,
            "surface.raised" => self.surface.raised,
            "surface.overlay" => self.surface.overlay,
            "progress.track" => self.progress.track,
            "text.primary" => self.text.primary,
            "text.secondary" => self.text.secondary,
            "text.tertiary" => self.text.tertiary,
            "text.disabled" => self.text.disabled,
            "text.on-accent" => self.text.on_accent,
            "border.subtle" => self.border.subtle,
            "border.default" => self.border.r#default,
            "border.strong" => self.border.strong,
            "border.focus" => self.border.focus,
            "border.disabled" => self.border.disabled,
            "accent.default" => self.accent.r#default,
            "accent.hover" => self.accent.hover,
            "accent.pressed" => self.accent.pressed,
            "accent.fg" => self.accent.fg,
            "accent.on" => self.accent.on,
            "accent.subtle.frame" => self.accent.subtle_frame,
            "accent.subtle.canvas" => self.accent.subtle_canvas,
            "accent.subtle.raised" => self.accent.subtle_raised,
            "accent.subtle.overlay" => self.accent.subtle_overlay,
            "status.success.fg" => self.status.success_fg,
            "status.success.solid" => self.status.success_solid,
            "status.success.on" => self.status.success_on,
            "status.success.bg" => self.status.success_bg,
            "status.success.border" => self.status.success_border,
            "status.warning.fg" => self.status.warning_fg,
            "status.warning.solid" => self.status.warning_solid,
            "status.warning.on" => self.status.warning_on,
            "status.warning.bg" => self.status.warning_bg,
            "status.warning.border" => self.status.warning_border,
            "status.danger.fg" => self.status.danger_fg,
            "status.danger.solid" => self.status.danger_solid,
            "status.danger.on" => self.status.danger_on,
            "status.danger.bg" => self.status.danger_bg,
            "status.danger.border" => self.status.danger_border,
            "status.danger.hover" => self.status.danger_hover,
            "status.danger.pressed" => self.status.danger_pressed,
            "status.info.fg" => self.status.info_fg,
            "status.info.solid" => self.status.info_solid,
            "status.info.on" => self.status.info_on,
            "status.info.bg" => self.status.info_bg,
            "status.info.border" => self.status.info_border,
            "agent.running" => self.agent.running,
            "agent.waiting" => self.agent.waiting,
            "agent.idle" => self.agent.idle,
            "agent.error" => self.agent.error,
            "agent.unreachable" => self.agent.unreachable,
            "agent.done" => self.agent.done,
            "agent.waiting.bg" => self.agent.waiting_bg,
            "agent.waiting.bg.overlay" => self.agent.waiting_bg_overlay,
            "agent.error.bg" => self.agent.error_bg,
            "agent.error.bg.overlay" => self.agent.error_bg_overlay,
            "agent.unreachable.bg" => self.agent.unreachable_bg,
            "agent.unreachable.bg.overlay" => self.agent.unreachable_bg_overlay,
            "agent.unreachable.border" => self.agent.unreachable_border,
            "diff.add.bg" => self.diff.add_bg,
            "diff.add.gutter" => self.diff.add_gutter,
            "diff.add.word" => self.diff.add_word,
            "diff.add.fg" => self.diff.add_fg,
            "diff.remove.bg" => self.diff.remove_bg,
            "diff.remove.gutter" => self.diff.remove_gutter,
            "diff.remove.word" => self.diff.remove_word,
            "diff.remove.fg" => self.diff.remove_fg,
            "terminal.bg" => self.terminal.bg,
            "terminal.fg" => self.terminal.fg,
            "terminal.cursor" => self.terminal.cursor,
            "terminal.selection" => self.terminal.selection,
            "terminal.ansi.black" => self.terminal.ansi_black,
            "terminal.ansi.red" => self.terminal.ansi_red,
            "terminal.ansi.green" => self.terminal.ansi_green,
            "terminal.ansi.yellow" => self.terminal.ansi_yellow,
            "terminal.ansi.blue" => self.terminal.ansi_blue,
            "terminal.ansi.magenta" => self.terminal.ansi_magenta,
            "terminal.ansi.cyan" => self.terminal.ansi_cyan,
            "terminal.ansi.white" => self.terminal.ansi_white,
            "terminal.ansi.bright-black" => self.terminal.ansi_bright_black,
            "terminal.ansi.bright-red" => self.terminal.ansi_bright_red,
            "terminal.ansi.bright-green" => self.terminal.ansi_bright_green,
            "terminal.ansi.bright-yellow" => self.terminal.ansi_bright_yellow,
            "terminal.ansi.bright-blue" => self.terminal.ansi_bright_blue,
            "terminal.ansi.bright-magenta" => self.terminal.ansi_bright_magenta,
            "terminal.ansi.bright-cyan" => self.terminal.ansi_bright_cyan,
            "terminal.ansi.bright-white" => self.terminal.ansi_bright_white,
            "syntax.comment" => self.syntax.comment,
            "syntax.keyword" => self.syntax.keyword,
            "syntax.string" => self.syntax.string,
            "syntax.number" => self.syntax.number,
            "syntax.type" => self.syntax.r#type,
            "syntax.function" => self.syntax.function,
            "syntax.variable" => self.syntax.variable,
            "syntax.property" => self.syntax.property,
            "syntax.constant" => self.syntax.constant,
            "syntax.operator" => self.syntax.operator,
            "syntax.punctuation" => self.syntax.punctuation,
            "syntax.tag" => self.syntax.tag,
            "syntax.attribute" => self.syntax.attribute,
            "syntax.invalid" => self.syntax.invalid,
            "usage.ring.1" => self.usage.ring_1,
            "usage.ring.2" => self.usage.ring_2,
            "usage.ring.3" => self.usage.ring_3,
            "usage.ring.4" => self.usage.ring_4,
            "usage.ring.5" => self.usage.ring_5,
            "usage.ring.6" => self.usage.ring_6,
            "usage.ring.track" => self.usage.ring_track,
            "usage.ring.warn" => self.usage.ring_warn,
            "usage.ring.limit" => self.usage.ring_limit,
            "selection.bg" => self.selection.bg,
            "match.bg" => self.r#match.bg,
            "match.active.bg" => self.r#match.active_bg,
            "fill.skeleton" => self.fill.skeleton,
            "fill.disabled" => self.fill.disabled,
            "fill.ripple" => self.fill.ripple,
            "fill.hover.frame" => self.fill.hover_frame,
            "fill.selected.frame" => self.fill.selected_frame,
            "fill.pressed.frame" => self.fill.pressed_frame,
            "fill.hover.canvas" => self.fill.hover_canvas,
            "fill.selected.canvas" => self.fill.selected_canvas,
            "fill.pressed.canvas" => self.fill.pressed_canvas,
            "fill.hover.raised" => self.fill.hover_raised,
            "fill.selected.raised" => self.fill.selected_raised,
            "fill.pressed.raised" => self.fill.pressed_raised,
            "fill.hover.overlay" => self.fill.hover_overlay,
            "fill.selected.overlay" => self.fill.selected_overlay,
            "fill.pressed.overlay" => self.fill.pressed_overlay,
            "badge.neutral.bg" => self.badge.neutral_bg,
            "badge.neutral.fg" => self.badge.neutral_fg,
            "switch.knob.off" => self.switch.knob_off,
            "scrim" => self.scrim,
            _ => return None,
        })
    }
}

/// Every colour token name, in token-file order.
pub const PALETTE_NAMES: &[&str] = &[
    "surface.frame",
    "surface.canvas",
    "surface.sunken",
    "progress.track",
    "surface.raised",
    "surface.overlay",
    "text.primary",
    "text.secondary",
    "text.tertiary",
    "text.disabled",
    "text.on-accent",
    "border.subtle",
    "border.default",
    "border.strong",
    "border.focus",
    "border.disabled",
    "accent.default",
    "accent.hover",
    "accent.pressed",
    "accent.fg",
    "accent.on",
    "status.success.fg",
    "status.success.solid",
    "status.success.on",
    "status.success.bg",
    "status.success.border",
    "status.warning.fg",
    "status.warning.solid",
    "status.warning.on",
    "status.warning.bg",
    "status.warning.border",
    "status.danger.fg",
    "status.danger.solid",
    "status.danger.on",
    "status.danger.bg",
    "status.danger.border",
    "status.danger.hover",
    "status.danger.pressed",
    "status.info.fg",
    "status.info.solid",
    "status.info.on",
    "status.info.bg",
    "status.info.border",
    "agent.running",
    "agent.waiting",
    "agent.idle",
    "agent.error",
    "agent.unreachable",
    "agent.done",
    "agent.waiting.bg",
    "agent.waiting.bg.overlay",
    "agent.error.bg",
    "agent.error.bg.overlay",
    "agent.unreachable.bg",
    "agent.unreachable.bg.overlay",
    "agent.unreachable.border",
    "diff.add.bg",
    "diff.add.gutter",
    "diff.add.word",
    "diff.add.fg",
    "diff.remove.bg",
    "diff.remove.gutter",
    "diff.remove.word",
    "diff.remove.fg",
    "terminal.bg",
    "terminal.fg",
    "terminal.cursor",
    "terminal.selection",
    "terminal.ansi.black",
    "terminal.ansi.red",
    "terminal.ansi.green",
    "terminal.ansi.yellow",
    "terminal.ansi.blue",
    "terminal.ansi.magenta",
    "terminal.ansi.cyan",
    "terminal.ansi.white",
    "terminal.ansi.bright-black",
    "terminal.ansi.bright-red",
    "terminal.ansi.bright-green",
    "terminal.ansi.bright-yellow",
    "terminal.ansi.bright-blue",
    "terminal.ansi.bright-magenta",
    "terminal.ansi.bright-cyan",
    "terminal.ansi.bright-white",
    "syntax.comment",
    "syntax.keyword",
    "syntax.string",
    "syntax.number",
    "syntax.type",
    "syntax.function",
    "syntax.variable",
    "syntax.property",
    "syntax.constant",
    "syntax.operator",
    "syntax.punctuation",
    "syntax.tag",
    "syntax.attribute",
    "syntax.invalid",
    "usage.ring.1",
    "usage.ring.2",
    "usage.ring.3",
    "usage.ring.4",
    "usage.ring.5",
    "usage.ring.6",
    "usage.ring.track",
    "usage.ring.warn",
    "usage.ring.limit",
    "selection.bg",
    "match.bg",
    "match.active.bg",
    "fill.skeleton",
    "fill.disabled",
    "fill.ripple",
    "badge.neutral.bg",
    "badge.neutral.fg",
    "switch.knob.off",
    "scrim",
    "fill.hover.frame",
    "fill.selected.frame",
    "fill.pressed.frame",
    "accent.subtle.frame",
    "fill.hover.canvas",
    "fill.selected.canvas",
    "fill.pressed.canvas",
    "accent.subtle.canvas",
    "fill.hover.raised",
    "fill.selected.raised",
    "fill.pressed.raised",
    "accent.subtle.raised",
    "fill.hover.overlay",
    "fill.selected.overlay",
    "fill.pressed.overlay",
    "accent.subtle.overlay",
];

/// Shadow tokens of one theme (decision 0022; spike S3 maps blur one to one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadows {
    /// `shadow.e0`
    pub e0: &'static [ShadowToken],
    /// `shadow.e1`
    pub e1: &'static [ShadowToken],
    /// `shadow.e2`
    pub e2: &'static [ShadowToken],
    /// `shadow.e3`
    pub e3: &'static [ShadowToken],
    /// `shadow.focus`
    pub focus: &'static [ShadowToken],
}

impl Shadows {
    pub const DARK: Shadows = Shadows {
        e0: &[],
        e1: &[ShadowToken { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Color::rgba8(0, 0, 0, 0.4) }, ],
        e2: &[ShadowToken { x: 0.0, y: 6.0, blur: 16.0, spread: -2.0, color: Color::rgba8(0, 0, 0, 0.55) }, ],
        e3: &[ShadowToken { x: 0.0, y: 16.0, blur: 40.0, spread: -4.0, color: Color::rgba8(0, 0, 0, 0.65) }, ],
        focus: &[ShadowToken { x: 0.0, y: 0.0, blur: 0.0, spread: 2.0, color: Color::rgba8(21, 21, 23, 1.0) }, ShadowToken { x: 0.0, y: 0.0, blur: 0.0, spread: 4.0, color: Color::rgba8(184, 168, 248, 1.0) }, ],
    };
    pub const LIGHT: Shadows = Shadows {
        e0: &[],
        e1: &[ShadowToken { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Color::rgba8(20, 20, 40, 0.08) }, ],
        e2: &[ShadowToken { x: 0.0, y: 6.0, blur: 16.0, spread: -2.0, color: Color::rgba8(20, 20, 40, 0.16) }, ],
        e3: &[ShadowToken { x: 0.0, y: 16.0, blur: 40.0, spread: -4.0, color: Color::rgba8(20, 20, 40, 0.22) }, ],
        focus: &[ShadowToken { x: 0.0, y: 0.0, blur: 0.0, spread: 2.0, color: Color::rgba8(251, 251, 252, 1.0) }, ShadowToken { x: 0.0, y: 0.0, blur: 0.0, spread: 4.0, color: Color::rgba8(91, 63, 196, 1.0) }, ],
    };
}

// Text (desktop, px)
/// Type style `caption`.
pub const TYPE_CAPTION: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 11.0, line_height: 16.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `overline`.
pub const TYPE_OVERLINE: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 11.0, line_height: 16.0, weight: 600, letter_spacing_em: 0.04 };
/// Type style `label`.
pub const TYPE_LABEL: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 12.0, line_height: 16.0, weight: 500, letter_spacing_em: 0.0 };
/// Type style `body-sm`.
pub const TYPE_BODY_SM: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 12.0, line_height: 18.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `body`.
pub const TYPE_BODY: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 13.0, line_height: 20.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `body-strong`.
pub const TYPE_BODY_STRONG: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 13.0, line_height: 20.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `title-sm`.
pub const TYPE_TITLE_SM: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 14.0, line_height: 20.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `title`.
pub const TYPE_TITLE: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 16.0, line_height: 24.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `heading`.
pub const TYPE_HEADING: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 20.0, line_height: 28.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `display`.
pub const TYPE_DISPLAY: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 28.0, line_height: 36.0, weight: 600, letter_spacing_em: -0.01 };
/// Type style `chat`.
pub const TYPE_CHAT: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 14.0, line_height: 22.0, weight: 400, letter_spacing_em: 0.0 };
// Mono (desktop, px)
/// Type style `mono-sm`.
pub const TYPE_MONO_SM: TextStyleToken = TextStyleToken { family: FontFamily::Mono, size: 12.0, line_height: 16.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `mono`.
pub const TYPE_MONO: TextStyleToken = TextStyleToken { family: FontFamily::Mono, size: 13.0, line_height: 20.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `terminal`.
pub const TYPE_TERMINAL: TextStyleToken = TextStyleToken { family: FontFamily::Mono, size: 13.0, line_height: 16.0, weight: 400, letter_spacing_em: 0.0 };
// Android (values in sp)
/// Type style `a-caption`.
pub const TYPE_A_CAPTION: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 12.0, line_height: 16.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `a-label`.
pub const TYPE_A_LABEL: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 14.0, line_height: 20.0, weight: 500, letter_spacing_em: 0.0 };
/// Type style `a-body-sm`.
pub const TYPE_A_BODY_SM: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 13.0, line_height: 18.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `a-body`.
pub const TYPE_A_BODY: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 14.0, line_height: 20.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `a-title-sm`.
pub const TYPE_A_TITLE_SM: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 16.0, line_height: 22.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `a-title`.
pub const TYPE_A_TITLE: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 18.0, line_height: 24.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `a-heading`.
pub const TYPE_A_HEADING: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 22.0, line_height: 28.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `a-display`.
pub const TYPE_A_DISPLAY: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 28.0, line_height: 36.0, weight: 600, letter_spacing_em: 0.0 };
/// Type style `a-chat`.
pub const TYPE_A_CHAT: TextStyleToken = TextStyleToken { family: FontFamily::Sans, size: 16.0, line_height: 24.0, weight: 400, letter_spacing_em: 0.0 };
/// Type style `a-mono`.
pub const TYPE_A_MONO: TextStyleToken = TextStyleToken { family: FontFamily::Mono, size: 13.0, line_height: 18.0, weight: 400, letter_spacing_em: 0.0 };

// spacing
/// `space.0`
pub const SPACE_0: f32 = 0.0;
/// `space.half`
pub const SPACE_HALF: f32 = 2.0;
/// `space.1`
pub const SPACE_1: f32 = 4.0;
/// `space.2`
pub const SPACE_2: f32 = 8.0;
/// `space.3`
pub const SPACE_3: f32 = 12.0;
/// `space.4`
pub const SPACE_4: f32 = 16.0;
/// `space.5`
pub const SPACE_5: f32 = 20.0;
/// `space.6`
pub const SPACE_6: f32 = 24.0;
/// `space.8`
pub const SPACE_8: f32 = 32.0;
/// `space.10`
pub const SPACE_10: f32 = 40.0;
/// `space.12`
pub const SPACE_12: f32 = 48.0;
/// `space.16`
pub const SPACE_16: f32 = 64.0;

// radius
/// `radius.none`
pub const RADIUS_NONE: f32 = 0.0;
/// `radius.xs`
pub const RADIUS_XS: f32 = 2.0;
/// `radius.sm`
pub const RADIUS_SM: f32 = 4.0;
/// `radius.md`
pub const RADIUS_MD: f32 = 6.0;
/// `radius.lg`
pub const RADIUS_LG: f32 = 8.0;
/// `radius.xl`
pub const RADIUS_XL: f32 = 12.0;
/// `radius.2xl`
pub const RADIUS_2XL: f32 = 16.0;
/// `radius.canvas`
pub const RADIUS_CANVAS: f32 = 12.0;
/// `radius.full`
pub const RADIUS_FULL: f32 = 9999.0;

// size
/// `size.control.compact`
pub const SIZE_CONTROL_COMPACT: f32 = 24.0;
/// `size.control.default`
pub const SIZE_CONTROL_DEFAULT: f32 = 28.0;
/// `size.control.comfortable`
pub const SIZE_CONTROL_COMFORTABLE: f32 = 32.0;
/// `size.touch.default`
pub const SIZE_TOUCH_DEFAULT: f32 = 44.0;
/// `size.touch.large`
pub const SIZE_TOUCH_LARGE: f32 = 48.0;
/// `size.hit.pointer`
pub const SIZE_HIT_POINTER: f32 = 24.0;
/// `size.hit.touch`
pub const SIZE_HIT_TOUCH: f32 = 48.0;
/// `size.icon.xs`
pub const SIZE_ICON_XS: f32 = 12.0;
/// `size.icon.sm`
pub const SIZE_ICON_SM: f32 = 16.0;
/// `size.icon.md`
pub const SIZE_ICON_MD: f32 = 20.0;
/// `size.icon.lg`
pub const SIZE_ICON_LG: f32 = 24.0;
/// `size.rail.width`
pub const SIZE_RAIL_WIDTH: f32 = 48.0;
/// `size.rail.button`
pub const SIZE_RAIL_BUTTON: f32 = 36.0;
/// `size.rail.ring`
pub const SIZE_RAIL_RING: f32 = 32.0;
/// `size.titlebar.windows`
pub const SIZE_TITLEBAR_WINDOWS: f32 = 36.0;
/// `size.titlebar.macos`
pub const SIZE_TITLEBAR_MACOS: f32 = 40.0;
/// `size.statusbar`
pub const SIZE_STATUSBAR: f32 = 24.0;
/// `size.avatar.sm`
pub const SIZE_AVATAR_SM: f32 = 20.0;
/// `size.avatar.md`
pub const SIZE_AVATAR_MD: f32 = 28.0;
/// `size.avatar.lg`
pub const SIZE_AVATAR_LG: f32 = 40.0;
/// `size.indicator.dot`
pub const SIZE_INDICATOR_DOT: f32 = 6.0;
/// `size.badge.height`
pub const SIZE_BADGE_HEIGHT: f32 = 16.0;
/// `size.tag.height`
pub const SIZE_TAG_HEIGHT: f32 = 20.0;
/// `size.kbd.height`
pub const SIZE_KBD_HEIGHT: f32 = 16.0;
/// `size.progress.height`
pub const SIZE_PROGRESS_HEIGHT: f32 = 4.0;
/// `size.skeleton.line`
pub const SIZE_SKELETON_LINE: f32 = 12.0;
/// `size.toolbar`
pub const SIZE_TOOLBAR: f32 = 36.0;
/// `size.switch.track.width`
pub const SIZE_SWITCH_TRACK_WIDTH: f32 = 32.0;
/// `size.switch.track.height`
pub const SIZE_SWITCH_TRACK_HEIGHT: f32 = 18.0;
/// `size.switch.knob`
pub const SIZE_SWITCH_KNOB: f32 = 12.0;
/// `size.switch.touch.track.width`
pub const SIZE_SWITCH_TOUCH_TRACK_WIDTH: f32 = 36.0;
/// `size.switch.touch.track.height`
pub const SIZE_SWITCH_TOUCH_TRACK_HEIGHT: f32 = 20.0;
/// `size.switch.touch.knob`
pub const SIZE_SWITCH_TOUCH_KNOB: f32 = 14.0;
/// `size.scrollbar.thickness`
pub const SIZE_SCROLLBAR_THICKNESS: f32 = 6.0;
/// `size.scrollbar.min-thumb`
pub const SIZE_SCROLLBAR_MIN_THUMB: f32 = 24.0;
/// `size.composer.min-height`
pub const SIZE_COMPOSER_MIN_HEIGHT: f32 = 44.0;
/// `size.composer.max-height`
pub const SIZE_COMPOSER_MAX_HEIGHT: f32 = 176.0;
/// `size.composer.start-min-height`
pub const SIZE_COMPOSER_START_MIN_HEIGHT: f32 = 66.0;
/// `size.minimap.track`
pub const SIZE_MINIMAP_TRACK: f32 = 24.0;
/// `size.minimap.tick.sm`
pub const SIZE_MINIMAP_TICK_SM: f32 = 8.0;
/// `size.minimap.tick.md`
pub const SIZE_MINIMAP_TICK_MD: f32 = 12.0;
/// `size.minimap.tick.lg`
pub const SIZE_MINIMAP_TICK_LG: f32 = 16.0;
/// `size.appbar.android`
pub const SIZE_APPBAR_ANDROID: f32 = 56.0;
/// `size.titlebar.control.width`
pub const SIZE_TITLEBAR_CONTROL_WIDTH: f32 = 46.0;
/// `size.usage.ring`
pub const SIZE_USAGE_RING: f32 = 32.0;
/// `size.sheet.handle.width`
pub const SIZE_SHEET_HANDLE_WIDTH: f32 = 32.0;
/// `size.sheet.handle.height`
pub const SIZE_SHEET_HANDLE_HEIGHT: f32 = 4.0;
/// `size.row.two-line`
pub const SIZE_ROW_TWO_LINE: f32 = 44.0;
/// `size.row.two-line.touch`
pub const SIZE_ROW_TWO_LINE_TOUCH: f32 = 56.0;
/// `size.row.settings`
pub const SIZE_ROW_SETTINGS: f32 = 40.0;
/// `size.row.settings.helper`
pub const SIZE_ROW_SETTINGS_HELPER: f32 = 48.0;
/// `size.diff.gutter`
pub const SIZE_DIFF_GUTTER: f32 = 40.0;
/// `size.diff.marker`
pub const SIZE_DIFF_MARKER: f32 = 20.0;
/// `size.field.multiline.min`
pub const SIZE_FIELD_MULTILINE_MIN: f32 = 68.0;

// layout
/// `layout.sidebar.default`
pub const LAYOUT_SIDEBAR_DEFAULT: f32 = 256.0;
/// `layout.sidebar.min`
pub const LAYOUT_SIDEBAR_MIN: f32 = 200.0;
/// `layout.sidebar.max`
pub const LAYOUT_SIDEBAR_MAX: f32 = 400.0;
/// `layout.conversation.min`
pub const LAYOUT_CONVERSATION_MIN: f32 = 360.0;
/// `layout.conversation.reading`
pub const LAYOUT_CONVERSATION_READING: f32 = 720.0;
/// `layout.summary.default`
pub const LAYOUT_SUMMARY_DEFAULT: f32 = 288.0;
/// `layout.summary.min`
pub const LAYOUT_SUMMARY_MIN: f32 = 240.0;
/// `layout.summary.max`
pub const LAYOUT_SUMMARY_MAX: f32 = 400.0;
/// `layout.workpanel.default`
pub const LAYOUT_WORKPANEL_DEFAULT: f32 = 480.0;
/// `layout.workpanel.min`
pub const LAYOUT_WORKPANEL_MIN: f32 = 320.0;
/// `layout.project.default`
pub const LAYOUT_PROJECT_DEFAULT: f32 = 288.0;
/// `layout.project.min`
pub const LAYOUT_PROJECT_MIN: f32 = 240.0;
/// `layout.project.max`
pub const LAYOUT_PROJECT_MAX: f32 = 480.0;
/// `layout.window.min-width`
pub const LAYOUT_WINDOW_MIN_WIDTH: f32 = 800.0;
/// `layout.window.min-height`
pub const LAYOUT_WINDOW_MIN_HEIGHT: f32 = 520.0;
/// `layout.page.settings`
pub const LAYOUT_PAGE_SETTINGS: f32 = 880.0;
/// `layout.page.default`
pub const LAYOUT_PAGE_DEFAULT: f32 = 1120.0;
/// `layout.android.master`
pub const LAYOUT_ANDROID_MASTER: f32 = 360.0;
/// `layout.android.medium`
pub const LAYOUT_ANDROID_MEDIUM: f32 = 600.0;
/// `layout.android.expanded`
pub const LAYOUT_ANDROID_EXPANDED: f32 = 840.0;
/// `layout.sheet.max-height`
pub const LAYOUT_SHEET_MAX_HEIGHT: Measure = Measure::Percent(90.0);
/// `layout.dialog.width.sm`
pub const LAYOUT_DIALOG_WIDTH_SM: f32 = 400.0;
/// `layout.dialog.width.md`
pub const LAYOUT_DIALOG_WIDTH_MD: f32 = 440.0;
/// `layout.dialog.width.lg`
pub const LAYOUT_DIALOG_WIDTH_LG: f32 = 560.0;
/// `layout.dialog.android.min`
pub const LAYOUT_DIALOG_ANDROID_MIN: f32 = 280.0;
/// `layout.dialog.android.max`
pub const LAYOUT_DIALOG_ANDROID_MAX: f32 = 560.0;
/// `layout.popover.width.sm`
pub const LAYOUT_POPOVER_WIDTH_SM: f32 = 280.0;
/// `layout.popover.width.md`
pub const LAYOUT_POPOVER_WIDTH_MD: f32 = 320.0;
/// `layout.popover.width.lg`
pub const LAYOUT_POPOVER_WIDTH_LG: f32 = 360.0;
/// `layout.picker.width`
pub const LAYOUT_PICKER_WIDTH: f32 = 320.0;
/// `layout.model-picker.width`
pub const LAYOUT_MODEL_PICKER_WIDTH: f32 = 560.0;
/// `layout.model-picker.provider-column`
pub const LAYOUT_MODEL_PICKER_PROVIDER_COLUMN: f32 = 48.0;
/// `layout.model-picker.model-column`
pub const LAYOUT_MODEL_PICKER_MODEL_COLUMN: f32 = 296.0;
/// `layout.model-picker.reasoning-column`
pub const LAYOUT_MODEL_PICKER_REASONING_COLUMN: f32 = 216.0;
/// `layout.model-picker.max-height`
pub const LAYOUT_MODEL_PICKER_MAX_HEIGHT: f32 = 420.0;
/// `layout.model-picker.chip.max-width`
pub const LAYOUT_MODEL_PICKER_CHIP_MAX_WIDTH: f32 = 220.0;
/// `layout.menu.min-width`
pub const LAYOUT_MENU_MIN_WIDTH: f32 = 180.0;
/// `layout.menu.max-width`
pub const LAYOUT_MENU_MAX_WIDTH: f32 = 280.0;
/// `layout.tooltip.max-width`
pub const LAYOUT_TOOLTIP_MAX_WIDTH: f32 = 240.0;
/// `layout.toast.width.min`
pub const LAYOUT_TOAST_WIDTH_MIN: f32 = 320.0;
/// `layout.toast.width.default`
pub const LAYOUT_TOAST_WIDTH_DEFAULT: f32 = 360.0;
/// `layout.toast.width.max`
pub const LAYOUT_TOAST_WIDTH_MAX: f32 = 400.0;
/// `layout.palette.width`
pub const LAYOUT_PALETTE_WIDTH: f32 = 600.0;
/// `layout.palette.input-height`
pub const LAYOUT_PALETTE_INPUT_HEIGHT: f32 = 44.0;
/// `layout.palette.max-height`
pub const LAYOUT_PALETTE_MAX_HEIGHT: f32 = 480.0;
/// `layout.palette.top`
pub const LAYOUT_PALETTE_TOP: f32 = 96.0;
/// `layout.findbar.width`
pub const LAYOUT_FINDBAR_WIDTH: f32 = 360.0;
/// `layout.tab.max-width`
pub const LAYOUT_TAB_MAX_WIDTH: f32 = 200.0;
/// `layout.calendar.column.min`
pub const LAYOUT_CALENDAR_COLUMN_MIN: f32 = 96.0;
/// `layout.calendar.hour`
pub const LAYOUT_CALENDAR_HOUR: f32 = 48.0;
/// `layout.board.column.width`
pub const LAYOUT_BOARD_COLUMN_WIDTH: f32 = 280.0;
/// `layout.settings.control.min`
pub const LAYOUT_SETTINGS_CONTROL_MIN: f32 = 160.0;
/// `layout.settings.control.max`
pub const LAYOUT_SETTINGS_CONTROL_MAX: f32 = 320.0;
/// `layout.message.bubble.max-width`
pub const LAYOUT_MESSAGE_BUBBLE_MAX_WIDTH: Measure = Measure::Percent(85.0);
/// `layout.empty-state.text.max-width`
pub const LAYOUT_EMPTY_STATE_TEXT_MAX_WIDTH: Measure = Measure::Ch(60.0);

// border
/// `border.width.default`
pub const BORDER_WIDTH_DEFAULT: f32 = 1.0;
/// `border.width.thick`
pub const BORDER_WIDTH_THICK: f32 = 2.0;
/// `border.focus.width`
pub const BORDER_FOCUS_WIDTH: f32 = 2.0;
/// `border.focus.offset`
pub const BORDER_FOCUS_OFFSET: f32 = 2.0;
/// `border.splitter.hit`
pub const BORDER_SPLITTER_HIT: f32 = 6.0;

// duration
/// `duration.instant`
pub const DURATION_INSTANT: Duration = Duration::from_millis(0);
/// `duration.fast`
pub const DURATION_FAST: Duration = Duration::from_millis(100);
/// `duration.base`
pub const DURATION_BASE: Duration = Duration::from_millis(150);
/// `duration.moderate`
pub const DURATION_MODERATE: Duration = Duration::from_millis(200);
/// `duration.slow`
pub const DURATION_SLOW: Duration = Duration::from_millis(300);
/// `duration.card-insert`
pub const DURATION_CARD_INSERT: Duration = Duration::from_millis(250);
/// `duration.tooltip-delay`
pub const DURATION_TOOLTIP_DELAY: Duration = Duration::from_millis(400);
/// `duration.spinner`
pub const DURATION_SPINNER: Duration = Duration::from_millis(800);

// easing
/// `ease.standard`
pub const EASE_STANDARD: Easing = Easing::CubicBezier(0.2, 0.0, 0.0, 1.0);
/// `ease.enter`
pub const EASE_ENTER: Easing = Easing::CubicBezier(0.16, 1.0, 0.3, 1.0);
/// `ease.exit`
pub const EASE_EXIT: Easing = Easing::CubicBezier(0.3, 0.0, 0.8, 0.15);
/// `ease.linear`
pub const EASE_LINEAR: Easing = Easing::Linear;

// zIndex
/// `z.base`
pub const Z_BASE: u16 = 0;
/// `z.sticky`
pub const Z_STICKY: u16 = 10;
/// `z.region-overlay`
pub const Z_REGION_OVERLAY: u16 = 30;
/// `z.popover`
pub const Z_POPOVER: u16 = 40;
/// `z.dialog`
pub const Z_DIALOG: u16 = 50;
/// `z.toast`
pub const Z_TOAST: u16 = 60;

// contrast
/// `terminal.min-contrast`
pub const TERMINAL_MIN_CONTRAST: f32 = 4.5;

