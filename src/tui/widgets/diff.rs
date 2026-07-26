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
    domain::{DiffSide, PatchAvailability},
    tui::{
        text::{display_width, truncate_to_width},
        theme, viewport,
    },
};

/// Width of the gutter carried by every row: a 5-wide line number column plus
/// its trailing space (`{number:>5} `), or that many blank columns for
/// comment/orphan rows so their `│` prefix lines up underneath it.
const GUTTER: &str = "      ";

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // Borders (2) plus one column of breathing room on each side.
    let inner_width = area.width.saturating_sub(4) as usize;
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
                .title(" [3] Diff ")
                .borders(Borders::ALL)
                .padding(ratatui::widgets::Padding::horizontal(1))
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
        DisplayRow::Diff { row } => diff_line(diff, *row, inner_width),
        DisplayRow::Comment {
            entry,
            kind,
            text,
            author,
        } => comment_line(state, entry, *kind, text, author.as_deref(), inner_width),
        DisplayRow::FileHeader { path } => Line::styled(
            path.clone(),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        DisplayRow::OrphanHeader => {
            Line::styled("— outdated comments —", Style::default().fg(theme::MUTED))
        }
        DisplayRow::Gap { hidden, .. } => Line::from(vec![
            Span::raw(GUTTER),
            Span::styled(
                if *hidden == 0 {
                    "· · · z loads the rest of the file · · ·".to_owned()
                } else {
                    format!("· · · {hidden} hidden lines · · · — z expand")
                },
                Style::default().fg(theme::MUTED),
            ),
        ]),
        DisplayRow::HunkHeader { hunk } => hunk_header_line(state, *hunk),
        DisplayRow::SplitDiff { left, right } => split_line(diff, *left, *right, inner_width),
        DisplayRow::Context { new_line, text } => Line::from(vec![
            Span::styled(format!("{new_line:>5} "), Style::default().fg(theme::MUTED)),
            Span::styled(text.clone(), Style::default().fg(theme::FG)),
        ]),
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

fn hunk_header_line(state: &AppState, hunk: u32) -> Line<'static> {
    let total = state.active_hunk_total();
    let reviewed = state
        .provider
        .files
        .get(state.active_file_index)
        .and_then(|file| state.session.files.get(&file.path))
        .is_some_and(|progress| progress.reviewed_hunks.contains(&hunk));
    let (marker, marker_color) = if reviewed {
        ("✓ reviewed", theme::SUCCESS)
    } else {
        ("M mark", theme::MUTED)
    };
    Line::from(vec![
        Span::raw(GUTTER),
        Span::styled(
            format!("hunk {}/{total}", hunk + 1),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(theme::MUTED)),
        Span::styled(marker.to_owned(), Style::default().fg(marker_color)),
    ])
}

fn diff_line(diff: &RenderedDiff, row: usize, inner_width: usize) -> Line<'static> {
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
    let mut line = Line::from(spans);
    if let Some(bg) = line.spans.iter().skip(1).find_map(|span| span.style.bg) {
        line.spans[0].style = line.spans[0].style.bg(bg);
        let text_width = line.width();
        if text_width < inner_width {
            line.spans.push(Span::styled(
                " ".repeat(inner_width - text_width),
                Style::default().bg(bg),
            ));
        }
    }
    line
}

fn split_line(
    diff: &RenderedDiff,
    left: Option<usize>,
    right: Option<usize>,
    inner_width: usize,
) -> Line<'static> {
    let column = inner_width.saturating_sub(3) / 2;
    let mut spans = side_spans(diff, left, DiffSide::Left, column);
    spans.push(Span::styled(" │ ", Style::default().fg(theme::BORDER)));
    spans.extend(side_spans(diff, right, DiffSide::Right, column));
    Line::from(spans)
}

fn side_spans(
    diff: &RenderedDiff,
    row: Option<usize>,
    side: DiffSide,
    width: usize,
) -> Vec<Span<'static>> {
    let Some(rendered) = row.and_then(|row| diff.rows.get(row)) else {
        return vec![Span::styled(
            "\u{2591}".repeat(width),
            Style::default().fg(theme::BORDER),
        )];
    };
    let position = match side {
        DiffSide::Left => rendered.binding.left.as_ref(),
        DiffSide::Right => rendered.binding.right.as_ref(),
    };
    let number = position
        .map(|position| position.line.to_string())
        .unwrap_or_default();
    let mut spans = vec![Span::styled(
        format!("{number:>5} "),
        Style::default().fg(theme::MUTED),
    )];
    spans.extend(truncate_spans(
        &rendered.text.spans,
        width.saturating_sub(6),
    ));
    let used: usize = spans.iter().map(|span| display_width(&span.content)).sum();
    let background = rendered
        .text
        .spans
        .iter()
        .find_map(|span| span.style.bg)
        .map(|bg| Style::default().bg(bg));
    if let Some(style) = background {
        spans[0].style = spans[0].style.patch(style);
    }
    if used < width {
        spans.push(Span::styled(
            " ".repeat(width - used),
            background.unwrap_or_default(),
        ));
    }
    spans
}

fn truncate_spans(spans: &[Span<'static>], width: usize) -> Vec<Span<'static>> {
    let mut taken = 0;
    let mut out = Vec::new();
    for span in spans {
        if taken >= width {
            break;
        }
        let length = display_width(&span.content);
        if taken + length <= width {
            out.push(span.clone());
            taken += length;
        } else {
            let text = truncate_to_width(&span.content, width - taken);
            out.push(Span::styled(text, span.style));
            taken = width;
        }
    }
    out
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
    let border_style = Style::default().fg(theme::COMMENT);
    let mut spans = vec![Span::raw(GUTTER)];
    match kind {
        CommentRowKind::Header => {
            // ╭─ @autor · marcador ─────╮
            let author_label = author.map_or_else(|| "you".to_owned(), str::to_owned);
            let (marker_text, marker_color) =
                marker(state, entry).unwrap_or(("comment", theme::MUTED));
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
            let used = 3 + 1 + display_width(&author_label) + 3 + display_width(marker_text) + 1;
            let dashes = card_width.saturating_sub(used + 1);
            spans.push(Span::styled("─".repeat(dashes), border_style));
            spans.push(Span::styled("╮", border_style));
        }
        CommentRowKind::Body => {
            // │   texto (padding interno) │
            spans.push(Span::styled("│   ", border_style));
            spans.push(Span::styled(
                text.to_owned(),
                Style::default().fg(theme::FG),
            ));
            let used = 4 + display_width(text);
            let pad = card_width.saturating_sub(used + 1);
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled("│", border_style));
        }
        CommentRowKind::Footer => {
            // ╰─ e editar · x excluir ──╯  (teclas em accent bold)
            let hints: &[(&str, &str)] = match entry {
                CommentEntry::Draft { .. } => &[("e", "edit"), ("x", "delete")],
                CommentEntry::Thread { .. } => &[("r", "reply")],
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
                used += display_width(key) + 1 + display_width(label);
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
        PatchAvailability::Available(_) => "Loading diff…".into(),
        PatchAvailability::Binary => "Binary file: inline review unavailable".into(),
        PatchAvailability::TooLarge => "Diff too large for inline review".into(),
        PatchAvailability::Collapsed => "Diff collapsed by the provider".into(),
        PatchAvailability::Truncated { reason } => format!("Diff unavailable: {reason}"),
    }
}
