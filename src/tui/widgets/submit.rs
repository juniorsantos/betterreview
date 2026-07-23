use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app::AppState,
    domain::{ReviewOutcome, Support},
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(modal) = &state.submission_modal else {
        return;
    };
    let popup = centered(area, 70, 14);
    let support = state.provider.capabilities.for_outcome(modal.outcome);
    let reason = match support {
        Support::Supported => String::new(),
        Support::Unsupported { reason } => format!("disabled: {reason}"),
    };
    let draft_label = if state.provider.drafts.len() == 1 {
        "1 draft will be published".to_owned()
    } else {
        format!("{} drafts will be published", state.provider.drafts.len())
    };
    let outcomes = vec![
        outcome("COMMENT", modal.outcome == ReviewOutcome::Comment),
        Span::raw("  "),
        outcome("APPROVE", modal.outcome == ReviewOutcome::Approve),
        Span::raw("  "),
        outcome(
            "REQUEST_CHANGES",
            modal.outcome == ReviewOutcome::RequestChanges,
        ),
    ];
    let lines = vec![
        Line::raw(draft_label),
        Line::raw(""),
        Line::raw("Summary"),
        Line::raw(modal.summary.clone()),
        Line::raw(""),
        Line::from(outcomes),
        Line::styled(reason, Style::default().fg(Color::Yellow)),
        Line::raw(""),
        Line::raw(action_label(modal.outcome)),
        Line::raw("Tab field  ↑/↓ outcome"),
        Line::raw("Enter submit  Esc cancel"),
    ];
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(" Submit review ")
                .borders(Borders::ALL),
        ),
        popup,
    );
}

fn outcome(label: &'static str, selected: bool) -> Span<'static> {
    if selected {
        Span::styled(
            format!("[{label}]"),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(label)
    }
}

fn action_label(outcome: ReviewOutcome) -> &'static str {
    match outcome {
        ReviewOutcome::Comment => "Comment review",
        ReviewOutcome::Approve => "Approve review",
        ReviewOutcome::RequestChanges => "Request changes",
    }
}

fn centered(area: Rect, maximum_width: u16, maximum_height: u16) -> Rect {
    let width = maximum_width.min(area.width.saturating_sub(2)).max(1);
    let height = maximum_height.min(area.height.saturating_sub(2)).max(1);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
