use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::{AppFocus, AppState},
    domain::PatchAvailability,
    tui::{theme, viewport},
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let lines = match &state.rendered_diff {
        Some(diff) => diff
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                // One gutter column: the line number on the side the row
                // exists on (new side wins when both are present).
                let number = row
                    .binding
                    .right
                    .as_ref()
                    .or(row.binding.left.as_ref())
                    .map(|position| position.line.to_string())
                    .unwrap_or_default();
                let mut spans = vec![Span::styled(
                    format!("{number:>5} "),
                    Style::default().fg(theme::MUTED),
                )];
                spans.extend(row.text.spans.clone());
                let selected = state.selection_anchor.is_some_and(|anchor| {
                    let start = anchor.min(state.session.cursor_row);
                    let end = anchor.max(state.session.cursor_row);
                    (start..=end).contains(&index)
                });
                let mut line = Line::from(spans);
                let style = if index == state.session.cursor_row {
                    Some(
                        Style::default()
                            .bg(theme::CURSOR_LINE)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if selected {
                    Some(Style::default().bg(theme::SELECTION))
                } else {
                    None
                };
                if let Some(style) = style {
                    // Pad so the background reaches the panel's right edge.
                    let text_width = line.width();
                    if text_width < inner_width {
                        line.spans
                            .push(Span::raw(" ".repeat(inner_width - text_width)));
                    }
                    line = line.style(style);
                }
                line
            })
            .collect(),
        None => vec![Line::raw(unavailable_reason(state))],
    };
    let border = if state.focus == AppFocus::Diff {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    let visible = area.height.saturating_sub(2) as usize;
    let start = viewport::start(state.session.cursor_row, lines.len(), visible);
    let scroll = u16::try_from(start).unwrap_or(u16::MAX);
    frame.render_widget(
        Paragraph::new(lines).scroll((scroll, 0)).block(
            Block::default()
                .title(" Diff ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
}

fn unavailable_reason(state: &AppState) -> String {
    let Some(file) = state.provider.files.get(state.active_file_index) else {
        return "No changed files".into();
    };
    match &file.patch {
        PatchAvailability::Available(_) => "Loading diff...".into(),
        PatchAvailability::Binary => "Binary file: inline review unavailable".into(),
        PatchAvailability::TooLarge => "Patch is too large for inline review".into(),
        PatchAvailability::Collapsed => "Patch was collapsed by the provider".into(),
        PatchAvailability::Truncated { reason } => format!("Patch unavailable: {reason}"),
    }
}
