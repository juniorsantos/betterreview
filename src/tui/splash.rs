use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use super::theme;

const BANNER: &str = include_str!("../../assets/banner.txt");
const BANNER_WIDTH: u16 = 50;
const BANNER_HEIGHT: u16 = 12;
const CHIP_WIDTH: u16 = 20;

pub fn splash(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme::canvas()), area);
    let (lines, width) = if fits_banner(area) {
        (banner_lines(), BANNER_WIDTH)
    } else {
        (chip_lines(), CHIP_WIDTH)
    };
    let height = u16::try_from(lines.len()).unwrap_or(BANNER_HEIGHT);
    let centered = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    };
    frame.render_widget(Paragraph::new(lines).centered(), centered);
}

fn fits_banner(area: Rect) -> bool {
    area.width >= BANNER_WIDTH + 4 && area.height >= BANNER_HEIGHT + 8
}

fn banner_lines() -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = BANNER
        .lines()
        .map(|row| {
            Line::from(Span::styled(
                row.to_owned(),
                Style::default().fg(theme::ACCENT),
            ))
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        concat!("BetterReview · v", env!("CARGO_PKG_VERSION")),
        Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(""));
    lines.push(loading_line());
    lines
}

fn chip_lines() -> Vec<Line<'static>> {
    vec![
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
        loading_line(),
    ]
}

fn loading_line() -> Line<'static> {
    Line::from(Span::styled(
        "⠋ loading…",
        Style::default().fg(theme::MUTED),
    ))
}
