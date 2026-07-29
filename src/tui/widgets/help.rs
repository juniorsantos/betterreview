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
type Entry = (&'static str, &'static str);

struct Column {
    title: &'static str,
    rows: &'static [Entry],
}

const COLUMN_WIDTH: usize = 20;

const COLUMNS: [Column; 4] = [
    Column {
        title: "Move",
        rows: &[
            ("j/k", "line"),
            ("]h [h", "hunk"),
            ("]c [c", "comment"),
            ("]f [f", "file"),
            ("]u [u", "unreviewed"),
            ("gg G", "start/end"),
            ("/", "search"),
            ("n N", "match"),
            ("⇥", "focus"),
            ("2 3", "panel"),
        ],
    },
    Column {
        title: "Review",
        rows: &[
            ("m / space", "file done"),
            ("M", "hunk done"),
            ("v", "select"),
            ("y Y", "copy code/hunk"),
            ("p C", "patch/comments"),
            ("c", "comment"),
            ("s", "suggestion"),
            ("r", "reply"),
            ("e", "edit"),
            ("x", "delete"),
            ("t", "threads"),
        ],
    },
    Column {
        title: "View",
        rows: &[
            ("\\", "split"),
            ("|", "one side"),
            ("w", "wrap"),
            ("z", "expand gap"),
            ("f", "files panel"),
            ("e", "wider panel"),
            ("T", "comments"),
            ("b", "blame"),
            ("\u{2588}", "moved code"),
            ("click", "open link"),
        ],
    },
    Column {
        title: "Session",
        rows: &[
            ("R", "submit"),
            ("r", "refresh"),
            ("Ctrl+Z", "use fg"),
            ("Q", "back to list"),
            ("q", "quit"),
            ("?", "help"),
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
            hints: "⎋ close",
            sizing: Sizing::Content { max_width: 78 },
            zones: Vec::new(),
        },
    );
}
fn body_lines() -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let headings = COLUMNS
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
    lines.push(Line::from(headings));

    let rows = COLUMNS.iter().map(|c| c.rows.len()).max().unwrap_or(0);
    for index in 0..rows {
        let spans: Vec<Span<'static>> = COLUMNS
            .iter()
            .flat_map(|column| match column.rows.get(index) {
                Some(entry) => entry_spans(*entry, COLUMN_WIDTH),
                None => vec![Span::raw(" ".repeat(COLUMN_WIDTH))],
            })
            .collect();
        lines.push(Line::from(spans));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("↵", accent()),
        Span::styled("  save · ", Style::default().fg(theme::MUTED)),
        Span::styled("⌥↵", accent()),
        Span::styled("  new line · ", Style::default().fg(theme::MUTED)),
        Span::styled("⎋", accent()),
        Span::styled(
            "  close — inside the editor",
            Style::default().fg(theme::MUTED),
        ),
    ]));

    lines
}

fn accent() -> Style {
    Style::default()
        .fg(theme::ACCENT)
        .add_modifier(Modifier::BOLD)
}
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

fn pad(text: &str, width: usize) -> String {
    let used = display_width(text);
    if used >= width {
        return text.to_owned();
    }
    format!("{text}{}", " ".repeat(width - used))
}
