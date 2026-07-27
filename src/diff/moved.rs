use std::collections::{BTreeSet, HashMap};

use super::{DiffRowKind, ParsedFileDiff};

const MIN_MOVED_SUBSTANCE: usize = 20;

pub fn moved_rows(parsed: &ParsedFileDiff) -> BTreeSet<usize> {
    let removed = side(parsed, DiffRowKind::Removed);
    let added = side(parsed, DiffRowKind::Added);
    if removed.is_empty() || added.is_empty() {
        return BTreeSet::new();
    }

    let mut starts: HashMap<&str, Vec<usize>> = HashMap::new();
    for (position, (_, text)) in added.iter().enumerate() {
        starts.entry(text.as_str()).or_default().push(position);
    }

    let mut moved = BTreeSet::new();
    let mut from = 0;
    while from < removed.len() {
        let Some(candidates) = starts.get(removed[from].1.as_str()) else {
            from += 1;
            continue;
        };
        let best = candidates
            .iter()
            .map(|start| (*start, run_length(&removed[from..], &added[*start..])))
            .max_by_key(|(_, length)| *length);
        let Some((start, length)) = best.filter(|(_, length)| *length > 0) else {
            from += 1;
            continue;
        };
        if substance(&removed[from..from + length]) < MIN_MOVED_SUBSTANCE {
            from += 1;
            continue;
        }
        for offset in 0..length {
            moved.insert(removed[from + offset].0);
            moved.insert(added[start + offset].0);
        }
        from += length;
    }
    moved
}

fn side(parsed: &ParsedFileDiff, kind: DiffRowKind) -> Vec<(usize, String)> {
    parsed
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.kind == kind)
        .map(|(index, row)| (index, content(&row.raw)))
        .collect()
}

fn content(raw: &str) -> String {
    raw.get(1..).unwrap_or_default().trim().to_owned()
}

fn run_length(removed: &[(usize, String)], added: &[(usize, String)]) -> usize {
    removed
        .iter()
        .zip(added)
        .take_while(|((_, left), (_, right))| left == right && !left.is_empty())
        .count()
}

fn substance(block: &[(usize, String)]) -> usize {
    block
        .iter()
        .map(|(_, text)| text.chars().filter(char::is_ascii_alphanumeric).count())
        .sum()
}
