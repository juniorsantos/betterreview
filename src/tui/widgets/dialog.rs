//! Unified modal component: a square box with the title on the border and a
//! standardized muted hint footer as the last inner line. Every dialog in the
//! TUI (quit, delete, help, submit, editor) renders through this so the look
//! stays identical everywhere.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::{text::display_width, theme};

pub struct Dialog<'a> {
    /// e.g. " Delete comment "
    pub title: Line<'a>,
    /// Content lines, NOT including the hints footer.
    pub body: Vec<Line<'a>>,
    /// e.g. "j/k move · Enter confirm · Esc cancel"
    pub hints: &'a str,
    pub sizing: Sizing,
    /// Vertical regions the interior is split into, above the hints row. An
    /// empty list means one region holding the whole body.
    pub zones: Vec<Zone>,
}

/// How a dialog decides its outer size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sizing {
    /// Exact dimensions, still clamped to the area.
    Fixed { width: u16, height: u16 },
    /// Grows with the content up to `max_width`, and as tall as the body needs.
    Content { max_width: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// Takes whatever is left after the fixed zones.
    Fill,
    /// Exactly this many rows.
    Fixed(u16),
}

impl Dialog<'_> {
    fn outer(&self, area: Rect) -> (u16, u16) {
        let (width, height) = match self.sizing {
            Sizing::Fixed { width, height } => (width, height),
            Sizing::Content { max_width } => {
                let widest = self
                    .body
                    .iter()
                    .map(|line| line.width())
                    .chain([self.title.width(), display_width(self.hints)])
                    .max()
                    .unwrap_or(0);
                // Borders (2), the breathing space every body line gets (1),
                // and one column of slack on the right.
                let width = u16::try_from(widest + 4).unwrap_or(u16::MAX);
                let height = u16::try_from(self.body.len() + 4).unwrap_or(u16::MAX);
                (width.min(max_width), height)
            }
        };
        (
            clamped_width(width, area.width),
            height.min(((area.height as u32) * 4 / 5) as u16).max(1),
        )
    }
}

pub(in crate::tui) fn clamped_width(width: u16, area_width: u16) -> u16 {
    width.min(((area_width as u32) * 4 / 5) as u16).max(1)
}

/// Renders `dialog` centered in `area` (clamped to 80% of it) and returns the
/// inner body `Rect` — the region above the blank/hints rows, for callers
/// that need to position the terminal cursor without overlapping the hints
/// line.
pub fn render_dialog(frame: &mut Frame, area: Rect, dialog: Dialog) -> Vec<Rect> {
    let (width, height) = dialog.outer(area);
    let outer = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    let block = Block::default()
        .title(dialog.title)
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme::ACCENT));
    let inner = block.inner(outer);

    let inner_height = inner.height as usize;
    let has_blank = inner_height >= 3;
    let hint_rows = usize::from(inner_height >= 1);
    let blank_rows = usize::from(has_blank);
    let body_capacity = inner_height.saturating_sub(hint_rows + blank_rows);
    let body_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: body_capacity as u16,
    };

    let mut lines = dialog.body;
    lines.truncate(body_capacity);
    // One column of breathing room against the left border; selected rows
    // keep their background full-bleed because the prefix inherits the
    // line's style.
    for line in &mut lines {
        if !line.spans.is_empty() {
            line.spans.insert(0, Span::raw(" "));
        }
    }
    while lines.len() < body_capacity {
        lines.push(Line::raw(""));
    }
    for _ in 0..blank_rows {
        lines.push(Line::raw(""));
    }
    if hint_rows > 0 {
        lines.push(centered_hint_line(dialog.hints, inner.width));
    }

    // Any line carrying its own background (e.g. a selected menu row) needs
    // trailing padding so that background reaches the panel's right edge —
    // same convention used by the diff/files panels for cursor highlights.
    for line in &mut lines {
        if line.style.bg.is_some() {
            let text_width = line.width();
            if text_width < inner.width as usize {
                line.spans
                    .push(Span::raw(" ".repeat(inner.width as usize - text_width)));
            }
        }
    }

    frame.render_widget(Clear, outer);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(theme::FG).bg(theme::BG))
            .block(block),
        outer,
    );

    split_zones(body_rect, &dialog.zones)
}

/// Splits the body region into the requested zones, top to bottom. With no
/// zones the whole region is returned as a single rect, which is what every
/// caller that just stacks lines wants.
fn split_zones(body: Rect, zones: &[Zone]) -> Vec<Rect> {
    if zones.is_empty() {
        return vec![body];
    }
    let constraints: Vec<Constraint> = zones
        .iter()
        .map(|zone| match zone {
            Zone::Fill => Constraint::Min(1),
            Zone::Fixed(rows) => Constraint::Length(*rows),
        })
        .collect();
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(body)
        .to_vec()
}

#[derive(Clone, Copy)]
pub(in crate::tui) struct ActionButton<'a> {
    pub label: &'a str,
    pub selected: bool,
    pub enabled: bool,
}

pub(in crate::tui) fn outlined_button_rows(
    buttons: &[ActionButton<'_>],
    gap: usize,
    padding: usize,
) -> Vec<Line<'static>> {
    (0..3)
        .map(|row| {
            let mut spans = Vec::new();
            for (index, button) in buttons.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::styled(
                        " ".repeat(gap),
                        Style::default().bg(theme::BG),
                    ));
                }
                let background = if button.selected {
                    theme::ACCENT
                } else if button.enabled {
                    theme::BORDER
                } else {
                    theme::FILLER
                };
                let border_style = Style::default().fg(background).bg(background);
                let mut label_style = Style::default()
                    .fg(if button.enabled {
                        theme::BG
                    } else {
                        theme::MUTED
                    })
                    .bg(background);
                if button.selected {
                    label_style = label_style.add_modifier(Modifier::BOLD);
                }
                let inside_width = display_width(button.label) + padding * 2;
                match row {
                    0 => spans.push(Span::styled(
                        format!("┌{}┐", "─".repeat(inside_width)),
                        border_style,
                    )),
                    1 => {
                        spans.push(Span::styled("│", border_style));
                        spans.push(Span::styled(
                            format!(
                                "{}{}{}",
                                " ".repeat(padding),
                                button.label,
                                " ".repeat(padding)
                            ),
                            label_style,
                        ));
                        spans.push(Span::styled("│", border_style));
                    }
                    _ => spans.push(Span::styled(
                        format!("└{}┘", "─".repeat(inside_width)),
                        border_style,
                    )),
                }
            }
            Line::from(spans)
        })
        .collect()
}

pub(in crate::tui) fn center_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|mut line| {
            let left_pad = width.saturating_sub(line.width()) / 2;
            if left_pad > 0 {
                line.spans.insert(
                    0,
                    Span::styled(" ".repeat(left_pad), Style::default().bg(theme::BG)),
                );
            }
            line
        })
        .collect()
}

/// Centers `hints` horizontally within `width` columns, in `theme::MUTED`.
fn centered_hint_line(hints: &str, width: u16) -> Line<'static> {
    let hint_width = display_width(hints);
    let left_pad = (width as usize).saturating_sub(hint_width) / 2;
    let mut spans = vec![Span::raw(" ".repeat(left_pad))];
    // Same key styling as everywhere else: the key in accent bold, the
    // label muted, separators dimmed.
    for (index, pair) in hints.split(" · ").enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme::BORDER)));
        }
        match pair.split_once(' ') {
            Some((key, label)) => {
                spans.push(Span::styled(
                    key.to_owned(),
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(" {label}"),
                    Style::default().fg(theme::MUTED),
                ));
            }
            None => spans.push(Span::styled(
                pair.to_owned(),
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )),
        }
    }
    Line::from(spans)
}
