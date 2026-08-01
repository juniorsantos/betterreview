use betterreview::{
    app::{CommentEntry, CommentRowKind, DisplayRow, build_display_rows},
    diff::{RenderedDiff, RenderedRow, RowBinding},
    domain::{
        DiffPosition, DiffSelection, DiffSide, DraftComment, DraftId, RepoPath, ReviewComment,
        ReviewThread, ThreadId,
    },
};
use ratatui::text::Line;

fn pos(path: &RepoPath, side: DiffSide, line: u32) -> DiffPosition {
    DiffPosition {
        path: path.clone(),
        side,
        line,
        hunk: 0,
        old_line: None,
        new_line: None,
    }
}

fn row(row_index: usize, left: Option<DiffPosition>, right: Option<DiffPosition>) -> RenderedRow {
    RenderedRow {
        text: Line::raw(format!("row-{row_index}")),
        binding: RowBinding {
            row_index,
            left,
            right,
        },
    }
}

/// Three rows:
/// - row 0: left line 1 / right line 1
/// - row 1: left line 2 / no right
/// - row 2: no left / right line 3
fn rendered(path: &RepoPath) -> RenderedDiff {
    RenderedDiff {
        rows: vec![
            row(
                0,
                Some(pos(path, DiffSide::Left, 1)),
                Some(pos(path, DiffSide::Right, 1)),
            ),
            row(1, Some(pos(path, DiffSide::Left, 2)), None),
            row(2, None, Some(pos(path, DiffSide::Right, 3))),
        ],
    }
}

fn active_path() -> RepoPath {
    RepoPath("src/app.rs".into())
}

fn other_path() -> RepoPath {
    RepoPath("src/other.rs".into())
}

fn thread_comment(
    id: &str,
    author: &str,
    body: &str,
    position: Option<DiffPosition>,
) -> ReviewComment {
    ReviewComment {
        id: id.into(),
        author: author.into(),
        body: body.into(),
        position,
        selection: None,
        pending: false,
    }
}

#[test]
fn hidden_returns_only_diff_rows() {
    let path = active_path();
    let diff = rendered(&path);
    let thread = ReviewThread {
        id: ThreadId("t1".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![thread_comment(
            "c1",
            "alice",
            "hello",
            Some(pos(&path, DiffSide::Right, 1)),
        )],
    };
    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "wip".into(),
        selection: Some(DiffSelection {
            start: pos(&path, DiffSide::Right, 1),
            end: pos(&path, DiffSide::Right, 1),
        }),
        thread_id: None,
    };

    let rows = build_display_rows(&diff, &[thread], &[draft], &path, true);

    assert_eq!(
        rows,
        vec![
            DisplayRow::Diff { row: 0 },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Diff { row: 2 },
        ]
    );
}

#[test]
fn draft_block_appears_under_its_anchor() {
    let path = active_path();
    let diff = rendered(&path);
    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "please fix".into(),
        selection: Some(DiffSelection {
            start: pos(&path, DiffSide::Left, 2),
            end: pos(&path, DiffSide::Left, 2),
        }),
        thread_id: None,
    };

    let rows = build_display_rows(&diff, &[], std::slice::from_ref(&draft), &path, false);

    assert_eq!(
        rows,
        vec![
            DisplayRow::Diff { row: 0 },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Header,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Body,
                text: "please fix".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Footer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Actions,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::ActionsBottom,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Diff { row: 2 },
        ]
    );
}

#[test]
fn multiline_body_expands_between_header_and_footer() {
    let path = active_path();
    let diff = rendered(&path);
    let thread = ReviewThread {
        id: ThreadId("t1".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![thread_comment(
            "c1",
            "alice",
            "line1\nline2\nline3",
            Some(pos(&path, DiffSide::Right, 1)),
        )],
    };

    let rows = build_display_rows(&diff, std::slice::from_ref(&thread), &[], &path, false);

    assert_eq!(
        rows,
        vec![
            DisplayRow::Diff { row: 0 },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Header,
                text: String::new(),
                author: Some("alice".into()),
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: "line1".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: "line2".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: "line3".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Footer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Actions,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::ActionsBottom,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Diff { row: 2 },
        ]
    );
}

#[test]
fn thread_with_two_comments_produces_two_blocks() {
    let path = active_path();
    let diff = rendered(&path);
    // Second comment has no position of its own, so it falls back to the
    // thread's first comment with a position (also row 0).
    let thread = ReviewThread {
        id: ThreadId("t1".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![
            thread_comment("c1", "alice", "first", Some(pos(&path, DiffSide::Right, 1))),
            thread_comment("c2", "bob", "second", None),
        ],
    };

    let rows = build_display_rows(&diff, std::slice::from_ref(&thread), &[], &path, false);

    assert_eq!(
        rows,
        vec![
            DisplayRow::Diff { row: 0 },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Header,
                text: String::new(),
                author: Some("alice".into()),
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: "first".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Footer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Actions,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::ActionsBottom,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Header,
                text: String::new(),
                author: Some("bob".into()),
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Body,
                text: "second".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Footer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Actions,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::ActionsBottom,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 1,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Diff { row: 2 },
        ]
    );
}

#[test]
fn unanchored_comments_group_after_an_orphan_header() {
    let path = active_path();
    let diff = rendered(&path);
    let thread = ReviewThread {
        id: ThreadId("t1".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![thread_comment(
            "c1",
            "alice",
            "stale",
            Some(pos(&path, DiffSide::Right, 999)),
        )],
    };
    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "no selection".into(),
        selection: None,
        thread_id: None,
    };

    let rows = build_display_rows(
        &diff,
        std::slice::from_ref(&thread),
        std::slice::from_ref(&draft),
        &path,
        false,
    );

    assert_eq!(
        rows,
        vec![
            DisplayRow::Diff { row: 0 },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Diff { row: 2 },
            DisplayRow::OrphanHeader,
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Header,
                text: String::new(),
                author: Some("alice".into()),
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: "stale".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Footer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Actions,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::ActionsBottom,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Thread {
                    thread: ThreadId("t1".into()),
                    comment_index: 0,
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Header,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Body,
                text: "no selection".into(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Body,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Footer,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Actions,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::ActionsBottom,
                text: String::new(),
                author: None,
            },
            DisplayRow::Comment {
                entry: CommentEntry::Draft {
                    id: DraftId("d1".into())
                },
                kind: CommentRowKind::Spacer,
                text: String::new(),
                author: None,
            },
        ]
    );
}

#[test]
fn other_files_comments_are_ignored() {
    let path = active_path();
    let elsewhere = other_path();
    let diff = rendered(&path);
    let thread = ReviewThread {
        id: ThreadId("t1".into()),
        path: elsewhere.clone(),
        resolved: false,
        outdated: false,
        comments: vec![thread_comment(
            "c1",
            "alice",
            "wrong file",
            Some(pos(&elsewhere, DiffSide::Right, 1)),
        )],
    };
    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "wrong file draft".into(),
        selection: Some(DiffSelection {
            start: pos(&elsewhere, DiffSide::Right, 1),
            end: pos(&elsewhere, DiffSide::Right, 1),
        }),
        thread_id: None,
    };

    let rows = build_display_rows(
        &diff,
        std::slice::from_ref(&thread),
        std::slice::from_ref(&draft),
        &path,
        false,
    );

    assert_eq!(
        rows,
        vec![
            DisplayRow::Diff { row: 0 },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Diff { row: 2 },
        ]
    );
}

#[test]
fn a_draft_spanning_several_lines_sits_under_the_last_of_them() {
    let path = active_path();
    let diff = rendered(&path);
    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "covers the block".into(),
        selection: Some(DiffSelection {
            start: pos(&path, DiffSide::Left, 1),
            end: pos(&path, DiffSide::Left, 2),
        }),
        thread_id: None,
    };

    let rows = build_display_rows(&diff, &[], std::slice::from_ref(&draft), &path, false);

    let card = rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Comment { .. }))
        .expect("the card rendered");
    let last_diff_above = rows[..card]
        .iter()
        .rev()
        .find_map(|row| match row {
            DisplayRow::Diff { row } => Some(*row),
            _ => None,
        })
        .expect("a diff row above the card");

    assert_eq!(
        last_diff_above, 1,
        "the card belongs under the last selected line, not the first: {rows:?}"
    );
}
