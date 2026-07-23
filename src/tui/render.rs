use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{AppFocus, AppState};

use super::{
    theme,
    widgets::{diff, editor, files, quit, status, submit, threads},
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
    frame.render_widget(
        Paragraph::new(
            " Tab/h/l focus  j/k move  ]f/[f file  m reviewed  e expand  R submit  ? help  q quit",
        )
        .style(Style::default().fg(theme::MUTED)),
        rows[3],
    );

    if state.help_visible {
        let text = "Navigation          Files                Review\n\
                    j/k       move      e    expand panel   v      selection\n\
                    Tab/h/l   focus     z    fold folder    c      comment\n\
                    ]f / [f   file      m    reviewed       s      suggestion\n\
                    ]u / [u   unreviewed                    t      threads\n\
                    \n\
                    Editor: Enter save   Alt+Enter newline   Esc close\n\
                    R submit review      r refresh           q quit";
        // Comfortable, but never the full screen.
        let width = 66.min(area.width * 4 / 5);
        let height = 12.min(area.height * 4 / 5);
        let overlay = ratatui::layout::Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        };
        frame.render_widget(Clear, overlay);
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().bg(theme::BG).fg(theme::FG))
                .block(
                    Block::default()
                        .title(" Help — Esc close ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::ACCENT)),
                ),
            overlay,
        );
    }
    threads::render(frame, area, state);
    editor::render(frame, area, state);
    submit::render(frame, area, state);
    if state.quit_dialog {
        quit::render(frame, area, state);
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
