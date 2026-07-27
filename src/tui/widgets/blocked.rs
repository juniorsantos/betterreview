use ratatui::{Frame, layout::Rect, style::Style, text::Line};

use crate::{
    app::AppState,
    tui::{
        theme,
        widgets::dialog::{Dialog, Sizing, render_dialog},
    },
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(blocked) = state.blocked.as_ref() else {
        return;
    };
    let mut body = vec![
        Line::styled(blocked.guidance.clone(), Style::default().fg(theme::FG)),
        Line::raw(""),
    ];
    body.extend(
        blocked
            .reason
            .lines()
            .map(|line| Line::styled(line.to_owned(), Style::default().fg(theme::MUTED))),
    );
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::styled(
                format!(" {} ", blocked.title),
                Style::default().fg(theme::DANGER),
            ),
            body,
            hints: "⎋ dismiss",
            sizing: Sizing::Content { max_width: 70 },
            zones: Vec::new(),
        },
    );
}
