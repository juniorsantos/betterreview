use super::{DiffRow, DiffRowKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitPair {
    pub left: Option<usize>,
    pub right: Option<usize>,
}

pub fn pair_rows(rows: &[DiffRow]) -> Vec<SplitPair> {
    let mut pairs = Vec::new();
    let mut removed: Vec<usize> = Vec::new();
    let mut added: Vec<usize> = Vec::new();

    for (index, row) in rows.iter().enumerate() {
        match row.kind {
            DiffRowKind::Removed if added.is_empty() => removed.push(index),
            DiffRowKind::Removed => {
                flush(&mut removed, &mut added, &mut pairs);
                removed.push(index);
            }
            DiffRowKind::Added => added.push(index),
            DiffRowKind::Context => {
                flush(&mut removed, &mut added, &mut pairs);
                pairs.push(SplitPair {
                    left: Some(index),
                    right: Some(index),
                });
            }
            DiffRowKind::Header | DiffRowKind::HunkHeader | DiffRowKind::Metadata => {
                flush(&mut removed, &mut added, &mut pairs)
            }
        }
    }
    flush(&mut removed, &mut added, &mut pairs);
    pairs
}

fn flush(removed: &mut Vec<usize>, added: &mut Vec<usize>, pairs: &mut Vec<SplitPair>) {
    for index in 0..removed.len().max(added.len()) {
        pairs.push(SplitPair {
            left: removed.get(index).copied(),
            right: added.get(index).copied(),
        });
    }
    removed.clear();
    added.clear();
}
