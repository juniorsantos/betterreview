//! GitHub Dark High Contrast palette.
//!
//! Values follow @primer/primitives dark_high_contrast; diff backgrounds are
//! the muted overlay colors pre-blended onto the canvas color.

use ratatui::style::Color;

pub const BG: Color = Color::Rgb(0x0a, 0x0c, 0x10);
pub const FG: Color = Color::Rgb(0xf0, 0xf3, 0xf6);
pub const MUTED: Color = Color::Rgb(0x9e, 0xa7, 0xb3);
pub const BORDER: Color = Color::Rgb(0x7a, 0x82, 0x8e);
pub const ACCENT: Color = Color::Rgb(0x71, 0xb7, 0xff);
pub const SUCCESS: Color = Color::Rgb(0x26, 0xcd, 0x4d);
pub const DANGER: Color = Color::Rgb(0xff, 0x6a, 0x69);
pub const WARNING: Color = Color::Rgb(0xf0, 0xb7, 0x2f);
pub const CURSOR_LINE: Color = Color::Rgb(0x27, 0x2b, 0x33);
pub const SELECTION: Color = Color::Rgb(0x14, 0x3d, 0x79);

/// Diff line backgrounds handed to delta (hex, without the leading `#`
/// escaping) — keep in sync with the constants above.
pub const DELTA_PLUS_BG: &str = "#0e2919";
pub const DELTA_MINUS_BG: &str = "#2f1a1d";
pub const DELTA_PLUS_EMPH_BG: &str = "#1a4c2a";
pub const DELTA_MINUS_EMPH_BG: &str = "#58212a";
