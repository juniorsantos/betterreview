use std::collections::HashMap;

use crate::diff::RenderedDiff;
use crate::domain::{DiffPosition, DraftComment, DraftId, RepoPath, ReviewThread, ThreadId};

use super::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentEntry {
    Draft {
        id: DraftId,
    },
    Thread {
        thread: ThreadId,
        comment_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    Diff {
        row: usize,
    },
    Comment {
        entry: CommentEntry,
        block_start: bool,
        text: String,
        author: Option<String>,
    },
    OrphanHeader,
}

/// A comment block waiting to be placed into the display, either right after
/// its anchor row or, if it has none, among the orphaned blocks at the end.
struct PendingBlock {
    entry: CommentEntry,
    body: String,
    author: Option<String>,
}

/// Builds the flattened list of rows to render: the diff rows themselves
/// interleaved, GitHub-style, with comment blocks anchored to the diff line
/// they were left on. When `hidden` is true comments are omitted entirely
/// and only the diff rows are returned, in their original order.
pub fn build_display_rows(
    rendered: &RenderedDiff,
    threads: &[ReviewThread],
    drafts: &[DraftComment],
    active_path: &RepoPath,
    hidden: bool,
) -> Vec<DisplayRow> {
    if hidden {
        return rendered
            .rows
            .iter()
            .map(|row| DisplayRow::Diff {
                row: row.binding.row_index,
            })
            .collect();
    }

    let mut anchored: HashMap<usize, Vec<PendingBlock>> = HashMap::new();
    let mut orphans: Vec<PendingBlock> = Vec::new();

    for thread in threads.iter().filter(|thread| &thread.path == active_path) {
        let fallback = thread
            .comments
            .iter()
            .find_map(|comment| comment.position.clone());
        for (comment_index, comment) in thread.comments.iter().enumerate() {
            let anchor = comment.position.clone().or_else(|| fallback.clone());
            let block = PendingBlock {
                entry: CommentEntry::Thread {
                    thread: thread.id.clone(),
                    comment_index,
                },
                body: comment.body.clone(),
                author: Some(comment.author.clone()),
            };
            place(rendered, anchor, block, &mut anchored, &mut orphans);
        }
    }

    for draft in drafts
        .iter()
        .filter(|draft| draft_belongs(draft, active_path))
    {
        let anchor = draft
            .selection
            .as_ref()
            .map(|selection| selection.end.clone());
        let block = PendingBlock {
            entry: CommentEntry::Draft {
                id: draft.id.clone(),
            },
            body: draft.body.clone(),
            author: None,
        };
        place(rendered, anchor, block, &mut anchored, &mut orphans);
    }

    let mut rows = Vec::new();
    for row in &rendered.rows {
        rows.push(DisplayRow::Diff {
            row: row.binding.row_index,
        });
        if let Some(blocks) = anchored.remove(&row.binding.row_index) {
            for block in blocks {
                push_block(&mut rows, block);
            }
        }
    }
    if !orphans.is_empty() {
        rows.push(DisplayRow::OrphanHeader);
        for block in orphans {
            push_block(&mut rows, block);
        }
    }
    rows
}

/// Convenience wrapper around [`build_display_rows`] that pulls its inputs
/// straight from `AppState`: the currently rendered diff, the active
/// change request's threads and drafts, the active file's path, and whether
/// comments are currently hidden. Returns an empty vector when there is no
/// rendered diff yet or no active file to anchor comments to.
pub fn display_rows(state: &AppState) -> Vec<DisplayRow> {
    let Some(rendered) = state.rendered_diff.as_ref() else {
        return Vec::new();
    };
    let Some(active_path) = state
        .provider
        .files
        .get(state.active_file_index)
        .map(|file| &file.path)
    else {
        return Vec::new();
    };
    build_display_rows(
        rendered,
        &state.provider.threads,
        &state.provider.drafts,
        active_path,
        state.comments_hidden,
    )
}

/// Rebuilds `state.display_rows` from the current diff/threads/drafts/active
/// file/comments_hidden inputs and resyncs `display_cursor` to the display
/// row carrying `session.cursor_row` (falling back to 0 when no row
/// matches). Call this every time one of those inputs changes: it is the
/// single place that keeps the cached rows and the cursor consistent with
/// each other, whether that's the first render after resuming a session or
/// a later mutation such as a new draft or thread update.
pub fn refresh_display_rows(state: &mut AppState) {
    state.display_rows = display_rows(state);
    let target = state.session.cursor_row;
    state.display_cursor = state
        .display_rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Diff { row } if *row == target))
        .unwrap_or(0);
}

/// Display rows whose rendered text contains `state.search_query`
/// (case-insensitive), in display order. `Diff` rows are matched against the
/// rendered diff text; `Comment` rows against their own `text`. Returns an
/// empty vector when there is no active query. Shared by the reducer (to
/// land on/step between matches) and the status bar (to show the match
/// count).
pub fn search_matches(state: &AppState) -> Vec<usize> {
    let Some(query) = state.search_query.as_deref() else {
        return Vec::new();
    };
    let needle = query.to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    state
        .display_rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            row_search_text(state, row)
                .is_some_and(|text| text.to_lowercase().contains(&needle))
                .then_some(index)
        })
        .collect()
}

fn row_search_text(state: &AppState, row: &DisplayRow) -> Option<String> {
    match row {
        DisplayRow::Diff { row } => state
            .rendered_diff
            .as_ref()?
            .rows
            .get(*row)
            .map(|rendered| line_text(&rendered.text)),
        DisplayRow::Comment { text, .. } => Some(text.clone()),
        DisplayRow::OrphanHeader => None,
    }
}

fn line_text(line: &ratatui::text::Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

/// A draft belongs to `active_path` when its selection targets that file, or
/// when it has no selection at all (its file cannot be determined, so it is
/// kept and surfaced as an orphan rather than silently dropped).
fn draft_belongs(draft: &DraftComment, active_path: &RepoPath) -> bool {
    match &draft.selection {
        Some(selection) => &selection.end.path == active_path,
        None => true,
    }
}

fn place(
    rendered: &RenderedDiff,
    anchor: Option<DiffPosition>,
    block: PendingBlock,
    anchored: &mut HashMap<usize, Vec<PendingBlock>>,
    orphans: &mut Vec<PendingBlock>,
) {
    match anchor.and_then(|position| find_anchor_row(rendered, &position)) {
        Some(row_index) => anchored.entry(row_index).or_default().push(block),
        None => orphans.push(block),
    }
}

fn find_anchor_row(rendered: &RenderedDiff, target: &DiffPosition) -> Option<usize> {
    rendered.rows.iter().find_map(|row| {
        let matches = |position: &Option<DiffPosition>| {
            position.as_ref().is_some_and(|position| {
                position.side == target.side && position.line == target.line
            })
        };
        if matches(&row.binding.left) || matches(&row.binding.right) {
            Some(row.binding.row_index)
        } else {
            None
        }
    })
}

fn push_block(rows: &mut Vec<DisplayRow>, block: PendingBlock) {
    let mut lines = block.body.lines();
    let first = lines.next().unwrap_or("");
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        block_start: true,
        text: first.to_owned(),
        author: block.author,
    });
    for line in lines {
        rows.push(DisplayRow::Comment {
            entry: block.entry.clone(),
            block_start: false,
            text: line.to_owned(),
            author: None,
        });
    }
}
