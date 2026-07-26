use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    app::AppState,
    tui::{
        text::display_width,
        theme,
        widgets::dialog::{Dialog, Sizing, render_dialog},
    },
};

/// One key/description entry in a help column.
type Entry = (&'static str, &'static str);

struct Column {
    title: &'static str,
    rows: &'static [Entry],
}

const COLUMN_WIDTH: usize = 22;

const COLUMNS: [Column; 3] = [
    Column {
        title: "Navigation",
        rows: &[
            ("j/k", "move"),
            ("Tab/h/l", "focus"),
            ("]f / [f", "file"),
            ("]u / [u", "unreviewed"),
            ("]h / [h", "hunk"),
            ("]c / [c", "comment"),
            ("/", "search"),
            ("n / N", "next/prev match"),
            ("2 / 3", "focus panel"),
        ],
    },
    Column {
        title: "Files",
        rows: &[
            ("f", "hide/show panel"),
            ("e", "expand panel"),
            ("z", "collapse folder"),
            ("m", "file reviewed"),
            ("M", "hunk reviewed"),
        ],
    },
    Column {
        title: "Review",
        rows: &[
            ("v", "selection"),
            ("c", "comment"),
            ("s", "suggestion"),
            ("t", "threads"),
            ("\\", "split diff"),
            ("|", "expand one side"),
            ("w", "wrap long lines"),
        ],
    },
];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.help_visible {
        return;
    }
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Help "),
            body: body_lines(),
            hints: "Esc close",
            sizing: Sizing::Content { max_width: 78 },
            zones: Vec::new(),
        },
    );
}

/// Builds the help body: a title row (MUTED+BOLD per column), one row per
/// key binding (key ACCENT+BOLD, description FG) aligned in columns, and a
/// few extra key/description groups below that don't fit the column grid.
fn body_lines() -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let title_spans = COLUMNS
        .iter()
        .map(|column| {
            Span::styled(
                pad(column.title, COLUMN_WIDTH),
                Style::default()
                    .fg(theme::MUTED)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect::<Vec<_>>();
    lines.push(Line::from(title_spans));

    let max_rows = COLUMNS.iter().map(|c| c.rows.len()).max().unwrap_or(0);
    for row_index in 0..max_rows {
        let mut spans = Vec::new();
        for column in &COLUMNS {
            spans.push(match column.rows.get(row_index) {
                Some(entry) => entry_spans(*entry, COLUMN_WIDTH),
                None => vec![Span::raw(" ".repeat(COLUMN_WIDTH))],
            });
        }
        lines.push(Line::from(spans.into_iter().flatten().collect::<Vec<_>>()));
    }

    lines.push(Line::raw(""));
    lines.push(group_line(
        "Comments:",
        &[
            ("e", "edit"),
            ("x", "delete"),
            ("r", "reply"),
            ("T", "hide/show"),
        ],
    ));
    lines.push(group_line(
        "Editor:",
        &[
            ("Enter", "save"),
            ("Alt+Enter", "new line"),
            ("Esc", "close"),
        ],
    ));
    lines.push(group_line(
        "",
        &[
            ("R", "submit review"),
            ("r", "refresh"),
            ("Q", "back to list"),
            ("q", "quit"),
        ],
    ));

    lines
}

/// `key` in ACCENT+BOLD, `desc` in FG, padded with trailing spaces to
/// `width` visible columns so the next column lines up.
fn entry_spans((key, desc): Entry, width: usize) -> Vec<Span<'static>> {
    let key_span = Span::styled(
        key,
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    );
    let desc_span = Span::styled(desc, Style::default().fg(theme::FG));
    let used = display_width(key) + 2 + display_width(desc);
    let mut spans = vec![key_span, Span::raw("  "), desc_span];
    if used < width {
        spans.push(Span::raw(" ".repeat(width - used)));
    }
    spans
}

/// A standalone `Title: key desc · key desc · …` line (used for the entries
/// that don't fit the three-column grid).
fn group_line(title: &'static str, entries: &[Entry]) -> Line<'static> {
    let mut spans = Vec::new();
    if !title.is_empty() {
        spans.push(Span::styled(
            title,
            Style::default()
                .fg(theme::MUTED)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }
    for (index, (key, desc)) in entries.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme::MUTED)));
        }
        spans.push(Span::styled(
            *key,
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*desc, Style::default().fg(theme::FG)));
    }
    Line::from(spans)
}

fn pad(text: &str, width: usize) -> String {
    let used = display_width(text);
    if used >= width {
        return text.to_owned();
    }
    format!("{text}{}", " ".repeat(width - used))
}
