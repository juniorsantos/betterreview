use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::AppState;

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(editor) = &state.session.editor else {
        return;
    };
    let width = area.width.saturating_sub(6).min(76);
    let height = area.height.saturating_sub(4).min(14);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    let title = if editor.stale {
        " Stale editor — read only "
    } else {
        " Comment editor — Ctrl-S save / Esc close "
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(editor.lines.join("\n"))
            .block(Block::default().title(title).borders(Borders::ALL)),
        popup,
    );
}
