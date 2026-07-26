use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

const INDENT_WIDTH: usize = 1;

use crate::{
    app::{AppFocus, AppState, is_generated},
    domain::{ChangedFile, FileStatus},
    state::ReviewSync,
    tui::{
        text::{abbreviate_path, display_width, panel_title},
        theme, viewport,
    },
};

/// One row of the files panel: a directory header or a visible (unfolded)
/// file. Shared between the widget's rendering and the mouse click handler,
/// which maps a click's row position through the same list to find what it
/// landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilesRow<'a> {
    Directory {
        path: &'a str,
        label: &'a str,
        depth: usize,
    },
    File {
        index: usize,
        depth: usize,
    },
}

/// Builds the files panel's visible rows — directory headers interleaved
/// with unfolded files, already windowed to the viewport around the active
/// file for a panel of `height` rows.
pub(crate) fn visible_rows(state: &AppState, height: u16) -> Vec<FilesRow<'_>> {
    let mut rows = Vec::new();
    let mut current_dir: Option<&str> = None;
    let mut ancestors: Vec<&str> = Vec::new();
    let mut depth = 0;
    let mut active_row = 0;
    for (index, file) in state.provider.files.iter().enumerate() {
        let (dir, _) = split_path(&file.path.0);
        let folded = state.collapsed_dirs.contains(dir);
        if !dir.is_empty() && current_dir != Some(dir) {
            while ancestors
                .last()
                .is_some_and(|parent| !encloses(parent, dir))
            {
                ancestors.pop();
            }
            depth = ancestors.len();
            let label = match ancestors.last() {
                Some(parent) => &dir[parent.len() + 1..],
                None => dir,
            };
            rows.push(FilesRow::Directory {
                path: dir,
                label,
                depth,
            });
            ancestors.push(dir);
            current_dir = Some(dir);
        }
        if index == state.active_file_index {
            active_row = rows.len().saturating_sub(if folded { 1 } else { 0 });
        }
        if !folded {
            let depth = if dir.is_empty() { 0 } else { depth + 1 };
            rows.push(FilesRow::File { index, depth });
        }
    }

    let visible = height.saturating_sub(2) as usize;
    let start = viewport::start(active_row, rows.len(), visible);
    rows.into_iter().skip(start).take(visible).collect()
}

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let border = if state.focus == AppFocus::Files {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    let block = Block::default()
        .title(files_title(state, area.width))
        .borders(Borders::ALL)
        .padding(ratatui::widgets::Padding::horizontal(1))
        .border_style(Style::default().fg(border));
    let inner_width = block.inner(area).width as usize;
    let rows = visible_rows(state, area.height);
    let items = rows
        .iter()
        .map(|row| match row {
            FilesRow::Directory { path, label, depth } => {
                let folded = state.collapsed_dirs.contains(*path);
                let (reviewed, total) = directory_progress(state, path);
                // Chevron orientation signals the fold state: ▸ collapsed,
                // ▾ expanded (toggled with z/Enter).
                let text = if folded {
                    format!("\u{25b8} {label}/ ({reviewed}/{total})")
                } else {
                    format!("\u{25be} {label}/")
                };
                // A folded folder holding the active file carries the
                // highlight so the current position never disappears.
                let holds_active = (folded || state.folder_selected)
                    && state
                        .provider
                        .files
                        .get(state.active_file_index)
                        .is_some_and(|file| split_path(&file.path.0).0 == *path);
                let mut spans = indent(*depth);
                spans.push(Span::styled(text, Style::default().fg(theme::ACCENT)));
                let mut line = Line::from(spans);
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
            FilesRow::File { index, depth } => file_item(
                state,
                *index,
                &state.provider.files[*index],
                inner_width,
                *depth,
            ),
        })
        .collect::<Vec<_>>();

    frame.render_widget(List::new(items).block(block), area);
}

fn files_title(state: &AppState, width: u16) -> String {
    let total = state.provider.files.len();
    let reviewed = state
        .provider
        .files
        .iter()
        .filter(|file| {
            state
                .session
                .files
                .get(&file.path)
                .is_some_and(|progress| progress.reviewed)
        })
        .count();
    panel_title("[2] Files", Some(&format!("{reviewed}/{total}")), width)
}

fn file_item<'a>(
    state: &AppState,
    index: usize,
    file: &'a ChangedFile,
    inner_width: usize,
    depth: usize,
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
    let total_hunks = state.hunk_total(&file.path);
    if total_hunks > 0 {
        let done = progress.map_or(0, |progress| progress.reviewed_hunks.len());
        let color = match done as u32 {
            0 => theme::MUTED,
            done if done == total_hunks => theme::SUCCESS,
            _ => theme::ACCENT,
        };
        right.push(Span::styled(
            format!("{done}/{total_hunks} "),
            Style::default().fg(color),
        ));
    }
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

    let indent_width = depth * INDENT_WIDTH;
    let flagged = state.flagged_files.contains(&file.path);
    let left_prefix = format!(
        "{marker} {}{} ",
        if flagged { "\u{26a0} " } else { "" },
        status_letter(file.status)
    );
    let right_width: usize = right.iter().map(|span| display_width(&span.content)).sum();
    let name_budget = inner_width
        .saturating_sub(indent_width + display_width(&left_prefix) + right_width + 1)
        .max(1);
    let shown_name = abbreviate_path(name, name_budget);
    let padding = inner_width.saturating_sub(
        indent_width + display_width(&left_prefix) + display_width(&shown_name) + right_width,
    );

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

    let mut spans = indent(depth);
    if flagged {
        spans.push(Span::styled(
            "\u{26a0} ".to_owned(),
            Style::default().fg(theme::DANGER),
        ));
    }
    spans.extend([
        Span::styled(format!("{marker} "), Style::default().fg(marker_color)),
        status_span,
        name_span,
        Span::raw(" ".repeat(padding)),
    ]);
    spans.extend(right);

    let style = if index == state.active_file_index && !state.folder_selected {
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

fn encloses(parent: &str, child: &str) -> bool {
    child.len() > parent.len()
        && child.starts_with(parent)
        && child.as_bytes()[parent.len()] == b'/'
}

fn indent(depth: usize) -> Vec<Span<'static>> {
    if depth == 0 {
        return Vec::new();
    }
    vec![Span::raw(" ".repeat(depth * INDENT_WIDTH))]
}

fn split_path(path: &str) -> (&str, &str) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (dir, name),
        None => ("", path),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChangeRequestKey, CommitOid, PatchAvailability, ProviderCapabilities, ProviderKind,
        ProviderSnapshot,
    };
    use crate::state::{SESSION_SCHEMA_VERSION, SessionSnapshot};

    fn changed_file(path: &str) -> ChangedFile {
        ChangedFile {
            path: crate::domain::RepoPath(path.into()),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            patch: PatchAvailability::Available("@@ -1 +1 @@\n-old\n+new\n".into()),
            base_blob: None,
            head_blob: None,
            remotely_reviewed: Some(false),
        }
    }

    fn state_with_files(paths: &[&str]) -> AppState {
        let key = ChangeRequestKey {
            provider: ProviderKind::GitHub,
            host: "github.com".into(),
            repository: "owner/repo".into(),
            number: 1,
        };
        let provider = ProviderSnapshot {
            key: key.clone(),
            title: String::new(),
            author: String::new(),
            web_url: String::new(),
            base: CommitOid("base".into()),
            head: CommitOid("head".into()),
            files: paths.iter().map(|path| changed_file(path)).collect(),
            threads: Vec::new(),
            drafts: Vec::new(),
            capabilities: ProviderCapabilities::all_supported(),
        };
        let session = SessionSnapshot {
            schema_version: SESSION_SCHEMA_VERSION,
            key,
            base: CommitOid("base".into()),
            head: CommitOid("head".into()),
            active_file: None,
            cursor_row: 0,
            scroll_row: 0,
            files: Default::default(),
            editor: None,
            pending_submit: None,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        AppState::new(provider, session)
    }

    #[test]
    fn lists_directory_headers_interleaved_with_their_files() {
        let state = state_with_files(&["a/one.rs", "a/two.rs", "b/three.rs"]);

        let rows = visible_rows(&state, 10);

        assert_eq!(
            rows,
            vec![
                FilesRow::Directory {
                    path: "a",
                    label: "a",
                    depth: 0,
                },
                FilesRow::File { index: 0, depth: 1 },
                FilesRow::File { index: 1, depth: 1 },
                FilesRow::Directory {
                    path: "b",
                    label: "b",
                    depth: 0,
                },
                FilesRow::File { index: 2, depth: 1 },
            ]
        );
    }

    #[test]
    fn folded_directory_hides_its_files_but_keeps_its_header() {
        let mut state = state_with_files(&["a/one.rs", "a/two.rs", "b/three.rs"]);
        state.collapsed_dirs.insert("a".into());

        let rows = visible_rows(&state, 10);

        assert_eq!(
            rows,
            vec![
                FilesRow::Directory {
                    path: "a",
                    label: "a",
                    depth: 0,
                },
                FilesRow::Directory {
                    path: "b",
                    label: "b",
                    depth: 0,
                },
                FilesRow::File { index: 2, depth: 1 },
            ]
        );
    }

    #[test]
    fn scrolled_viewport_windows_around_the_active_file() {
        // Ten single-file directories, no active-file grouping ambiguity:
        // each directory contributes exactly one header + one file row, so
        // row `2 * index` is the header for file `index` and `2 * index + 1`
        // is the file itself.
        let paths: Vec<String> = (0..10).map(|index| format!("d{index}/f.rs")).collect();
        let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let mut state = state_with_files(&path_refs);
        state.active_file_index = 9;

        // height 6 => visible = 4; active row is 2*9+1 = 19 of 20 total rows.
        let rows = visible_rows(&state, 6);

        assert_eq!(rows.len(), 4);
        // viewport::start(19, 20, 4) = 19 - 2 = 17, clamped to 20 - 4 = 16.
        assert_eq!(
            rows[0],
            FilesRow::Directory {
                path: "d8",
                label: "d8",
                depth: 0,
            }
        );
        assert_eq!(rows[3], FilesRow::File { index: 9, depth: 1 });
    }
}
