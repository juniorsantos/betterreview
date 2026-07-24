use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Clear, Paragraph},
};

use crate::app::{AppFocus, AppState};

use super::{
    theme,
    widgets::{delete, diff, editor, files, header, help, quit, status, submit, threads},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BG).fg(theme::FG)),
        area,
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let middle = format!(
        " {} #{} · {} · @{} ",
        state.provider.key.repository,
        state.provider.key.number,
        state.provider.title,
        state.provider.author
    );
    frame.render_widget(
        Paragraph::new(header::chip_line(&middle, area.width)),
        rows[0],
    );

    if area.width >= 80 {
        let files_width = if state.files_expanded { 50 } else { 30 };
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(files_width), Constraint::Min(1)])
            .split(rows[1]);
        files::render(frame, columns[0], state);
        diff::render(frame, columns[1], state);
    } else {
        diff::render(frame, rows[1], state);
        if state.focus == AppFocus::Files {
            let overlay = inset(rows[1], 2, 1);
            frame.render_widget(Clear, overlay);
            files::render(frame, overlay, state);
        }
    }

    status::render(frame, rows[2], state);

    help::render(frame, area, state);
    threads::render(frame, area, state);
    editor::render(frame, area, state);
    submit::render(frame, area, state);
    if state.quit_dialog {
        quit::render(frame, area, state);
    }
    if state.delete_dialog.is_some() {
        delete::render(frame, area, state);
    }
}

fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(horizontal),
        y: area.y.saturating_add(vertical),
        width: area.width.saturating_sub(horizontal.saturating_mul(2)),
        height: area.height.saturating_sub(vertical.saturating_mul(2)),
    }
}
