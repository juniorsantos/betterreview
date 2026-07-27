use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    app::AppState,
    domain::{ReviewOutcome, Support},
    tui::{
        text::{display_width, truncate_to_width},
        theme,
        widgets::dialog::{
            ActionButton, Dialog, Sizing, button_line, center_lines, clamped_width, render_dialog,
        },
    },
};

const DIALOG_WIDTH: u16 = 70;

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(modal) = &state.submission_modal else {
        return;
    };
    let draft_label = if state.provider.drafts.len() == 1 {
        "1 draft will be published".to_owned()
    } else {
        format!("{} drafts will be published", state.provider.drafts.len())
    };

    let mut body = vec![Line::raw(draft_label), Line::raw("")];
    let action_width = usize::from(
        clamped_width(DIALOG_WIDTH, area.width)
            .saturating_sub(3)
            .max(1),
    );
    body.extend(summary_lines(
        &modal.summary,
        action_width.saturating_sub(1).max(2),
    ));
    body.push(Line::raw(""));
    body.extend(shortcut_lines(state, modal.outcome, action_width));
    if let Support::Unsupported { reason } = state.provider.capabilities.for_outcome(modal.outcome)
    {
        body.push(Line::styled(
            format!("unavailable: {reason}"),
            Style::default().fg(theme::WARNING),
        ));
    }

    let height = u16::try_from(body.len() + 4).unwrap_or(u16::MAX);
    render_dialog(
        frame,
        area,
        Dialog {
            title: title_line(modal.outcome),
            body,
            hints: "⇥ verdict · ↵ submit · ⌥↵ new line · ⎋ cancel",
            sizing: Sizing::Fixed {
                width: DIALOG_WIDTH,
                height,
            },
            zones: Vec::new(),
        },
    );
}

fn title_line(outcome: ReviewOutcome) -> Line<'static> {
    Line::from(vec![
        Span::raw(" Submit review "),
        Span::styled("· ", Style::default().fg(theme::BORDER)),
        Span::styled(
            format!("{} ", label(outcome)),
            Style::default()
                .fg(theme::MUTED)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn summary_lines(summary: &str, width: usize) -> Vec<Line<'static>> {
    let border_style = Style::default().fg(theme::ACCENT);
    let label = " Comment ";
    let top_fill = width.saturating_sub(display_width(label) + 2);
    let mut lines = vec![Line::styled(
        format!("┌{label}{}┐", "─".repeat(top_fill)),
        border_style,
    )];
    let summary_lines: Vec<&str> = summary.split('\n').collect();
    let inner_width = width.saturating_sub(2);
    for (index, line) in summary_lines.iter().enumerate() {
        let is_last = index + 1 == summary_lines.len();
        let caret_width = usize::from(is_last);
        let text_width = inner_width.saturating_sub(1 + caret_width);
        let text = truncate_to_width(line, text_width);
        let padding = inner_width.saturating_sub(1 + display_width(&text) + caret_width);
        let mut spans = vec![
            Span::styled("│", border_style),
            Span::raw(" "),
            Span::styled(text, Style::default().fg(theme::FG)),
        ];
        if is_last {
            spans.push(Span::styled(
                "▌",
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::raw(" ".repeat(padding)));
        spans.push(Span::styled("│", border_style));
        lines.push(Line::from(spans));
    }
    lines.push(Line::styled(
        format!("└{}┘", "─".repeat(inner_width)),
        border_style,
    ));
    lines
}

fn shortcut_lines(
    state: &AppState,
    active: ReviewOutcome,
    available_width: usize,
) -> Vec<Line<'static>> {
    let outcomes = [
        ReviewOutcome::Approve,
        ReviewOutcome::RequestChanges,
        ReviewOutcome::Comment,
    ];
    let buttons: Vec<ActionButton<'_>> = outcomes
        .iter()
        .map(|outcome| ActionButton {
            label: label(*outcome),
            selected: *outcome == active,
            enabled: matches!(
                state.provider.capabilities.for_outcome(*outcome),
                Support::Supported
            ),
        })
        .collect();
    center_lines(vec![button_line(&buttons, 1)], available_width)
}

fn label(outcome: ReviewOutcome) -> &'static str {
    match outcome {
        ReviewOutcome::Comment => "COMMENT",
        ReviewOutcome::Approve => "APPROVE",
        ReviewOutcome::RequestChanges => "REQUEST CHANGES",
    }
}
