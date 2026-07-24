use std::collections::{BTreeMap, HashMap};

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

/// Which terminal line of a comment card a row renders: the top border with
/// the author/state meta, a body line, or the bottom border with the key
/// hints. Navigation stops only on `Header`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentRowKind {
    Header,
    Body,
    Footer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    /// Single line at the top naming the file under review (replaces the
    /// raw `diff --git`/`index` header block).
    FileHeader {
        path: String,
    },
    Diff {
        row: usize,
    },
    Comment {
        entry: CommentEntry,
        kind: CommentRowKind,
        text: String,
        author: Option<String>,
    },
    OrphanHeader,
    /// A run of unchanged lines the diff didn't show, sitting right after
    /// new-file line `after_new_line` (`0` for a gap before the first hunk).
    /// `hidden` is how many lines it collapses. Replaced by `Context` rows
    /// once its key is in `AppState::expanded_gaps` and the file's contents
    /// are cached in `AppState::file_contexts`.
    Gap {
        after_new_line: u32,
        hidden: usize,
    },
    /// One expanded line of unchanged context, pulled from the cached file
    /// contents at the head revision.
    Context {
        new_line: u32,
        text: String,
    },
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
    // File headers and metadata (diff --git, index, ---/+++) are noise in a
    // panel that already names the file; keep only hunk headers and code.
    if let Some(parsed) = state.parsed_diff.as_ref() {
        let hidden: Vec<bool> = parsed
            .rows
            .iter()
            .map(|row| {
                matches!(
                    row.kind,
                    crate::diff::DiffRowKind::Header
                        | crate::diff::DiffRowKind::Metadata
                        | crate::diff::DiffRowKind::HunkHeader
                )
            })
            .collect();
        state
            .display_rows
            .retain(|row| !matches!(row, DisplayRow::Diff { row } if hidden.get(*row).copied().unwrap_or(false)));
        state.display_rows.insert(
            0,
            DisplayRow::FileHeader {
                path: parsed.path.0.clone(),
            },
        );
    }
    insert_gap_rows(state);
    let target = state.session.cursor_row;
    state.display_cursor = state
        .display_rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Diff { row } if *row == target))
        .or_else(|| {
            // The row `session.cursor_row` pointed at is no longer in the
            // display (e.g. a header/metadata row hidden by a later
            // refresh): snap forward to the nearest Diff row that still is,
            // and rewrite `session.cursor_row` to match so future refreshes
            // (and the persisted session) agree with where the cursor
            // actually landed.
            state
                .display_rows
                .iter()
                .enumerate()
                .find_map(|(index, row)| {
                    matches!(row, DisplayRow::Diff { row } if *row >= target).then_some(index)
                })
        })
        .unwrap_or(0);
    if let Some(DisplayRow::Diff { row }) = state.display_rows.get(state.display_cursor)
        && *row != target
    {
        state.session.cursor_row = *row;
        state.dirty = true;
    }
}

/// Finds runs of unchanged lines the parsed diff skipped over — between
/// hunks, before the first hunk, and after the last one — and splices a
/// `Gap` row (or, when the gap is expanded and the file's contents are
/// cached, one `Context` row per hidden line) into `state.display_rows` at
/// each spot. A no-op when there's no parsed diff or no active file, and
/// when every consecutive pair of code rows is already contiguous.
fn insert_gap_rows(state: &mut AppState) {
    let Some(parsed) = state.parsed_diff.as_ref() else {
        return;
    };
    let Some(active_path) = state
        .provider
        .files
        .get(state.active_file_index)
        .map(|file| file.path.clone())
    else {
        return;
    };

    let new_lines: Vec<Option<u32>> = parsed.rows.iter().map(|row| row.new_line).collect();
    let total_lines = state
        .file_contexts
        .get(&active_path)
        .map(|lines| lines.len() as u32);

    // Keyed by the display index a gap sits *before*; `old_len` (past the
    // last index) means "append at the very end".
    let mut insert_before: BTreeMap<usize, (u32, usize)> = BTreeMap::new();
    let mut last_new_line: Option<u32> = None;
    let mut last_diff_display_index: Option<usize> = None;

    for (display_index, row) in state.display_rows.iter().enumerate() {
        let DisplayRow::Diff { row: parsed_index } = row else {
            continue;
        };
        last_diff_display_index = Some(display_index);
        let Some(new_line) = new_lines.get(*parsed_index).copied().flatten() else {
            continue;
        };
        match last_new_line {
            None if new_line > 1 => {
                insert_before.insert(display_index, (0, (new_line - 1) as usize));
            }
            Some(prev) if new_line > prev + 1 => {
                insert_before.insert(display_index, (prev, (new_line - prev - 1) as usize));
            }
            _ => {}
        }
        last_new_line = Some(new_line);
    }

    match (total_lines, last_new_line, last_diff_display_index) {
        (Some(total), Some(prev), Some(last_index)) => {
            if total > prev {
                insert_before.insert(last_index + 1, (prev, (total - prev) as usize));
            }
        }
        // File length still unknown: offer a speculative trailing gap
        // (hidden == 0 is the "unknown" sentinel) so `z` can bootstrap the
        // first load even in single-hunk diffs.
        (None, Some(prev), Some(last_index)) => {
            insert_before.insert(last_index + 1, (prev, 0));
        }
        _ => {}
    }

    if insert_before.is_empty() {
        return;
    }

    let old_rows = std::mem::take(&mut state.display_rows);
    let old_len = old_rows.len();
    let mut new_rows = Vec::with_capacity(old_len + insert_before.len());
    for (index, row) in old_rows.into_iter().enumerate() {
        if let Some((after, hidden)) = insert_before.remove(&index) {
            push_gap_row(state, &mut new_rows, after, hidden, &active_path);
        }
        new_rows.push(row);
    }
    if let Some((after, hidden)) = insert_before.remove(&old_len) {
        push_gap_row(state, &mut new_rows, after, hidden, &active_path);
    }
    state.display_rows = new_rows;
}

/// Pushes either a single collapsed `Gap` row, or — when `after_new_line` is
/// in `expanded_gaps` and the file's contents are cached — one `Context` row
/// per hidden line, numbered from `after_new_line + 1`.
fn push_gap_row(
    state: &AppState,
    rows: &mut Vec<DisplayRow>,
    after_new_line: u32,
    hidden: usize,
    active_path: &RepoPath,
) {
    if state.expanded_gaps.contains(&after_new_line) {
        if let Some(lines) = state.file_contexts.get(active_path) {
            for offset in 1..=hidden as u32 {
                let new_line = after_new_line + offset;
                let text = lines
                    .get((new_line - 1) as usize)
                    .cloned()
                    .unwrap_or_default();
                rows.push(DisplayRow::Context { new_line, text });
            }
            return;
        }
    }
    rows.push(DisplayRow::Gap {
        after_new_line,
        hidden,
    });
}

/// Display rows whose rendered text contains `state.search_query`
/// (case-insensitive), in display order. `Diff` rows are matched against the
/// rendered diff text; `Comment` rows (including `Body` lines) against their
/// own `text`, but a match inside a comment is reported at its block's
/// `Header` row — the only row inside a comment block that is a navigation
/// stop — with consecutive duplicates from the same block collapsed. Returns
/// an empty vector when there is no active query. Shared by the reducer (to
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
    let mut matches = Vec::new();
    for (index, row) in state.display_rows.iter().enumerate() {
        let Some(text) = row_search_text(state, row) else {
            continue;
        };
        if !text.to_lowercase().contains(&needle) {
            continue;
        }
        let target = match row {
            DisplayRow::Comment { .. } => block_header_index(&state.display_rows, index),
            _ => index,
        };
        if matches.last() != Some(&target) {
            matches.push(target);
        }
    }
    matches
}

/// Walks backward from `index` to the nearest `Comment` row with
/// `CommentRowKind::Header` — the start of the block `index` belongs to.
/// Comment blocks are always pushed contiguously (`Header`, `Body`*,
/// `Footer`), so this always finds one for a genuine comment row.
fn block_header_index(rows: &[DisplayRow], index: usize) -> usize {
    let mut cursor = index;
    loop {
        if matches!(
            rows[cursor],
            DisplayRow::Comment {
                kind: CommentRowKind::Header,
                ..
            }
        ) {
            return cursor;
        }
        match cursor.checked_sub(1) {
            Some(previous) => cursor = previous,
            None => return index,
        }
    }
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
        DisplayRow::Context { text, .. } => Some(text.clone()),
        DisplayRow::FileHeader { .. } | DisplayRow::OrphanHeader | DisplayRow::Gap { .. } => None,
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
    // A card is a bordered box: top border (meta), one body row per line,
    // bottom border (key hints). Every row is one terminal line.
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        kind: CommentRowKind::Header,
        text: String::new(),
        author: block.author,
    });
    let mut body_lines = block.body.lines().peekable();
    if body_lines.peek().is_none() {
        rows.push(DisplayRow::Comment {
            entry: block.entry.clone(),
            kind: CommentRowKind::Body,
            text: String::new(),
            author: None,
        });
    }
    for line in body_lines {
        rows.push(DisplayRow::Comment {
            entry: block.entry.clone(),
            kind: CommentRowKind::Body,
            text: line.to_owned(),
            author: None,
        });
    }
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        kind: CommentRowKind::Footer,
        text: String::new(),
        author: None,
    });
}
