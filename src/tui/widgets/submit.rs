use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    app::AppState,
    domain::{ReviewOutcome, Support},
    tui::widgets::dialog::{Dialog, render_dialog},
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(modal) = &state.submission_modal else {
        return;
    };
    let support = state.provider.capabilities.for_outcome(modal.outcome);
    let reason = match support {
        Support::Supported => String::new(),
        Support::Unsupported { reason } => format!("indisponível: {reason}"),
    };
    let draft_label = if state.provider.drafts.len() == 1 {
        "1 draft será publicado".to_owned()
    } else {
        format!("{} drafts serão publicados", state.provider.drafts.len())
    };
    let outcomes = vec![
        outcome("COMENTAR", modal.outcome == ReviewOutcome::Comment),
        Span::raw("  "),
        outcome("APROVAR", modal.outcome == ReviewOutcome::Approve),
        Span::raw("  "),
        outcome(
            "PEDIR MUDANÇAS",
            modal.outcome == ReviewOutcome::RequestChanges,
        ),
    ];
    let body = vec![
        Line::raw(draft_label),
        Line::raw(""),
        Line::raw("Resumo"),
        Line::raw(modal.summary.clone()),
        Line::raw(""),
        Line::from(outcomes),
        Line::styled(reason, Style::default().fg(crate::tui::theme::WARNING)),
        Line::raw(""),
        Line::raw(action_label(modal.outcome)),
    ];
    render_dialog(
        frame,
        area,
        Dialog {
            title: " Enviar revisão ",
            body,
            hints: "Tab campo · ↑/↓ resultado · Enter enviar · Esc cancelar",
            width: 70,
            height: 14,
        },
    );
}

fn outcome(label: &'static str, selected: bool) -> Span<'static> {
    if selected {
        Span::styled(
            format!("[{label}]"),
            Style::default()
                .fg(crate::tui::theme::BG)
                .bg(crate::tui::theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(label)
    }
}

fn action_label(outcome: ReviewOutcome) -> &'static str {
    match outcome {
        ReviewOutcome::Comment => "Comentar na revisão",
        ReviewOutcome::Approve => "Aprovar a revisão",
        ReviewOutcome::RequestChanges => "Pedir mudanças",
    }
}
