use ratatui::style::{Color, Style};
use std::sync::atomic::{AtomicBool, Ordering};

static TRANSPARENT: AtomicBool = AtomicBool::new(false);

pub fn set_transparent(value: bool) {
    TRANSPARENT.store(value, Ordering::Relaxed);
}

pub fn canvas() -> Style {
    let style = Style::default().fg(FG);
    if TRANSPARENT.load(Ordering::Relaxed) {
        style
    } else {
        style.bg(BG)
    }
}

pub const BG: Color = Color::Rgb(0x0a, 0x0c, 0x10);
pub const FG: Color = Color::Rgb(0xf0, 0xf3, 0xf6);
pub const MUTED: Color = Color::Rgb(0x73, 0x7a, 0x83);
pub const BORDER: Color = Color::Rgb(0x40, 0x44, 0x4b);
pub const FILLER: Color = Color::Rgb(0x24, 0x27, 0x2b);
pub const ACCENT: Color = Color::Rgb(0x00, 0x89, 0xaa);
pub const ACCENT_SOFT: Color = Color::Rgb(0x4c, 0xac, 0xc3);
pub const SUCCESS: Color = Color::Rgb(0x26, 0xcd, 0x4d);
pub const DANGER: Color = Color::Rgb(0xff, 0x6a, 0x69);
pub const WARNING: Color = Color::Rgb(0xf0, 0xb7, 0x2f);
pub const COMMENT: Color = Color::Rgb(0xc7, 0x9b, 0xff);
pub const MODAL_BG: Color = Color::Rgb(0x15, 0x19, 0x22);
pub const COMMENT_BG: Color = Color::Rgb(0x20, 0x18, 0x2c);
pub const COMMENT_REPLY_BG: Color = Color::Rgb(0x13, 0x21, 0x28);
pub const BUTTON_SUCCESS: Color = Color::Rgb(0x31, 0x5c, 0x3d);
pub const BUTTON_WARNING: Color = Color::Rgb(0x6b, 0x55, 0x2e);
pub const BUTTON_COMMENT: Color = Color::Rgb(0x59, 0x46, 0x6d);
pub const BUTTON_DANGER: Color = Color::Rgb(0x6b, 0x38, 0x3b);
pub const BUTTON_NEUTRAL: Color = Color::Rgb(0x30, 0x34, 0x3a);
pub const MOVED: Color = Color::Rgb(0x2a, 0x33, 0x52);
pub const CURSOR_LINE: Color = Color::Rgb(0x27, 0x2b, 0x33);
pub const SELECTION: Color = Color::Rgb(0x14, 0x3d, 0x79);

/// Diff line backgrounds handed to delta (hex, without the leading `#`
/// escaping) — keep in sync with the constants above.
pub const DELTA_PLUS_BG: &str = "#1c4428";
pub const DELTA_MINUS_BG: &str = "#5c2124";
pub const DELTA_PLUS_EMPH_BG: &str = "#2d7a4a";
pub const DELTA_MINUS_EMPH_BG: &str = "#8b2f38";
