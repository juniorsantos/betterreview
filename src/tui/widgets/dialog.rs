//! Unified modal component: a rounded box with the title on the border and a
//! standardized muted hint footer as the last inner line. Every dialog in the
//! TUI (quit, delete, help, submit, editor) renders through this so the look
//! stays identical everywhere.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::tui::theme;

pub(in crate::tui) struct Dialog<'a> {
    /// e.g. " Excluir comentário "
    pub title: &'a str,
    /// Content lines, NOT including the hints footer.
    pub body: Vec<Line<'a>>,
    /// e.g. "j/k mover · Enter confirmar · Esc cancelar"
    pub hints: &'a str,
    /// Desired outer width, clamped to 80% of `area`.
    pub width: u16,
    /// Desired outer height, clamped to 80% of `area`.
    pub height: u16,
}

/// Renders `dialog` centered in `area` (clamped to 80% of it) and returns the
/// inner body `Rect` — the region above the blank/hints rows, for callers
/// that need to position the terminal cursor without overlapping the hints
/// line.
pub(in crate::tui) fn render_dialog(frame: &mut Frame, area: Rect, dialog: Dialog) -> Rect {
    let width = dialog
        .width
        .min(((area.width as u32) * 4 / 5) as u16)
        .max(1);
    let height = dialog
        .height
        .min(((area.height as u32) * 4 / 5) as u16)
        .max(1);
    let outer = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    let block = Block::default()
        .title(dialog.title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
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
            .style(Style::default().bg(theme::BG).fg(theme::FG))
            .block(block),
        outer,
    );

    body_rect
}

/// One row of a menu-style dialog (quit/delete): the selected row carries a
/// `▶ ` marker, a `theme::SELECTION` background and bold text spanning the
/// whole row (padding to the inner width happens in `render_dialog`);
/// unselected rows are plain text with a two-space indent.
pub(in crate::tui) fn menu_line(label: &str, selected: bool) -> Line<'static> {
    if selected {
        // The background must live on the Line itself (not a Span) so
        // `render_dialog`'s fill-to-width padding below can detect and
        // extend it — matching the diff/files panel highlight convention.
        Line::raw(format!("▶ {label}")).style(
            Style::default()
                .bg(theme::SELECTION)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Line::raw(format!("  {label}"))
    }
}

/// Centers `hints` horizontally within `width` columns, in `theme::MUTED`.
fn centered_hint_line(hints: &str, width: u16) -> Line<'static> {
    let hint_width = hints.chars().count();
    let left_pad = (width as usize).saturating_sub(hint_width) / 2;
    Line::from(vec![
        Span::raw(" ".repeat(left_pad)),
        Span::styled(hints.to_owned(), Style::default().fg(theme::MUTED)),
    ])
}
