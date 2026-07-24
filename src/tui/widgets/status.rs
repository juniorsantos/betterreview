use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

use crate::app::AppState;

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let reviewed = state
        .session
        .files
        .values()
        .filter(|progress| progress.reviewed)
        .count();
    const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    // Errors first, then in-flight operation feedback, then the idle summary.
    let (message, style) = if let Some(error) = &state.error_banner {
        (
            error.clone(),
            ratatui::style::Style::default().fg(crate::tui::theme::DANGER),
        )
    } else if let Some((_, label)) = state.pending_labels.iter().next_back() {
        (
            format!(" {} {label}", SPINNER[state.spinner_frame % SPINNER.len()]),
            ratatui::style::Style::default().fg(crate::tui::theme::ACCENT),
        )
    } else {
        (
            format!(
                " {reviewed}/{} reviewed  •  {} drafts  •  {} operations",
                state.provider.files.len(),
                state.provider.drafts.len(),
                state.busy_operations.len()
            ),
            ratatui::style::Style::default().fg(crate::tui::theme::MUTED),
        )
    };
    frame.render_widget(Paragraph::new(Line::raw(message)).style(style), area);
}
