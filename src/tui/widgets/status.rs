use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

use crate::app::AppState;

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let reviewed = state
        .session
        .files
        .values()
        .filter(|progress| progress.reviewed)
        .count();
    let message = state.error_banner.clone().unwrap_or_else(|| {
        format!(
            " {reviewed}/{} reviewed  •  {} drafts  •  {} operations",
            state.provider.files.len(),
            state.provider.drafts.len(),
            state.busy_operations.len()
        )
    });
    let style = if state.error_banner.is_some() {
        ratatui::style::Style::default().fg(crate::tui::theme::DANGER)
    } else {
        ratatui::style::Style::default().fg(crate::tui::theme::MUTED)
    };
    frame.render_widget(Paragraph::new(Line::raw(message)).style(style), area);
}
