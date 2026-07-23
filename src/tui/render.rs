use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{AppFocus, AppState};

use super::widgets::{diff, files, status};

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(format!(
            " {} {} #{} — {} — by {}",
            provider_name(state),
            state.provider.key.repository,
            state.provider.key.number,
            state.provider.title,
            state.provider.author
        )),
        rows[0],
    );

    if area.width >= 80 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(1)])
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
    frame.render_widget(
        Paragraph::new(" Tab focus  j/k move  ]f/[f file  ]u/[u unreviewed  m reviewed  R submit  ? help  q quit"),
        rows[3],
    );

    if state.help_visible {
        let overlay = inset(area, 5, 3);
        frame.render_widget(Clear, overlay);
        frame.render_widget(
            Paragraph::new(
                "Navigation\n\nTab / Shift-Tab  focus\nj / k            move\n]f / [f          file\n]u / [u          unreviewed\nm                reviewed\nv                selection\nc / s            comment / suggest\nt                threads\nR                submit\nr                refresh\nq                quit",
            )
            .block(Block::default().title(" Help ").borders(Borders::ALL)),
            overlay,
        );
    }
}

fn provider_name(state: &AppState) -> &'static str {
    match state.provider.key.provider {
        crate::domain::ProviderKind::GitHub => "GitHub",
        crate::domain::ProviderKind::GitLab => "GitLab",
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
