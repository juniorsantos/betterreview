use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Clear, Paragraph},
};

use crate::app::{AppFocus, AppState};

use super::{
    layout::{ScreenLayout, header_row, screen_layout, status_row},
    theme,
    widgets::{delete, diff, editor, files, header, help, quit, status, submit, threads},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BG).fg(theme::FG)),
        area,
    );

    let middle = format!(
        " {} #{} · {} · @{} ",
        state.provider.key.repository,
        state.provider.key.number,
        state.provider.title,
        state.provider.author
    );
    frame.render_widget(
        Paragraph::new(header::chip_line(&middle, area.width)),
        header_row(area),
    );

    let ScreenLayout {
        files: files_rect,
        diff: diff_rect,
    } = screen_layout(area, state);
    match files_rect {
        Some(files_rect) => {
            files::render(frame, files_rect, state);
            diff::render(frame, diff_rect, state);
        }
        None => {
            diff::render(frame, diff_rect, state);
            if state.focus == AppFocus::Files {
                let overlay = inset(diff_rect, 2, 1);
                frame.render_widget(Clear, overlay);
                files::render(frame, overlay, state);
            }
        }
    }

    status::render(frame, status_row(area), state);

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
