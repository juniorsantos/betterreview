use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Clear, Paragraph},
};

use crate::app::{AppFocus, AppState};
use crate::providers::ReviewLinks;

use super::{
    hyperlink,
    layout::{ScreenLayout, header_row, screen_layout, status_row},
    text::display_width,
    theme,
    widgets::{blocked, delete, diff, editor, files, header, help, quit, status, submit, threads},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme::canvas()), area);

    let head = state.provider.head.0.chars().take(7).collect::<String>();
    let middle = format!(
        " {} #{} · {} · {} · @{} ",
        state.provider.key.repository,
        state.provider.key.number,
        head,
        state.provider.title,
        state.provider.author
    );
    frame.render_widget(
        Paragraph::new(header::chip_line(&middle, area.width)),
        header_row(area),
    );
    if let Some(links) = ReviewLinks::new(&state.provider.key, &state.provider.web_url) {
        let review_label = format!("#{}", state.provider.key.number);
        let review_x = area.x.saturating_add(
            u16::try_from(
                display_width(header::NAME_CHIP)
                    + display_width(&format!(" {} ", state.provider.key.repository)),
            )
            .unwrap_or(u16::MAX),
        );
        apply_header_link(frame, area, review_x, &review_label, links.review_url());

        let head_x = review_x.saturating_add(
            u16::try_from(display_width(&review_label) + display_width(" · ")).unwrap_or(u16::MAX),
        );
        apply_header_link(
            frame,
            area,
            head_x,
            &head,
            &links.commit_url(&state.provider.head),
        );
    }

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
    if state.blocked.is_some() {
        blocked::render(frame, area, state);
    }
    if state.delete_dialog.is_some() {
        delete::render(frame, area, state);
    }
}

fn apply_header_link(frame: &mut Frame, area: Rect, x: u16, label: &str, target: &str) {
    let right = area.right();
    if x >= right {
        return;
    }
    let width = u16::try_from(display_width(label))
        .unwrap_or(u16::MAX)
        .min(right - x);
    hyperlink::apply(frame, Rect::new(x, area.y, width, 1), target);
}

fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(horizontal),
        y: area.y.saturating_add(vertical),
        width: area.width.saturating_sub(horizontal.saturating_mul(2)),
        height: area.height.saturating_sub(vertical.saturating_mul(2)),
    }
}
