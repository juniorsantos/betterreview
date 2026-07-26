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
        widgets::dialog::{Dialog, Sizing, render_dialog},
    },
};

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
    body.push(shortcut_line(state, modal.outcome));
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
            hints: "Tab verdict · Enter submit · ⌥Enter new line · Esc cancel",
            sizing: Sizing::Fixed { width: 70, height },
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
                .fg(color(outcome))
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

fn shortcut_line(state: &AppState, active: ReviewOutcome) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, outcome) in [
        ReviewOutcome::Approve,
        ReviewOutcome::RequestChanges,
        ReviewOutcome::Comment,
    ]
    .into_iter()
    .enumerate()
    {
        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme::BORDER)));
        }
        let supported = matches!(
            state.provider.capabilities.for_outcome(outcome),
            Support::Supported
        );
        spans.push(if outcome == active {
            Span::styled(
                format!("[{}]", label(outcome)),
                Style::default()
                    .fg(theme::BG)
                    .bg(color(outcome))
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                label(outcome).to_lowercase(),
                Style::default().fg(if supported {
                    theme::MUTED
                } else {
                    theme::BORDER
                }),
            )
        });
    }
    Line::from(spans)
}

fn label(outcome: ReviewOutcome) -> &'static str {
    match outcome {
        ReviewOutcome::Comment => "COMMENT",
        ReviewOutcome::Approve => "APPROVE",
        ReviewOutcome::RequestChanges => "REQUEST CHANGES",
    }
}

fn color(outcome: ReviewOutcome) -> ratatui::style::Color {
    match outcome {
        ReviewOutcome::Comment => theme::ACCENT,
        ReviewOutcome::Approve => theme::SUCCESS,
        ReviewOutcome::RequestChanges => theme::WARNING,
    }
}
