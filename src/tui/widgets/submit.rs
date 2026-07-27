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
        theme,
        widgets::dialog::{
            ActionButton, Dialog, Sizing, center_lines, clamped_width, outlined_button_rows,
            render_dialog,
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
    body.extend(summary_lines(&modal.summary));
    body.push(Line::raw(""));
    let action_width = usize::from(
        clamped_width(DIALOG_WIDTH, area.width)
            .saturating_sub(3)
            .max(1),
    );
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

fn summary_lines(summary: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = summary
        .split('\n')
        .map(|line| Line::styled(line.to_owned(), Style::default().fg(theme::FG)))
        .collect();
    if let Some(last) = lines.last_mut() {
        last.spans.push(Span::styled(
            "▌",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines.insert(
        0,
        Line::styled("Summary", Style::default().fg(theme::MUTED)),
    );
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
    center_lines(outlined_button_rows(&buttons, 1, 0), available_width)
}

fn label(outcome: ReviewOutcome) -> &'static str {
    match outcome {
        ReviewOutcome::Comment => "COMMENT",
        ReviewOutcome::Approve => "APPROVE",
        ReviewOutcome::RequestChanges => "REQUEST CHANGES",
    }
}
