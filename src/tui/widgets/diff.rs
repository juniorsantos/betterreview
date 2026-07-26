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
        text::{display_width, expand_tabs, truncate_to_width},
        theme, viewport,
    },
};

const GUTTER_SPANS: usize = 2;
const SIDE_GUTTER_WIDTH: usize = 6;
const MIN_GUTTER_DIGITS: usize = 2;

#[derive(Clone, Copy)]
struct Gutter {
    digits: usize,
}

impl Gutter {
    fn for_state(state: &AppState) -> Self {
        let highest_row = state
            .rendered_diff
            .iter()
            .flat_map(|diff| diff.rows.iter())
            .flat_map(|row| [row.binding.left.as_ref(), row.binding.right.as_ref()])
            .flatten()
            .map(|position| position.line)
            .max()
            .unwrap_or(0);
        let highest_cached = state
            .provider
            .files
            .get(state.active_file_index)
            .and_then(|file| state.file_contexts.get(&file.path))
            .map_or(0, |lines| lines.len() as u32);
        Self {
            digits: highest_row
                .max(highest_cached)
                .to_string()
                .len()
                .max(MIN_GUTTER_DIGITS),
        }
    }

    fn width(self) -> usize {
        self.digits * 2 + 4
    }

    fn blank(self) -> Span<'static> {
        Span::raw(" ".repeat(self.width()))
    }

    fn cells(self, old: Option<u32>, new: Option<u32>) -> String {
        let text = |value: Option<u32>| value.map_or_else(String::new, |line| line.to_string());
        format!(
            "{:>digits$} {:>digits$} ",
            text(old),
            text(new),
            digits = self.digits
        )
    }
}

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let inner_width = area.width.saturating_sub(4) as usize;
    let columns = crate::tui::diff_columns(area, state);
    let gutter = Gutter::for_state(state);
    let lines = match &state.rendered_diff {
        Some(diff) => state
            .display_rows
            .iter()
            .enumerate()
            .map(|(index, display_row)| {
                let line = render_display_row(
                    state,
                    diff,
                    display_row,
                    index,
                    inner_width,
                    columns,
                    gutter,
                );
                if state.wrap_lines {
                    line
                } else {
                    mark_if_cut(line, inner_width)
                }
            })
            .collect(),
        None => vec![Line::raw(unavailable_reason(state))],
    };
    let border = if state.focus == AppFocus::Diff {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    let block = Block::default()
        .title(" [3] Diff ")
        .borders(Borders::ALL)
        .padding(ratatui::widgets::Padding::horizontal(1))
        .border_style(Style::default().fg(border));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let full = inner.height as usize;
    let rows: Vec<Vec<Line<'static>>> = lines
        .into_iter()
        .map(|line| {
            if state.wrap_lines {
                wrap_with_gutter(line, inner.width as usize, gutter.width())
            } else {
                vec![line]
            }
        })
        .collect();
    let heights: Vec<usize> = rows.iter().map(Vec::len).collect();
    let pinned = viewport::start_wrapped(state.display_cursor, &heights, full) > 0;
    let visible = if pinned { full.saturating_sub(1) } else { full };
    let start = viewport::start_wrapped(state.display_cursor, &heights, visible);
    let scroll = u16::try_from(start).unwrap_or(u16::MAX);

    let body = if pinned {
        frame.render_widget(
            Paragraph::new(pinned_line(state)),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1)
    } else {
        inner
    };
    let paragraph =
        Paragraph::new(rows.into_iter().flatten().collect::<Vec<_>>()).scroll((scroll, 0));
    frame.render_widget(paragraph, body);
}

fn expand_span_tabs(spans: &[Span<'static>], tab_width: usize) -> Vec<Span<'static>> {
    let mut column = 0;
    spans
        .iter()
        .map(|span| {
            let expanded = expand_tabs(&span.content, tab_width, column);
            column += display_width(&expanded);
            if expanded == span.content {
                span.clone()
            } else {
                Span::styled(expanded, span.style)
            }
        })
        .collect()
}

fn wrap_with_gutter(line: Line<'static>, width: usize, gutter_width: usize) -> Vec<Line<'static>> {
    if width == 0 || line.width() <= width {
        return vec![line];
    }
    let style = line.style;
    let continuation = width.saturating_sub(gutter_width).max(1);
    let mut remaining = line.spans;
    let mut rows = Vec::new();
    let mut room = width;
    let mut indent = 0;
    loop {
        let (head, tail) = split_spans(&remaining, room);
        if head.is_empty() {
            rows.push(Line::from(remaining).style(style));
            break;
        }
        let mut spans = Vec::with_capacity(head.len() + 1);
        if indent > 0 {
            spans.push(Span::raw(" ".repeat(indent)));
        }
        spans.extend(head);
        rows.push(Line::from(spans).style(style));
        if tail.is_empty() {
            break;
        }
        remaining = tail;
        room = continuation;
        indent = gutter_width;
    }
    rows
}

fn split_spans(spans: &[Span<'static>], width: usize) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut used = 0;
    for span in spans {
        if used >= width {
            tail.push(span.clone());
            continue;
        }
        let span_width = display_width(&span.content);
        if used + span_width <= width {
            used += span_width;
            head.push(span.clone());
            continue;
        }
        let taken = truncate_to_width(&span.content, width - used);
        let rest = span.content[taken.len()..].to_owned();
        if !taken.is_empty() {
            head.push(Span::styled(taken, span.style));
        }
        if !rest.is_empty() {
            tail.push(Span::styled(rest, span.style));
        }
        used = width;
    }
    (head, tail)
}

fn pinned_line(state: &AppState) -> Line<'static> {
    let path = state
        .parsed_diff
        .as_ref()
        .map(|parsed| parsed.path.0.clone())
        .unwrap_or_default();
    let mut spans = vec![Span::styled(
        path,
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    )];
    if let Some(hunk) = hunk_at_cursor(state) {
        spans.push(Span::styled(" · ", Style::default().fg(theme::BORDER)));
        spans.push(Span::styled(
            format!("hunk {}/{}", hunk + 1, state.active_hunk_total()),
            Style::default().fg(theme::MUTED),
        ));
        if let Some(section) = section_of(state, hunk) {
            spans.push(Span::styled(" · ", Style::default().fg(theme::BORDER)));
            spans.push(Span::styled(section, Style::default().fg(theme::MUTED)));
        }
    }
    Line::from(spans)
}

fn file_header_line(path: &str, previous_path: Option<&str>) -> Line<'static> {
    let strong = Style::default()
        .fg(theme::ACCENT)
        .add_modifier(Modifier::BOLD);
    let Some(previous) = previous_path else {
        return Line::styled(path.to_owned(), strong);
    };
    let shared = shared_prefix(previous, path);
    let dim = Style::default().fg(theme::MUTED);
    let mut spans = Vec::new();
    if shared > 0 {
        spans.push(Span::styled(previous[..shared].to_owned(), dim));
    }
    spans.push(Span::styled(previous[shared..].to_owned(), dim));
    spans.push(Span::styled(" \u{2192} ", dim));
    if shared > 0 {
        spans.push(Span::styled(path[..shared].to_owned(), dim));
    }
    spans.push(Span::styled(path[shared..].to_owned(), strong));
    Line::from(spans)
}

/// Bytes the two paths share, cut back to the last `/` so a partially
/// matching segment is emphasised whole rather than split mid-name.
fn shared_prefix(previous: &str, path: &str) -> usize {
    let common = previous
        .bytes()
        .zip(path.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    previous[..common].rfind('/').map_or(0, |slash| slash + 1)
}

fn section_of(state: &AppState, hunk: u32) -> Option<String> {
    state
        .parsed_diff
        .as_ref()?
        .hunks
        .iter()
        .find(|candidate| candidate.id == hunk)?
        .section
        .clone()
}

fn hunk_at_cursor(state: &AppState) -> Option<u32> {
    let row = state
        .display_rows
        .get(state.display_cursor)
        .and_then(DisplayRow::anchor_row)?;
    state
        .parsed_diff
        .as_ref()?
        .hunks
        .iter()
        .find(|hunk| hunk.row_range.contains(&row))
        .map(|hunk| hunk.id)
}

fn mark_if_cut(line: Line<'static>, width: usize) -> Line<'static> {
    if width == 0 || line.width() <= width {
        return line;
    }
    let mut kept = Vec::new();
    let mut used = 0;
    for span in line.spans {
        let span_width = display_width(&span.content);
        if used + span_width <= width.saturating_sub(1) {
            used += span_width;
            kept.push(span);
            continue;
        }
        let room = width.saturating_sub(1) - used;
        if room > 0 {
            kept.push(Span::styled(
                truncate_to_width(&span.content, room),
                span.style,
            ));
        }
        break;
    }
    kept.push(Span::styled("…", Style::default().fg(theme::MUTED)));
    Line::from(kept).style(line.style)
}

fn render_display_row(
    state: &AppState,
    diff: &RenderedDiff,
    display_row: &DisplayRow,
    index: usize,
    inner_width: usize,
    columns: Option<crate::tui::DiffColumns>,
    gutter: Gutter,
) -> Line<'static> {
    let mut line = match display_row {
        DisplayRow::Diff { row } => diff_line(diff, *row, inner_width, gutter, state.tab_width),
        DisplayRow::Comment {
            entry,
            kind,
            text,
            author,
        } => comment_line(
            state,
            entry,
            *kind,
            text,
            author.as_deref(),
            inner_width,
            gutter,
        ),
        DisplayRow::FileHeader {
            path,
            previous_path,
        } => file_header_line(path, previous_path.as_deref()),
        DisplayRow::OrphanHeader => {
            Line::styled("— outdated comments —", Style::default().fg(theme::MUTED))
        }
        DisplayRow::Gap { hidden, .. } => Line::from(vec![
            gutter.blank(),
            Span::styled(
                if *hidden == 0 {
                    "· · · z loads the rest of the file · · ·".to_owned()
                } else {
                    format!("· · · {hidden} hidden lines · · · — z expand")
                },
                Style::default().fg(theme::MUTED),
            ),
        ]),
        DisplayRow::HunkHeader { hunk } => hunk_header_line(state, *hunk, gutter),
        DisplayRow::SplitDiff { left, right } => {
            split_line(diff, *left, *right, columns, state.tab_width)
        }
        DisplayRow::Context {
            old_line,
            new_line,
            text,
        } => Line::from(vec![
            Span::styled(
                gutter.cells(*old_line, Some(*new_line)),
                Style::default().fg(theme::MUTED),
            ),
            Span::styled("\u{2502}  ", Style::default().fg(theme::BORDER)),
            Span::styled(
                expand_tabs(text, state.tab_width, 0),
                Style::default().fg(theme::FG),
            ),
        ]),
    };

    let selected = matches!(display_row, DisplayRow::Diff { row } if state.selection_anchor.is_some_and(|anchor| {
        let start = anchor.min(state.session.cursor_row);
        let end = anchor.max(state.session.cursor_row);
        (start..=end).contains(row)
    }));

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
        let text_width = line.width();
        if text_width < inner_width {
            line.spans
                .push(Span::raw(" ".repeat(inner_width - text_width)));
        }
        if matches!(display_row, DisplayRow::Comment { .. }) {
            for span in line.spans.iter_mut().skip(1) {
                span.style = span.style.patch(style);
            }
        } else {
            line = line.style(style);
        }
    }
    line
}

fn hunk_header_line(state: &AppState, hunk: u32, gutter: Gutter) -> Line<'static> {
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
    let mut spans = vec![
        gutter.blank(),
        Span::styled(
            format!("hunk {}/{total}", hunk + 1),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(section) = section_of(state, hunk) {
        spans.push(Span::styled(" · ", Style::default().fg(theme::MUTED)));
        spans.push(Span::styled(section, Style::default().fg(theme::FG)));
    }
    spans.push(Span::styled(" · ", Style::default().fg(theme::MUTED)));
    spans.push(Span::styled(
        marker.to_owned(),
        Style::default().fg(marker_color),
    ));
    Line::from(spans)
}

fn diff_line(
    diff: &RenderedDiff,
    row: usize,
    inner_width: usize,
    gutter: Gutter,
    tab_width: usize,
) -> Line<'static> {
    let Some(rendered_row): Option<&RenderedRow> = diff.rows.get(row) else {
        return Line::default();
    };
    let line_of = |position: &Option<crate::domain::DiffPosition>| {
        position.as_ref().map(|position| position.line)
    };
    let mut spans = vec![
        Span::styled(
            gutter.cells(
                line_of(&rendered_row.binding.left),
                line_of(&rendered_row.binding.right),
            ),
            Style::default().fg(theme::MUTED),
        ),
        Span::styled("\u{2502} ", Style::default().fg(theme::BORDER)),
    ];
    spans.extend(expand_span_tabs(&rendered_row.text.spans, tab_width));
    let mut line = Line::from(spans);
    if let Some(bg) = line
        .spans
        .iter()
        .skip(GUTTER_SPANS)
        .find_map(|span| span.style.bg)
    {
        for span in line.spans.iter_mut().take(GUTTER_SPANS) {
            span.style = span.style.bg(bg);
        }
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
    columns: Option<crate::tui::DiffColumns>,
    tab_width: usize,
) -> Line<'static> {
    let Some(columns) = columns else {
        return Line::default();
    };
    let mut spans = Vec::new();
    if columns.left.width > 0 {
        spans.extend(side_spans(
            diff,
            left,
            DiffSide::Left,
            columns.left.width as usize,
            tab_width,
        ));
    }
    if columns.left.width > 0 && columns.right.width > 0 {
        spans.push(Span::styled(" │ ", Style::default().fg(theme::BORDER)));
    }
    if columns.right.width > 0 {
        spans.extend(side_spans(
            diff,
            right,
            DiffSide::Right,
            columns.right.width as usize,
            tab_width,
        ));
    }
    Line::from(spans)
}

fn side_spans(
    diff: &RenderedDiff,
    row: Option<usize>,
    side: DiffSide,
    width: usize,
    tab_width: usize,
) -> Vec<Span<'static>> {
    let Some(rendered) = row.and_then(|row| diff.rows.get(row)) else {
        return vec![Span::styled(
            "\u{2571}".repeat(width),
            Style::default().fg(theme::FILLER),
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
        &expand_span_tabs(&rendered.text.spans, tab_width),
        width.saturating_sub(SIDE_GUTTER_WIDTH),
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
    gutter: Gutter,
) -> Line<'static> {
    let card_width = inner_width.saturating_sub(gutter.width()).max(4);
    let border_style = Style::default().fg(theme::COMMENT);
    let mut spans = vec![gutter.blank()];
    match kind {
        CommentRowKind::Header => {
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
