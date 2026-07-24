use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::{AppFocus, AppState, CommentEntry, CommentRowKind, DisplayRow},
    diff::{RenderedDiff, RenderedRow},
    domain::PatchAvailability,
    tui::{theme, viewport},
};

/// Width of the gutter carried by every row: a 5-wide line number column plus
/// its trailing space (`{number:>5} `), or that many blank columns for
/// comment/orphan rows so their `│` prefix lines up underneath it.
const GUTTER: &str = "      ";

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let lines = match &state.rendered_diff {
        Some(diff) => state
            .display_rows
            .iter()
            .enumerate()
            .map(|(index, display_row)| {
                render_display_row(state, diff, display_row, index, inner_width)
            })
            .collect(),
        None => vec![Line::raw(unavailable_reason(state))],
    };
    let border = if state.focus == AppFocus::Diff {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    let visible = area.height.saturating_sub(2) as usize;
    let start = viewport::start(state.display_cursor, lines.len(), visible);
    let scroll = u16::try_from(start).unwrap_or(u16::MAX);
    frame.render_widget(
        Paragraph::new(lines).scroll((scroll, 0)).block(
            Block::default()
                .title(" Diff ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
}

fn render_display_row(
    state: &AppState,
    diff: &RenderedDiff,
    display_row: &DisplayRow,
    index: usize,
    inner_width: usize,
) -> Line<'static> {
    let mut line = match display_row {
        DisplayRow::Diff { row } => diff_line(diff, *row),
        DisplayRow::Comment {
            entry,
            kind,
            text,
            author,
        } => comment_line(state, entry, *kind, text, author.as_deref(), inner_width),
        DisplayRow::OrphanHeader => Line::styled(
            "— comentários desatualizados —",
            Style::default().fg(theme::MUTED),
        ),
    };

    let selected = matches!(display_row, DisplayRow::Diff { row } if state.selection_anchor.is_some_and(|anchor| {
        let start = anchor.min(state.session.cursor_row);
        let end = anchor.max(state.session.cursor_row);
        (start..=end).contains(row)
    }));

    // The whole comment card lights up while the cursor sits on its block.
    let cursor_on_block = match display_row {
        DisplayRow::Comment { entry, .. } => matches!(
            state.display_rows.get(state.display_cursor),
            Some(DisplayRow::Comment { entry: current, .. }) if current == entry
        ),
        _ => false,
    };

    let style = if index == state.display_cursor || cursor_on_block {
        Some(
            Style::default()
                .bg(theme::CURSOR_LINE)
                .add_modifier(Modifier::BOLD),
        )
    } else if selected {
        Some(Style::default().bg(theme::SELECTION))
    } else {
        None
    };
    if let Some(style) = style {
        // Pad so the background reaches the panel's right edge.
        let text_width = line.width();
        if text_width < inner_width {
            line.spans
                .push(Span::raw(" ".repeat(inner_width - text_width)));
        }
        if matches!(display_row, DisplayRow::Comment { .. }) {
            // The card lives in the code body area: keep the line-number
            // gutter unpainted so the box visually starts after it.
            for span in line.spans.iter_mut().skip(1) {
                span.style = span.style.patch(style);
            }
        } else {
            line = line.style(style);
        }
    }
    line
}

fn diff_line(diff: &RenderedDiff, row: usize) -> Line<'static> {
    let Some(rendered_row): Option<&RenderedRow> = diff.rows.get(row) else {
        return Line::default();
    };
    // One gutter column: the line number on the side the row exists on (new
    // side wins when both are present).
    let number = rendered_row
        .binding
        .right
        .as_ref()
        .or(rendered_row.binding.left.as_ref())
        .map(|position| position.line.to_string())
        .unwrap_or_default();
    let mut spans = vec![Span::styled(
        format!("{number:>5} "),
        Style::default().fg(theme::MUTED),
    )];
    spans.extend(rendered_row.text.spans.clone());
    Line::from(spans)
}

fn comment_line(
    state: &AppState,
    entry: &CommentEntry,
    kind: CommentRowKind,
    text: &str,
    author: Option<&str>,
    inner_width: usize,
) -> Line<'static> {
    let card_width = inner_width.saturating_sub(GUTTER.len()).max(4);
    let border_style = Style::default().fg(theme::BORDER);
    let mut spans = vec![Span::raw(GUTTER)];
    match kind {
        CommentRowKind::Header => {
            // ╭─ @autor · marcador ─────╮
            let author_label = author.map_or_else(|| "você".to_owned(), str::to_owned);
            let (marker_text, marker_color) =
                marker(state, entry).unwrap_or(("comentário", theme::MUTED));
            spans.push(Span::styled("╭─ ", border_style));
            spans.push(Span::styled(
                format!("@{author_label}"),
                Style::default().fg(theme::ACCENT),
            ));
            spans.push(Span::styled(" · ", border_style));
            spans.push(Span::styled(
                marker_text.to_owned(),
                Style::default().fg(marker_color),
            ));
            spans.push(Span::raw(" "));
            let used = 3 + 1 + author_label.chars().count() + 3 + marker_text.chars().count() + 1;
            let dashes = card_width.saturating_sub(used + 1);
            spans.push(Span::styled("─".repeat(dashes), border_style));
            spans.push(Span::styled("╮", border_style));
        }
        CommentRowKind::Body => {
            // │ texto ──────────────────│
            spans.push(Span::styled("│ ", border_style));
            spans.push(Span::styled(
                text.to_owned(),
                Style::default().fg(theme::FG),
            ));
            let used = 2 + text.chars().count();
            let pad = card_width.saturating_sub(used + 1);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled("│", border_style));
        }
        CommentRowKind::Footer => {
            // ╰─ e editar · x excluir ──╯  (teclas em accent bold)
            let hints: &[(&str, &str)] = match entry {
                CommentEntry::Draft { .. } => &[("e", "editar"), ("x", "excluir")],
                CommentEntry::Thread { .. } => &[("r", "responder")],
            };
            spans.push(Span::styled("╰─ ", border_style));
            let mut used = 3;
            for (index, (key, label)) in hints.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::styled(" · ", border_style));
                    used += 3;
                }
                spans.push(Span::styled(
                    (*key).to_owned(),
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    (*label).to_owned(),
                    Style::default().fg(theme::MUTED),
                ));
                used += key.chars().count() + 1 + label.chars().count();
            }
            spans.push(Span::raw(" "));
            used += 1;
            let dashes = card_width.saturating_sub(used + 1);
            spans.push(Span::styled("─".repeat(dashes), border_style));
            spans.push(Span::styled("╯", border_style));
        }
    }
    Line::from(spans)
}

/// The badge shown next to a comment block's header: `draft` for local drafts
/// awaiting submission, `✓` for threads that have been resolved on the
/// provider. Unresolved threads carry no badge.
fn marker(state: &AppState, entry: &CommentEntry) -> Option<(&'static str, ratatui::style::Color)> {
    match entry {
        CommentEntry::Draft { .. } => Some(("draft", theme::WARNING)),
        CommentEntry::Thread { thread, .. } => state
            .provider
            .threads
            .iter()
            .find(|candidate| &candidate.id == thread)
            .filter(|thread| thread.resolved)
            .map(|_| ("✓", theme::SUCCESS)),
    }
}

fn unavailable_reason(state: &AppState) -> String {
    let Some(file) = state.provider.files.get(state.active_file_index) else {
        return "No changed files".into();
    };
    match &file.patch {
        PatchAvailability::Available(_) => "Loading diff...".into(),
        PatchAvailability::Binary => "Binary file: inline review unavailable".into(),
        PatchAvailability::TooLarge => "Patch is too large for inline review".into(),
        PatchAvailability::Collapsed => "Patch was collapsed by the provider".into(),
        PatchAvailability::Truncated { reason } => format!("Patch unavailable: {reason}"),
    }
}
