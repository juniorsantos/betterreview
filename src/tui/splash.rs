use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use super::theme;

/// Opening screen shown while the review context loads (doctor + listing):
/// the app chip, the version, and a loading hint, centered.
pub fn splash(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BG).fg(theme::FG)),
        area,
    );
    let lines = vec![
        Line::from(Span::styled(
            " betterreview ",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            concat!("v", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme::MUTED),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "⠋ carregando…",
            Style::default().fg(theme::MUTED),
        )),
    ];
    let height = u16::try_from(lines.len()).unwrap_or(5);
    let width = 20u16;
    let centered = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    };
    frame.render_widget(Paragraph::new(lines).centered(), centered);
}
