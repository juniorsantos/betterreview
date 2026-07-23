use crate::domain::{ChangedFile, CommitOid, DiffPosition, DiffSide, PatchAvailability, RepoPath};

use super::{DiffError, DiffHunk, DiffRow, DiffRowKind, MAX_PATCH_BYTES, ParsedFileDiff};

struct ActiveHunk {
    id: u32,
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
    old_line: u32,
    new_line: u32,
    consumed_old: u32,
    consumed_new: u32,
    row_start: usize,
}

pub fn parse_file_patch(file: &ChangedFile, head: &CommitOid) -> Result<ParsedFileDiff, DiffError> {
    let patch = match &file.patch {
        PatchAvailability::Available(patch) => patch,
        unavailable => {
            return Err(DiffError::PatchUnavailable {
                reason: unavailable_reason(unavailable),
            });
        }
    };
    if patch.len() > MAX_PATCH_BYTES {
        return Err(DiffError::PatchTooLarge {
            size: patch.len(),
            maximum: MAX_PATCH_BYTES,
        });
    }

    let mut rows = Vec::new();
    let mut hunks = Vec::new();
    let mut active = None;

    for raw in patch
        .split_terminator('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
    {
        if raw.starts_with("@@") {
            if let Some(previous) = active.take() {
                finish_hunk(previous, rows.len(), &mut hunks)?;
            }
            let (old_start, old_count, new_start, new_count) = parse_hunk_header(raw)?;
            let id = u32::try_from(hunks.len()).map_err(|_| DiffError::LineOverflow)?;
            rows.push(empty_row(raw, DiffRowKind::HunkHeader));
            active = Some(ActiveHunk {
                id,
                old_start,
                old_count,
                new_start,
                new_count,
                old_line: old_start,
                new_line: new_start,
                consumed_old: 0,
                consumed_new: 0,
                row_start: rows.len(),
            });
            continue;
        }

        let Some(hunk) = active.as_mut() else {
            rows.push(empty_row(raw, DiffRowKind::Header));
            continue;
        };

        let row = match raw.as_bytes().first().copied() {
            Some(b' ') => consume_context(hunk, &file.path, raw)?,
            Some(b'-') => consume_removed(hunk, &file.path, raw)?,
            Some(b'+') => consume_added(hunk, &file.path, raw)?,
            Some(b'\\') => empty_row(raw, DiffRowKind::Metadata),
            _ => empty_row(raw, DiffRowKind::Metadata),
        };
        rows.push(row);
    }

    if let Some(last) = active {
        finish_hunk(last, rows.len(), &mut hunks)?;
    }

    Ok(ParsedFileDiff {
        path: file.path.clone(),
        head: head.clone(),
        rows,
        hunks,
    })
}

fn parse_hunk_header(line: &str) -> Result<(u32, u32, u32, u32), DiffError> {
    let body = line
        .strip_prefix("@@ ")
        .and_then(|line| line.split_once(" @@").map(|(body, _)| body))
        .ok_or_else(|| DiffError::MalformedHunk {
            line: line.to_owned(),
        })?;
    let mut ranges = body.split_whitespace();
    let old = ranges.next().ok_or_else(|| DiffError::MalformedHunk {
        line: line.to_owned(),
    })?;
    let new = ranges.next().ok_or_else(|| DiffError::MalformedHunk {
        line: line.to_owned(),
    })?;
    if ranges.next().is_some() {
        return Err(DiffError::MalformedHunk {
            line: line.to_owned(),
        });
    }
    let (old_start, old_count) = parse_range(old, '-').ok_or_else(|| DiffError::MalformedHunk {
        line: line.to_owned(),
    })?;
    let (new_start, new_count) = parse_range(new, '+').ok_or_else(|| DiffError::MalformedHunk {
        line: line.to_owned(),
    })?;
    Ok((old_start, old_count, new_start, new_count))
}

fn parse_range(value: &str, prefix: char) -> Option<(u32, u32)> {
    let value = value.strip_prefix(prefix)?;
    match value.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((value.parse().ok()?, 1)),
    }
}

fn consume_context(
    hunk: &mut ActiveHunk,
    path: &RepoPath,
    raw: &str,
) -> Result<DiffRow, DiffError> {
    ensure_capacity(hunk, 1, 1)?;
    let old_line = hunk.old_line;
    let new_line = hunk.new_line;
    let left = position(path, DiffSide::Left, old_line, hunk.id);
    let right = position(path, DiffSide::Right, new_line, hunk.id);
    advance(hunk, 1, 1)?;
    Ok(DiffRow {
        raw: raw.to_owned(),
        kind: DiffRowKind::Context,
        old_line: Some(old_line),
        new_line: Some(new_line),
        left: Some(left),
        right: Some(right),
    })
}

fn consume_removed(
    hunk: &mut ActiveHunk,
    path: &RepoPath,
    raw: &str,
) -> Result<DiffRow, DiffError> {
    ensure_capacity(hunk, 1, 0)?;
    let old_line = hunk.old_line;
    let left = position(path, DiffSide::Left, old_line, hunk.id);
    advance(hunk, 1, 0)?;
    Ok(DiffRow {
        raw: raw.to_owned(),
        kind: DiffRowKind::Removed,
        old_line: Some(old_line),
        new_line: None,
        left: Some(left),
        right: None,
    })
}

fn consume_added(hunk: &mut ActiveHunk, path: &RepoPath, raw: &str) -> Result<DiffRow, DiffError> {
    ensure_capacity(hunk, 0, 1)?;
    let new_line = hunk.new_line;
    let right = position(path, DiffSide::Right, new_line, hunk.id);
    advance(hunk, 0, 1)?;
    Ok(DiffRow {
        raw: raw.to_owned(),
        kind: DiffRowKind::Added,
        old_line: None,
        new_line: Some(new_line),
        left: None,
        right: Some(right),
    })
}

fn ensure_capacity(
    hunk: &ActiveHunk,
    old_increment: u32,
    new_increment: u32,
) -> Result<(), DiffError> {
    let actual_old = hunk
        .consumed_old
        .checked_add(old_increment)
        .ok_or(DiffError::LineOverflow)?;
    let actual_new = hunk
        .consumed_new
        .checked_add(new_increment)
        .ok_or(DiffError::LineOverflow)?;
    if actual_old > hunk.old_count || actual_new > hunk.new_count {
        return Err(mismatch(hunk, actual_old, actual_new));
    }
    Ok(())
}

fn advance(hunk: &mut ActiveHunk, old_increment: u32, new_increment: u32) -> Result<(), DiffError> {
    hunk.consumed_old = hunk
        .consumed_old
        .checked_add(old_increment)
        .ok_or(DiffError::LineOverflow)?;
    hunk.consumed_new = hunk
        .consumed_new
        .checked_add(new_increment)
        .ok_or(DiffError::LineOverflow)?;
    hunk.old_line = hunk
        .old_line
        .checked_add(old_increment)
        .ok_or(DiffError::LineOverflow)?;
    hunk.new_line = hunk
        .new_line
        .checked_add(new_increment)
        .ok_or(DiffError::LineOverflow)?;
    Ok(())
}

fn finish_hunk(
    hunk: ActiveHunk,
    row_end: usize,
    hunks: &mut Vec<DiffHunk>,
) -> Result<(), DiffError> {
    if hunk.consumed_old != hunk.old_count || hunk.consumed_new != hunk.new_count {
        return Err(mismatch(&hunk, hunk.consumed_old, hunk.consumed_new));
    }
    hunks.push(DiffHunk {
        id: hunk.id,
        old_start: hunk.old_start,
        old_count: hunk.old_count,
        new_start: hunk.new_start,
        new_count: hunk.new_count,
        row_range: hunk.row_start..row_end,
    });
    Ok(())
}

fn mismatch(hunk: &ActiveHunk, actual_old: u32, actual_new: u32) -> DiffError {
    DiffError::HunkCountMismatch {
        hunk: hunk.id,
        expected_old: hunk.old_count,
        expected_new: hunk.new_count,
        actual_old,
        actual_new,
    }
}

fn position(path: &RepoPath, side: DiffSide, line: u32, hunk: u32) -> DiffPosition {
    DiffPosition {
        path: path.clone(),
        side,
        line,
        hunk,
    }
}

fn empty_row(raw: &str, kind: DiffRowKind) -> DiffRow {
    DiffRow {
        raw: raw.to_owned(),
        kind,
        old_line: None,
        new_line: None,
        left: None,
        right: None,
    }
}

fn unavailable_reason(availability: &PatchAvailability) -> String {
    match availability {
        PatchAvailability::Available(_) => "available".into(),
        PatchAvailability::Binary => "binary file".into(),
        PatchAvailability::TooLarge => "provider reports the patch is too large".into(),
        PatchAvailability::Collapsed => "provider collapsed the patch".into(),
        PatchAvailability::Truncated { reason } => reason.clone(),
    }
}
