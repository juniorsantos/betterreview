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
        widgets::dialog::{Dialog, Sizing, clamped_width, render_dialog},
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
    body.push(shortcut_line(state, modal.outcome, action_width));
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

fn shortcut_line(state: &AppState, active: ReviewOutcome, available_width: usize) -> Line<'static> {
    let spacious = available_width >= 49;
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
            spans.push(Span::raw(if spacious { "  " } else { " " }));
        }
        let supported = matches!(
            state.provider.capabilities.for_outcome(outcome),
            Support::Supported
        );
        let text = if outcome == active {
            if spacious {
                format!(" [  {}  ] ", label(outcome))
            } else {
                format!("[{}]", label(outcome))
            }
        } else if spacious {
            format!("  {}  ", label(outcome))
        } else {
            format!(" {} ", label(outcome))
        };
        let mut style = if supported {
            Style::default().fg(theme::BG).bg(color(outcome))
        } else {
            Style::default().fg(theme::MUTED).bg(theme::FILLER)
        };
        if outcome == active {
            style = style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(text, style));
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
        ReviewOutcome::Comment => theme::COMMENT,
        ReviewOutcome::Approve => theme::SUCCESS,
        ReviewOutcome::RequestChanges => theme::WARNING,
    }
}
