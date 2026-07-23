use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::{AppFocus, AppState},
    domain::PatchAvailability,
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let lines = match &state.rendered_diff {
        Some(diff) => diff
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let old = row
                    .binding
                    .left
                    .as_ref()
                    .map(|position| position.line.to_string())
                    .unwrap_or_default();
                let new = row
                    .binding
                    .right
                    .as_ref()
                    .map(|position| position.line.to_string())
                    .unwrap_or_default();
                let mut spans = vec![Span::raw(format!("{old:>4} {new:>4} "))];
                spans.extend(row.text.spans.clone());
                let selected = state.selection_anchor.is_some_and(|anchor| {
                    let start = anchor.min(state.session.cursor_row);
                    let end = anchor.max(state.session.cursor_row);
                    (start..=end).contains(&index)
                });
                let style = if index == state.session.cursor_row {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if selected {
                    Style::default().bg(Color::Blue)
                } else {
                    Style::default()
                };
                Line::from(spans).style(style)
            })
            .collect(),
        None => vec![Line::raw(unavailable_reason(state))],
    };
    let border = if state.focus == AppFocus::Diff {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((state.session.scroll_row as u16, 0))
            .block(
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
