use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    app::{AppFocus, AppState},
    domain::FileStatus,
    state::ReviewSync,
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let items = state
        .provider
        .files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let progress = state.session.files.get(&file.path);
            let marker = match progress {
                Some(progress)
                    if matches!(
                        progress.sync,
                        ReviewSync::Pending { .. } | ReviewSync::Failed { .. }
                    ) =>
                {
                    "[~]"
                }
                Some(progress) if progress.reviewed => "[x]",
                _ => "[ ]",
            };
            let unresolved = state
                .provider
                .threads
                .iter()
                .filter(|thread| thread.path == file.path && !thread.resolved)
                .count();
            let drafts = state
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
            let mut line = format!("{marker} {} {}", status(file.status), file.path.0);
            if unresolved > 0 || drafts > 0 {
                line.push_str(&format!("  {unresolved} unresolved / {drafts} drafts"));
            } else if matches!(
                progress.map(|item| &item.sync),
                Some(ReviewSync::Pending { .. })
            ) {
                line.push_str("  reviewed sync pending");
            }
            let style = if index == state.active_file_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::raw(line)).style(style)
        })
        .collect::<Vec<_>>();
    let border = if state.focus == AppFocus::Files {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(" Files ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
}

fn status(status: FileStatus) -> char {
    match status {
        FileStatus::Added => 'A',
        FileStatus::Modified => 'M',
        FileStatus::Deleted => 'D',
        FileStatus::Renamed => 'R',
        FileStatus::Copied => 'C',
    }
}
