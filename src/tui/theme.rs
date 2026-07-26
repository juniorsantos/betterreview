//! GitHub Dark High Contrast palette, re-tiered by role.
//!
//! Content colors follow @primer/primitives dark_high_contrast, and the diff
//! backgrounds are its muted overlays pre-blended onto the canvas. The
//! decoration tokens deliberately do not: that palette raises every value,
//! including the ones whose job is to recede, and a terminal has neither
//! whitespace nor font weight left to rebuild the hierarchy with. `BORDER`,
//! `MUTED` and `FILLER` are therefore scaled down along their own hue until
//! they sit at the contrast their role calls for — see `tests/theme_contrast.rs`,
//! which enforces the tiering rather than the hex values.

use ratatui::style::Color;

pub const BG: Color = Color::Rgb(0x0a, 0x0c, 0x10);
pub const FG: Color = Color::Rgb(0xf0, 0xf3, 0xf6);
pub const MUTED: Color = Color::Rgb(0x73, 0x7a, 0x83);
pub const BORDER: Color = Color::Rgb(0x40, 0x44, 0x4b);
pub const FILLER: Color = Color::Rgb(0x24, 0x27, 0x2b);
pub const ACCENT: Color = Color::Rgb(0x71, 0xb7, 0xff);
pub const SUCCESS: Color = Color::Rgb(0x26, 0xcd, 0x4d);
pub const DANGER: Color = Color::Rgb(0xff, 0x6a, 0x69);
pub const WARNING: Color = Color::Rgb(0xf0, 0xb7, 0x2f);
pub const COMMENT: Color = Color::Rgb(0xc7, 0x9b, 0xff);
pub const CURSOR_LINE: Color = Color::Rgb(0x27, 0x2b, 0x33);
pub const SELECTION: Color = Color::Rgb(0x14, 0x3d, 0x79);

/// Diff line backgrounds handed to delta (hex, without the leading `#`
/// escaping) — keep in sync with the constants above.
pub const DELTA_PLUS_BG: &str = "#1c4428";
pub const DELTA_MINUS_BG: &str = "#5c2124";
pub const DELTA_PLUS_EMPH_BG: &str = "#2d7a4a";
pub const DELTA_MINUS_EMPH_BG: &str = "#8b2f38";
