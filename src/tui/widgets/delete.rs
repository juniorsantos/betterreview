use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{app::AppState, tui::theme};

const OPTIONS: [&str; 2] = ["Excluir", "Cancelar"];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let width = area.width.saturating_sub(4).min(52);
    let height = 6.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    let mut lines: Vec<Line> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, option)| {
            if index == state.delete_selected {
                Line::styled(
                    format!("▸ {option}"),
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::raw(format!("  {option}"))
            }
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "j/k move  Enter confirm  Esc cancel",
        Style::default().fg(theme::MUTED),
    ));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme::BG).fg(theme::FG))
            .block(
                Block::default()
                    .title(" Excluir comentário? ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::ACCENT)),
            ),
        popup,
    );
}
