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
    domain::{DiffLayout, DiffSide, PatchAvailability},
    providers::ReviewLinks,
    tui::{
        hyperlink,
        text::{display_width, expand_tabs, truncate_to_width},
        theme, viewport,
        widgets::dialog::{ActionButton, button_line},
    },
};

const GUTTER_SPANS: usize = 3;
const SIDE_GUTTER_WIDTH: usize = 6;
const MIN_GUTTER_DIGITS: usize = 2;
const MAX_BLAME_WIDTH: usize = 22;

struct RowLayout<'a> {
    inner_width: usize,
    columns: Option<crate::tui::DiffColumns>,
    gutter: Gutter,
    commented: &'a std::collections::BTreeSet<usize>,
    moved: &'a std::collections::BTreeSet<usize>,
    blame: Blame<'a>,
}

#[derive(Clone, Copy)]
struct CardLayout {
    inner_width: usize,
    gutter: Gutter,
}

#[derive(Clone, Copy, Default)]
struct Blame<'a> {
    lines: Option<&'a std::collections::BTreeMap<u32, crate::blame::BlameLine>>,
    width: usize,
}

impl Blame<'_> {
    fn cell(self, old: Option<u32>) -> Option<Span<'static>> {
        let lines = self.lines?;
        let text = old
            .and_then(|line| lines.get(&line))
            .map(|entry| format!("{} {}", entry.author, entry.age))
            .unwrap_or_default();
        Some(Span::styled(
            format!(
                "{:<width$} ",
                truncate_to_width(&text, self.width),
                width = self.width
            ),
            Style::default().fg(theme::MUTED),
        ))
    }
}

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
        self.digits * 2 + 5
    }

    fn blank(self) -> Span<'static> {
        Span::raw(" ".repeat(self.width()))
    }

    fn bar(self, on_cursor: bool) -> Span<'static> {
        if on_cursor {
            Span::styled("\u{258c}", Style::default().fg(theme::ACCENT))
        } else {
            Span::raw(" ")
        }
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
    let border = if state.focus == AppFocus::Diff {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    let block = Block::default()
        .title(crate::tui::text::panel_title("[3] Diff", None, area.width))
        .borders(Borders::ALL)
        .padding(ratatui::widgets::Padding::horizontal(1))
        .border_style(Style::default().fg(border));
    let inner = block.inner(area);
    let inner_width = inner.width as usize;
    let columns = crate::tui::diff_columns(area, state);
    let gutter = Gutter::for_state(state);
    let layout = RowLayout {
        inner_width,
        columns,
        gutter,
        commented: &crate::app::commented_rows(state),
        moved: &state
            .parsed_diff
            .as_ref()
            .map(crate::diff::moved_rows)
            .unwrap_or_default(),
        blame: blame_for(state),
    };
    let lines = match &state.rendered_diff {
        Some(diff) => state
            .display_rows
            .iter()
            .enumerate()
            .map(|(index, display_row)| {
                let line = render_display_row(state, diff, display_row, index, &layout);
                let line = if state.wrap_lines {
                    line
                } else {
                    mark_if_cut(line, inner_width)
                };
                (line, matches!(display_row, DisplayRow::FileHeader { .. }))
            })
            .collect(),
        None => vec![(Line::raw(unavailable_reason(state)), false)],
    };
    frame.render_widget(block, area);

    let full = inner.height as usize;
    let rows: Vec<(Vec<Line<'static>>, bool)> = lines
        .into_iter()
        .map(|(line, linked)| {
            let lines = if state.wrap_lines {
                wrap_with_gutter(line, inner.width as usize, gutter.width())
            } else {
                vec![line]
            };
            (lines, linked)
        })
        .collect();
    let heights: Vec<usize> = rows.iter().map(|(lines, _)| lines.len()).collect();
    let pinned = viewport::start_wrapped(state.display_cursor, &heights, full) > 0;
    let visible = if pinned { full.saturating_sub(1) } else { full };
    let start = viewport::start_wrapped(state.display_cursor, &heights, visible);
    let scroll = u16::try_from(start).unwrap_or(u16::MAX);
    let file_url = state
        .provider
        .files
        .get(state.active_file_index)
        .and_then(|file| {
            ReviewLinks::new(&state.provider.key, &state.provider.web_url)
                .map(|links| links.file_url(&file.path))
        });

    let body = if pinned {
        let line = pinned_line(state);
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        if let (Some(url), Some(path)) =
            (&file_url, state.parsed_diff.as_ref().map(|diff| &diff.path))
        {
            hyperlink::apply(
                frame,
                Rect::new(
                    inner.x,
                    inner.y,
                    u16::try_from(display_width(&path.0))
                        .unwrap_or(u16::MAX)
                        .min(inner.width),
                    1,
                ),
                url,
            );
        }
        Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1)
    } else {
        inner
    };
    let mut paragraph_lines = Vec::new();
    let mut linked_lines = Vec::new();
    for (lines, linked) in rows {
        for line in lines {
            if linked {
                linked_lines.push((paragraph_lines.len(), line.width()));
            }
            paragraph_lines.push(line);
        }
    }
    let paragraph = Paragraph::new(paragraph_lines).scroll((scroll, 0));
    frame.render_widget(paragraph, body);
    if let Some(url) = file_url {
        let bottom = start.saturating_add(body.height as usize);
        for (line, width) in linked_lines {
            if !(start..bottom).contains(&line) {
                continue;
            }
            hyperlink::apply(
                frame,
                Rect::new(
                    body.x,
                    body.y + u16::try_from(line - start).unwrap_or(u16::MAX),
                    u16::try_from(width).unwrap_or(u16::MAX).min(body.width),
                    1,
                ),
                &url,
            );
        }
    }
}

fn lift(color: ratatui::style::Color) -> ratatui::style::Color {
    let ratatui::style::Color::Rgb(red, green, blue) = color else {
        return color;
    };
    let raise = |value: u8| {
        value
            .saturating_add(value / 2)
            .max(value.saturating_add(24))
    };
    ratatui::style::Color::Rgb(raise(red), raise(green), raise(blue))
}

fn blame_for(state: &AppState) -> Blame<'_> {
    if !state.blame_visible || crate::app::effective_layout(state) == DiffLayout::Split {
        return Blame::default();
    }
    let lines = state
        .provider
        .files
        .get(state.active_file_index)
        .and_then(|file| state.blame.get(&file.path));
    let width = lines.map_or(0, |lines| {
        lines
            .values()
            .map(|entry| display_width(&entry.author) + 1 + display_width(&entry.age))
            .max()
            .unwrap_or(0)
            .min(MAX_BLAME_WIDTH)
    });
    Blame { lines, width }
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
    layout: &RowLayout<'_>,
) -> Line<'static> {
    let &RowLayout {
        inner_width,
        columns,
        gutter,
        commented,
        moved,
        blame,
    } = layout;
    let cursor_on_block = match display_row {
        DisplayRow::Comment { entry, .. } => matches!(
            state.display_rows.get(state.display_cursor),
            Some(DisplayRow::Comment { entry: current, .. }) if current == entry
        ),
        _ => false,
    };
    let mut line = match display_row {
        DisplayRow::Diff { row } => diff_line(
            diff,
            *row,
            inner_width,
            RowStyle {
                gutter,
                tab_width: state.tab_width,
                on_cursor: index == state.display_cursor || commented.contains(row),
                moved: moved.contains(row),
                blame,
            },
        ),
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
            CardLayout {
                inner_width,
                gutter,
            },
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
                if !matches!(
                    span.style.bg,
                    Some(theme::ACCENT | theme::ACCENT_SOFT | theme::FILLER)
                ) {
                    span.style = span.style.patch(style);
                }
            }
        } else {
            for span in line.spans.iter_mut() {
                if let Some(background) = span.style.bg {
                    span.style = span.style.bg(lift(background));
                }
            }
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

struct RowStyle<'a> {
    gutter: Gutter,
    tab_width: usize,
    on_cursor: bool,
    moved: bool,
    blame: Blame<'a>,
}

fn diff_line(
    diff: &RenderedDiff,
    row: usize,
    inner_width: usize,
    style: RowStyle<'_>,
) -> Line<'static> {
    let RowStyle {
        gutter,
        tab_width,
        on_cursor,
        moved,
        blame,
    } = style;
    let Some(rendered_row): Option<&RenderedRow> = diff.rows.get(row) else {
        return Line::default();
    };
    let line_of = |position: &Option<crate::domain::DiffPosition>| {
        position.as_ref().map(|position| position.line)
    };
    let mut spans = vec![
        gutter.bar(on_cursor),
        Span::styled(
            gutter.cells(
                line_of(&rendered_row.binding.left),
                line_of(&rendered_row.binding.right),
            ),
            Style::default().fg(theme::MUTED),
        ),
        Span::styled("\u{2502} ", Style::default().fg(theme::BORDER)),
    ];
    if let Some(cell) = blame.cell(line_of(&rendered_row.binding.left)) {
        spans.insert(GUTTER_SPANS - 1, cell);
    }
    spans.extend(expand_span_tabs(&rendered_row.text.spans, tab_width));
    let mut line = Line::from(spans);
    let detected = line
        .spans
        .iter()
        .skip(GUTTER_SPANS)
        .find_map(|span| span.style.bg);
    if moved && detected.is_some() {
        for span in line.spans.iter_mut().skip(GUTTER_SPANS) {
            span.style = span.style.bg(theme::MOVED);
        }
    }
    if let Some(bg) = if moved { Some(theme::MOVED) } else { detected } {
        for span in line.spans.iter_mut().take(GUTTER_SPANS).skip(1) {
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
    layout: CardLayout,
) -> Line<'static> {
    let CardLayout {
        inner_width,
        gutter,
    } = layout;
    let card_width = inner_width.saturating_sub(gutter.width()).max(4);
    let is_reply = matches!(
        entry,
        CommentEntry::Thread { comment_index, .. } if *comment_index > 0
    );
    let accent = if is_reply {
        theme::ACCENT_SOFT
    } else {
        theme::ACCENT
    };
    let border_style = Style::default().fg(accent);
    let mut spans = vec![
        Span::styled("\u{258c}", Style::default().fg(accent)),
        Span::raw(" ".repeat(gutter.width().saturating_sub(1))),
    ];
    let card_start = spans.len();
    match kind {
        CommentRowKind::Spacer => {}
        CommentRowKind::Header => {
            let author_label = author.map_or_else(|| "you".to_owned(), str::to_owned);
            let (marker_text, marker_color) =
                marker(state, entry).unwrap_or(("comment", theme::MUTED));
            spans.push(Span::styled("┌─ ", border_style));
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
            spans.push(Span::styled("┐", border_style));
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
            let actions = comment_actions(entry, accent);
            let mut footer = Line::from(Span::styled("├", border_style));
            for span in &actions.spans {
                footer
                    .spans
                    .push(Span::styled("─".repeat(span.width()), border_style));
                footer.spans.push(Span::styled("┬", border_style));
            }
            let used = footer.width();
            spans.extend(footer.spans);
            spans.push(Span::styled(
                "─".repeat(card_width.saturating_sub(used + 1)),
                border_style,
            ));
            spans.push(Span::styled("┘", border_style));
        }
        CommentRowKind::Actions => {
            let actions = comment_actions(entry, accent);
            for (index, span) in actions.spans.into_iter().enumerate() {
                if index % 2 == 0 {
                    spans.push(Span::styled("│", border_style));
                    spans.push(span);
                    spans.push(Span::styled("│", border_style));
                } else {
                    spans.push(span);
                }
            }
        }
        CommentRowKind::ActionsBottom => {
            let actions = comment_actions(entry, accent);
            for (index, span) in actions.spans.into_iter().enumerate() {
                if index % 2 == 0 {
                    spans.push(Span::styled("└", border_style));
                    spans.push(Span::styled("─".repeat(span.width()), border_style));
                    spans.push(Span::styled("┘", border_style));
                } else {
                    spans.push(span);
                }
            }
        }
    }
    if matches!(
        kind,
        CommentRowKind::Header | CommentRowKind::Body | CommentRowKind::Footer
    ) {
        for span in &mut spans[card_start..] {
            if span.style.bg.is_none() {
                span.style = span.style.bg(theme::BG);
            }
        }
    }
    Line::from(spans)
}

fn comment_actions(entry: &CommentEntry, accent: ratatui::style::Color) -> Line<'static> {
    let hints: &[(&str, &str)] = match entry {
        CommentEntry::Draft { .. } => &[("e", "edit"), ("x", "delete")],
        CommentEntry::Thread { .. } => &[("r", "reply")],
    };
    let labels: Vec<String> = hints
        .iter()
        .map(|(key, label)| format!("{key} {label}"))
        .collect();
    let buttons: Vec<ActionButton<'_>> = labels
        .iter()
        .map(|label| ActionButton {
            label,
            selected: false,
            enabled: true,
        })
        .collect();
    let mut actions = button_line(&buttons, 1);
    for (index, span) in actions.spans.iter_mut().enumerate() {
        if index % 2 == 0 {
            span.style = Style::default().fg(accent);
        }
    }
    actions
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
