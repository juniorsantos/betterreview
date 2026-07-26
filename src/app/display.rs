use std::collections::{BTreeMap, BTreeSet, HashMap};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentRowKind {
    Spacer,
    Header,
    Actions,
    Body,
    Footer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    FileHeader {
        path: String,
        previous_path: Option<String>,
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
    Gap {
        after_new_line: u32,
        hidden: usize,
    },
    Context {
        old_line: Option<u32>,
        new_line: u32,
        text: String,
    },
    HunkHeader {
        hunk: u32,
    },
    SplitDiff {
        left: Option<usize>,
        right: Option<usize>,
    },
}

impl DisplayRow {
    pub fn anchor_row(&self) -> Option<usize> {
        match self {
            DisplayRow::Diff { row } => Some(*row),
            DisplayRow::SplitDiff { left, right } => right.or(*left),
            _ => None,
        }
    }
}

struct PendingBlock {
    entry: CommentEntry,
    body: String,
    author: Option<String>,
}

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

pub fn refresh_display_rows(state: &mut AppState) {
    state.display_rows = display_rows(state);
    if let Some(parsed) = state.parsed_diff.as_ref() {
        let kinds: Vec<crate::diff::DiffRowKind> = parsed.rows.iter().map(|row| row.kind).collect();
        let hunk_ids: BTreeMap<usize, u32> = parsed
            .hunks
            .iter()
            .filter_map(|hunk| Some((hunk.row_range.start.checked_sub(1)?, hunk.id)))
            .collect();
        let rows = std::mem::take(&mut state.display_rows);
        state.display_rows = rows
            .into_iter()
            .filter_map(|row| match row {
                DisplayRow::Diff { row: index } => match kinds.get(index) {
                    Some(crate::diff::DiffRowKind::HunkHeader) => hunk_ids
                        .get(&index)
                        .map(|hunk| DisplayRow::HunkHeader { hunk: *hunk }),
                    Some(crate::diff::DiffRowKind::Header | crate::diff::DiffRowKind::Metadata) => {
                        None
                    }
                    _ => Some(DisplayRow::Diff { row: index }),
                },
                other => Some(other),
            })
            .collect();
        state.display_rows.insert(
            0,
            DisplayRow::FileHeader {
                path: parsed.path.0.clone(),
                previous_path: state
                    .provider
                    .files
                    .get(state.active_file_index)
                    .and_then(|file| file.previous_path.as_ref())
                    .map(|previous| previous.0.clone()),
            },
        );
    }
    insert_gap_rows(state);
    fold_split_rows(state);
    let target = state.session.cursor_row;
    state.display_cursor = state
        .display_rows
        .iter()
        .position(|row| row.anchor_row() == Some(target))
        .or_else(|| {
            state
                .display_rows
                .iter()
                .enumerate()
                .find_map(|(index, row)| {
                    row.anchor_row()
                        .is_some_and(|row| row >= target)
                        .then_some(index)
                })
        })
        .unwrap_or(0);
    if let Some(row) = state
        .display_rows
        .get(state.display_cursor)
        .and_then(DisplayRow::anchor_row)
        && row != target
    {
        state.session.cursor_row = row;
        state.dirty = true;
    }
}

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
        let anchor = match display_index.checked_sub(1) {
            Some(previous)
                if matches!(state.display_rows[previous], DisplayRow::HunkHeader { .. }) =>
            {
                previous
            }
            _ => display_index,
        };
        match last_new_line {
            None if new_line > 1 => {
                insert_before.insert(anchor, (0, (new_line - 1) as usize));
            }
            Some(prev) if new_line > prev + 1 => {
                insert_before.insert(anchor, (prev, (new_line - prev - 1) as usize));
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
        (None, Some(prev), Some(last_index)) => {
            insert_before.insert(last_index + 1, (prev, 0));
        }
        _ => {}
    }

    if insert_before.is_empty() {
        return;
    }

    let offsets: BTreeMap<u32, Option<i64>> = insert_before
        .values()
        .map(|(after, _)| (*after, old_offset(&parsed.hunks, *after)))
        .collect();
    let old_rows = std::mem::take(&mut state.display_rows);
    let old_len = old_rows.len();
    let mut new_rows = Vec::with_capacity(old_len + insert_before.len());
    for (index, row) in old_rows.into_iter().enumerate() {
        if let Some((after, hidden)) = insert_before.remove(&index) {
            push_gap_row(
                state,
                &mut new_rows,
                after,
                hidden,
                &active_path,
                offsets.get(&after).copied().flatten(),
            );
        }
        new_rows.push(row);
    }
    if let Some((after, hidden)) = insert_before.remove(&old_len) {
        push_gap_row(
            state,
            &mut new_rows,
            after,
            hidden,
            &active_path,
            offsets.get(&after).copied().flatten(),
        );
    }
    state.display_rows = new_rows;
}

fn old_offset(hunks: &[crate::diff::DiffHunk], after_new_line: u32) -> Option<i64> {
    if let Some(next) = hunks
        .iter()
        .find(|hunk| hunk.new_start as i64 > after_new_line as i64)
    {
        return Some(next.old_start as i64 - next.new_start as i64);
    }
    let last = hunks.last()?;
    Some(
        (last.old_start as i64 + last.old_count as i64)
            - (last.new_start as i64 + last.new_count as i64),
    )
}

fn push_gap_row(
    state: &AppState,
    rows: &mut Vec<DisplayRow>,
    after_new_line: u32,
    hidden: usize,
    active_path: &RepoPath,
    old_offset: Option<i64>,
) {
    if state.expanded_gaps.contains(&after_new_line) {
        if let Some(lines) = state.file_contexts.get(active_path) {
            for offset in 1..=hidden as u32 {
                let new_line = after_new_line + offset;
                let text = lines
                    .get((new_line - 1) as usize)
                    .cloned()
                    .unwrap_or_default();
                rows.push(DisplayRow::Context {
                    old_line: old_offset
                        .map(|delta| new_line as i64 + delta)
                        .filter(|line| *line >= 1)
                        .map(|line| line as u32),
                    new_line,
                    text,
                });
            }
            return;
        }
    }
    rows.push(DisplayRow::Gap {
        after_new_line,
        hidden,
    });
}

pub const SPLIT_MIN_DIFF_WIDTH: u16 = 94;

pub fn sync_terminal_width(state: &mut AppState, width: u16) -> bool {
    if state.terminal_width == width {
        return false;
    }
    state.terminal_width = width;
    refresh_display_rows(state);
    true
}

pub fn diff_panel_width(state: &AppState) -> u16 {
    if state.files_hidden {
        return state.terminal_width;
    }
    let panel = if state.files_expanded { 50 } else { 30 };
    state.terminal_width.saturating_sub(panel)
}

fn fold_split_rows(state: &mut AppState) {
    if state.diff_layout != crate::domain::DiffLayout::Split
        || diff_panel_width(state) < SPLIT_MIN_DIFF_WIDTH
    {
        return;
    }
    let Some(parsed) = state.parsed_diff.as_ref() else {
        return;
    };
    let pairs = crate::diff::pair_rows(&parsed.rows);
    let mut pair_of_row: BTreeMap<usize, usize> = BTreeMap::new();
    for (index, pair) in pairs.iter().enumerate() {
        for row in [pair.left, pair.right].into_iter().flatten() {
            pair_of_row.insert(row, index);
        }
    }
    let mut emitted: BTreeSet<usize> = BTreeSet::new();
    let rows = std::mem::take(&mut state.display_rows);
    state.display_rows = rows
        .into_iter()
        .filter_map(|row| match row {
            DisplayRow::Diff { row: index } => {
                let pair_index = pair_of_row.get(&index)?;
                if !emitted.insert(*pair_index) {
                    return None;
                }
                let pair = pairs[*pair_index];
                Some(DisplayRow::SplitDiff {
                    left: pair.left,
                    right: pair.right,
                })
            }
            other => Some(other),
        })
        .collect();
}

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
        DisplayRow::Diff { .. } | DisplayRow::SplitDiff { .. } => {
            let rendered = state.rendered_diff.as_ref()?;
            let text = [row.anchor_row(), split_left(row)]
                .into_iter()
                .flatten()
                .filter_map(|index| rendered.rows.get(index))
                .map(|rendered| line_text(&rendered.text))
                .collect::<Vec<_>>()
                .join(" ");
            Some(text)
        }
        DisplayRow::Comment { text, .. } => Some(text.clone()),
        DisplayRow::Context { text, .. } => Some(text.clone()),
        DisplayRow::FileHeader { .. }
        | DisplayRow::OrphanHeader
        | DisplayRow::Gap { .. }
        | DisplayRow::HunkHeader { .. } => None,
    }
}

fn split_left(row: &DisplayRow) -> Option<usize> {
    match row {
        DisplayRow::SplitDiff { left, .. } => *left,
        _ => None,
    }
}

fn line_text(line: &ratatui::text::Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

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

pub fn commented_rows(state: &AppState) -> std::collections::BTreeSet<usize> {
    let mut rows = std::collections::BTreeSet::new();
    let (Some(rendered), Some(active_path)) = (
        state.rendered_diff.as_ref(),
        state
            .provider
            .files
            .get(state.active_file_index)
            .map(|file| file.path.clone()),
    ) else {
        return rows;
    };
    for draft in state
        .provider
        .drafts
        .iter()
        .filter(|draft| draft_belongs(draft, &active_path))
    {
        let Some(selection) = draft.selection.as_ref() else {
            continue;
        };
        let first = find_anchor_row(rendered, &selection.start);
        let last = find_anchor_row(rendered, &selection.end);
        match (first, last) {
            (Some(first), Some(last)) => rows.extend(first.min(last)..=first.max(last)),
            (Some(only), None) | (None, Some(only)) => {
                rows.insert(only);
            }
            (None, None) => {}
        }
    }
    for thread in state
        .provider
        .threads
        .iter()
        .filter(|thread| thread.path == active_path)
    {
        for position in thread.comments.iter().filter_map(|c| c.position.as_ref()) {
            if let Some(row) = find_anchor_row(rendered, position) {
                rows.insert(row);
            }
        }
    }
    rows
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
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        kind: CommentRowKind::Spacer,
        text: String::new(),
        author: None,
    });
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        kind: CommentRowKind::Header,
        text: String::new(),
        author: block.author,
    });
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        kind: CommentRowKind::Body,
        text: String::new(),
        author: None,
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
        kind: CommentRowKind::Body,
        text: String::new(),
        author: None,
    });
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        kind: CommentRowKind::Footer,
        text: String::new(),
        author: None,
    });
    rows.push(DisplayRow::Comment {
        entry: block.entry.clone(),
        kind: CommentRowKind::Actions,
        text: String::new(),
        author: None,
    });
    rows.push(DisplayRow::Comment {
        entry: block.entry,
        kind: CommentRowKind::Spacer,
        text: String::new(),
        author: None,
    });
}
