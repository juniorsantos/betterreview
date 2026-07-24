use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    app::{AppFocus, AppState, is_generated},
    domain::{ChangedFile, FileStatus},
    state::ReviewSync,
    tui::{theme, viewport},
};

enum Row<'a> {
    Directory { dir: &'a str, folded: bool },
    File { index: usize, file: &'a ChangedFile },
}

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let inner_width = area.width.saturating_sub(4) as usize;
    let mut rows = Vec::new();
    let mut current_dir: Option<&str> = None;
    let mut active_row = 0;
    for (index, file) in state.provider.files.iter().enumerate() {
        let (dir, _) = split_path(&file.path.0);
        let folded = state.collapsed_dirs.contains(dir);
        if !dir.is_empty() && current_dir != Some(dir) {
            rows.push(Row::Directory { dir, folded });
            current_dir = Some(dir);
        }
        if index == state.active_file_index {
            active_row = rows.len().saturating_sub(if folded { 1 } else { 0 });
        }
        if !folded {
            rows.push(Row::File { index, file });
        }
    }

    let visible = area.height.saturating_sub(2) as usize;
    let start = viewport::start(active_row, rows.len(), visible);
    let items = rows
        .iter()
        .skip(start)
        .take(visible)
        .map(|row| match row {
            Row::Directory { dir, folded } => {
                let (reviewed, total) = directory_progress(state, dir);
                // Chevron orientation signals the fold state: ▸ collapsed,
                // ▾ expanded (toggled with z/Enter).
                let text = if *folded {
                    format!("\u{25b8} {dir}/ ({reviewed}/{total})")
                } else {
                    format!("\u{25be} {dir}/")
                };
                // A folded folder holding the active file carries the
                // highlight so the current position never disappears.
                let holds_active = *folded
                    && state
                        .provider
                        .files
                        .get(state.active_file_index)
                        .is_some_and(|file| split_path(&file.path.0).0 == *dir);
                let mut line = Line::styled(text, Style::default().fg(theme::ACCENT));
                if holds_active {
                    let text_width = line.width();
                    if text_width < inner_width {
                        line.spans
                            .push(Span::raw(" ".repeat(inner_width - text_width)));
                    }
                    line = line.style(
                        Style::default()
                            .bg(theme::CURSOR_LINE)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                ListItem::new(line)
            }
            Row::File { index, file } => file_item(state, *index, file, inner_width),
        })
        .collect::<Vec<_>>();

    let border = if state.focus == AppFocus::Files {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(" [2] Arquivos — e expandir / z recolher ")
                .borders(Borders::ALL)
                .padding(ratatui::widgets::Padding::horizontal(1))
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
}

fn file_item<'a>(
    state: &AppState,
    index: usize,
    file: &'a ChangedFile,
    inner_width: usize,
) -> ListItem<'a> {
    let progress = state.session.files.get(&file.path);
    let (marker, marker_color) = match progress {
        Some(progress)
            if matches!(
                progress.sync,
                ReviewSync::Pending { .. } | ReviewSync::Failed { .. }
            ) =>
        {
            ("[~]", theme::WARNING)
        }
        Some(progress) if progress.reviewed => ("[x]", theme::SUCCESS),
        _ => ("[ ]", theme::MUTED),
    };
    let notes = state
        .provider
        .threads
        .iter()
        .filter(|thread| thread.path == file.path && !thread.resolved)
        .count()
        + state
            .provider
            .drafts
            .iter()
            .filter(|draft| {
                draft
                    .selection
                    .as_ref()
                    .is_some_and(|selection| selection.end.path == file.path)
            })
            .count();
    let (_, name) = split_path(&file.path.0);
    let generated = is_generated(&file.path.0);

    let mut right = Vec::new();
    if notes > 0 {
        right.push(Span::styled(
            format!("*{notes} "),
            Style::default().fg(theme::ACCENT),
        ));
    }
    if file.additions > 0 {
        right.push(Span::styled(
            format!("+{} ", file.additions),
            Style::default().fg(theme::SUCCESS),
        ));
    }
    if file.deletions > 0 {
        right.push(Span::styled(
            format!("-{} ", file.deletions),
            Style::default().fg(theme::DANGER),
        ));
    }
    if let Some(last) = right.last_mut() {
        *last = Span::styled(last.content.trim_end().to_owned(), last.style);
    }

    let left_prefix = format!("{marker} {} ", status_letter(file.status));
    let right_width: usize = right.iter().map(|span| span.content.chars().count()).sum();
    let name_budget = inner_width
        .saturating_sub(left_prefix.chars().count() + right_width + 1)
        .max(1);
    let shown_name = truncate(name, name_budget);
    let padding = inner_width
        .saturating_sub(left_prefix.chars().count() + shown_name.chars().count() + right_width);

    let status_span = if generated {
        Span::styled("\u{2298} ".to_owned(), Style::default().fg(theme::MUTED))
    } else {
        Span::styled(
            format!("{} ", status_letter(file.status)),
            Style::default().fg(status_color(file.status)),
        )
    };
    let name_span = if generated {
        Span::styled(shown_name, Style::default().fg(theme::MUTED))
    } else {
        Span::raw(shown_name)
    };

    let mut spans = vec![
        Span::styled(format!("{marker} "), Style::default().fg(marker_color)),
        status_span,
        name_span,
        Span::raw(" ".repeat(padding)),
    ];
    spans.extend(right);

    let style = if index == state.active_file_index {
        Style::default()
            .bg(theme::CURSOR_LINE)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    ListItem::new(Line::from(spans)).style(style)
}

fn directory_progress(state: &AppState, dir: &str) -> (usize, usize) {
    let mut reviewed = 0;
    let mut total = 0;
    for file in &state.provider.files {
        if split_path(&file.path.0).0 == dir {
            total += 1;
            if state
                .session
                .files
                .get(&file.path)
                .is_some_and(|progress| progress.reviewed)
            {
                reviewed += 1;
            }
        }
    }
    (reviewed, total)
}

fn split_path(path: &str) -> (&str, &str) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (dir, name),
        None => ("", path),
    }
}

fn truncate(name: &str, budget: usize) -> String {
    if name.chars().count() <= budget {
        return name.to_owned();
    }
    let mut shown: String = name.chars().take(budget.saturating_sub(1)).collect();
    shown.push('…');
    shown
}

fn status_letter(status: FileStatus) -> char {
    match status {
        FileStatus::Added => 'A',
        FileStatus::Modified => 'M',
        FileStatus::Deleted => 'D',
        FileStatus::Renamed => 'R',
        FileStatus::Copied => 'C',
    }
}

fn status_color(status: FileStatus) -> Color {
    match status {
        FileStatus::Added => theme::SUCCESS,
        FileStatus::Modified => theme::WARNING,
        FileStatus::Deleted => theme::DANGER,
        FileStatus::Renamed | FileStatus::Copied => theme::ACCENT,
    }
}
