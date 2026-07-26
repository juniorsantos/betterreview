use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::AppState,
    tui::{text::display_width, theme},
};

/// Right-side hints for the review screen's flat status bar (transversal
/// rule 1): key/label pairs, key ACCENT+BOLD, label MUTED. `j/k` has no
/// label — its meaning ("move") is assumed obvious from every other screen.
const REVIEW_HINTS: [(&str, &str); 11] = [
    ("j/k", ""),
    ("]h", "hunk"),
    ("]c", "comment"),
    ("/", "search"),
    ("R", "submit"),
    ("?", "help"),
    ("f", "files"),
    ("\\", "layout"),
    ("|", "side"),
    ("Q", "list"),
    ("q", "quit"),
];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let reviewed = state
        .session
        .files
        .values()
        .filter(|progress| progress.reviewed)
        .count();
    const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    // Errors first, then a live notice (refusal feedback etc.), then
    // in-flight operation feedback, then the idle summary.
    let (message, style) = if let Some(error) = &state.error_banner {
        (
            error.clone(),
            ratatui::style::Style::default().fg(crate::tui::theme::DANGER),
        )
    } else if state.notice_ttl > 0 && state.notices.last().is_some() {
        (
            state.notices.last().cloned().unwrap_or_default(),
            ratatui::style::Style::default().fg(crate::tui::theme::WARNING),
        )
    } else if let Some(query) = &state.search_input {
        (
            format!("/{query}▌"),
            ratatui::style::Style::default().fg(crate::tui::theme::FG),
        )
    } else if let Some((_, label)) = state.pending_labels.iter().next_back() {
        (
            format!(" {} {label}", SPINNER[state.spinner_frame % SPINNER.len()]),
            ratatui::style::Style::default().fg(crate::tui::theme::ACCENT),
        )
    } else if let Some(query) = &state.search_query {
        let matches = crate::app::search_matches(state);
        let total = matches.len();
        let current = matches
            .iter()
            .position(|&index| index == state.display_cursor)
            .map_or(0, |position| position + 1);
        (
            format!("\u{201c}{query}\u{201d} {current}/{total}  n/N navigate  Esc clear"),
            ratatui::style::Style::default().fg(crate::tui::theme::MUTED),
        )
    } else {
        (
            format!(
                " {reviewed}/{} reviewed · {} drafts · {} operations",
                state.provider.files.len(),
                state.provider.drafts.len(),
                state.busy_operations.len()
            ),
            ratatui::style::Style::default().fg(crate::tui::theme::MUTED),
        )
    };
    let left = Line::styled(message, style);
    let line = flat_line(left, &REVIEW_HINTS, area.width);
    frame.render_widget(Paragraph::new(line), area);
}

/// Renders `left` followed by right-aligned `hints` (key ACCENT+BOLD, label
/// MUTED, pairs separated by ` · `) within `width` columns. The left side
/// always keeps its full text: hints are truncated — dropping trailing
/// pairs and appending `…` — or dropped entirely when there isn't room.
pub(in crate::tui) fn flat_line(
    left: Line<'static>,
    hints: &[(&str, &str)],
    width: u16,
) -> Line<'static> {
    let width = width as usize;
    let left_width = left.width();
    if left_width + 1 >= width {
        return left;
    }
    let budget = width - left_width - 1;
    let (hint_spans, hint_width) = fit_hints(hints, budget);
    if hint_width == 0 {
        return left;
    }
    let gap = width - left_width - hint_width;
    let mut spans = left.spans;
    let style = left.style;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(hint_spans);
    Line::from(spans).style(style)
}

/// Builds the widest prefix of `pairs` (dropping from the tail) that fits
/// within `budget` columns, appending a `…` marker when anything had to be
/// dropped. Returns an empty result when even the marker alone doesn't fit.
fn fit_hints(pairs: &[(&str, &str)], budget: usize) -> (Vec<Span<'static>>, usize) {
    if budget == 0 || pairs.is_empty() {
        return (Vec::new(), 0);
    }
    let full = hint_spans(pairs);
    let full_width = spans_width(&full);
    if full_width <= budget {
        return (full, full_width);
    }
    for take in (0..pairs.len()).rev() {
        let subset = &pairs[..take];
        let mut spans = hint_spans(subset);
        let mut width = spans_width(&spans);
        let marker = if subset.is_empty() { "…" } else { " …" };
        let marker_width = display_width(marker);
        if width + marker_width <= budget {
            spans.push(Span::styled(marker, Style::default().fg(theme::MUTED)));
            width += marker_width;
            return (spans, width);
        }
    }
    (Vec::new(), 0)
}

fn spans_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|span| display_width(&span.content)).sum()
}

/// Styles `pairs` as `key label · key label · …`, key in ACCENT+BOLD, label
/// in MUTED. Pairs with an empty label render the key alone.
fn hint_spans(pairs: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (index, (key, label)) in pairs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme::MUTED)));
        }
        spans.push(Span::styled(
            key.to_string(),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        if !label.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                label.to_string(),
                Style::default().fg(theme::MUTED),
            ));
        }
    }
    spans
}
