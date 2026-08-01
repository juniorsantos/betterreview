use std::{collections::BTreeMap, fmt::Write, fs};

use betterreview::{
    app::{AppState, SubmissionModal, refresh_display_rows},
    diff::{RenderedDiff, RenderedRow, RowBinding},
    domain::{
        ChangeRequestKey, ChangeRequestSummary, ChangedFile, CommitOid, DiffPosition,
        DiffSelection, DiffSide, DraftComment, DraftId, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath, ReviewOutcome,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::{
        picker::{PickerItem, PickerState, render as render_picker},
        render as render_review, theme,
    },
};
use ratatui::{
    Frame, Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use time::{Duration, OffsetDateTime};

fn main() {
    capture(
        "assets/picker.svg",
        "betterreview — open reviews",
        140,
        36,
        |frame| {
            render_picker(frame, &picker_state());
        },
    );
    let review = review_state();
    capture(
        "assets/review.svg",
        "betterreview — code review",
        140,
        36,
        |frame| {
            render_review(frame, &review);
        },
    );
    let mut approval = review_state();
    approval.submission_modal = Some(SubmissionModal {
        summary: "Ready to merge — navigation and review status look good.".into(),
        outcome: ReviewOutcome::Approve,
    });
    capture(
        "assets/approve-review.svg",
        "betterreview — approve review",
        140,
        36,
        |frame| render_review(frame, &approval),
    );
}

fn capture(path: &str, title: &str, width: u16, height: u16, draw: impl FnOnce(&mut Frame)) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(draw).unwrap();
    fs::write(path, svg(terminal.backend().buffer(), title, width, height)).unwrap();
}

fn picker_state() -> PickerState {
    let now = OffsetDateTime::now_utc();
    let mut reviewed = summary(
        80,
        "Prepare BetterReview v1.2.0",
        "juniorsantos",
        "feature/v1.2.0",
        now - Duration::minutes(8),
    );
    reviewed.reviewed_head = Some(reviewed.head.clone());
    let mut external = summary(
        74,
        "Improve provider review feedback",
        "alexandre-montgomery",
        "feature/reviewer-status-layout",
        now - Duration::hours(3),
    );
    external.reviewed_head = Some(external.head.clone());
    let mut draft = summary(
        72,
        "Refactor terminal event handling",
        "mariana-de-albuquerque",
        "refactor/terminal-event-dispatch",
        now - Duration::days(1),
    );
    draft.draft = true;
    let items = vec![
        PickerItem {
            summary: reviewed,
            has_session: true,
            current_branch: true,
        },
        PickerItem {
            summary: external,
            has_session: false,
            current_branch: false,
        },
        PickerItem {
            summary: draft,
            has_session: true,
            current_branch: false,
        },
        PickerItem {
            summary: summary(
                69,
                "Add GitLab request changes support",
                "carlos-eduardo-silva",
                "feat/gitlab-request-changes",
                now - Duration::days(2),
            ),
            has_session: false,
            current_branch: false,
        },
    ];
    let mut state = PickerState::new(items, "juniorsantos/betterreview".into());
    state.highlight = 1;
    state
}

fn summary(
    number: u64,
    title: &str,
    author: &str,
    branch: &str,
    updated_at: OffsetDateTime,
) -> ChangeRequestSummary {
    ChangeRequestSummary {
        number,
        title: title.into(),
        author: author.into(),
        source_branch: branch.into(),
        updated_at,
        draft: false,
        web_url: format!("https://github.com/juniorsantos/betterreview/pull/{number}"),
        description: "Review changes without leaving the terminal. The highlighted item is prefetched while you read its description.".into(),
        head: CommitOid(format!("head-{number}")),
        reviewed_head: None,
    }
}

fn review_state() -> AppState {
    let path = RepoPath("src/tui/picker.rs".into());
    let patch = "@@ -548,3 +548,4 @@ fn header_line(columns: Columns) -> Line<'static> {\n     if columns.show_branch {\n-        text.push_str(\"WHEN\");\n+        text.push_str(&pad_cell(\"WHEN\", columns.when_width));\n+        text.push_str(\"STATUS\");\n     }\n";
    let files = vec![
        changed_file(path.clone(), FileStatus::Modified, 2, 1, patch),
        changed_file(
            RepoPath("src/providers/github/mod.rs".into()),
            FileStatus::Modified,
            13,
            0,
            "@@ -1 +1 @@\n-old\n+new\n",
        ),
        changed_file(
            RepoPath("src/providers/gitlab/mod.rs".into()),
            FileStatus::Modified,
            86,
            3,
            "@@ -1 +1 @@\n-old\n+new\n",
        ),
        changed_file(
            RepoPath("tests/picker_render.rs".into()),
            FileStatus::Modified,
            36,
            0,
            "@@ -1 +1 @@\n-old\n+new\n",
        ),
        changed_file(
            RepoPath("README.md".into()),
            FileStatus::Modified,
            18,
            5,
            "@@ -1 +1 @@\n-old\n+new\n",
        ),
    ];
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "juniorsantos/betterreview".into(),
        number: 80,
    };
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: "Prepare BetterReview v1.2.0".into(),
        author: "juniorsantos".into(),
        web_url: "https://github.com/juniorsantos/betterreview/pull/80".into(),
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        files,
        threads: Vec::new(),
        drafts: vec![DraftComment {
            id: DraftId("readme-shot".into()),
            body: "The status column makes reviewed pull requests much easier to scan.".into(),
            selection: Some(DiffSelection {
                start: position(&path, DiffSide::Right, 550),
                end: position(&path, DiffSide::Right, 550),
            }),
            thread_id: None,
        }],
        capabilities: ProviderCapabilities::all_supported(),
    };
    let session = SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key,
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        active_file: Some(path.clone()),
        cursor_row: 3,
        scroll_row: 0,
        files: BTreeMap::from([
            progress(path.clone(), true),
            progress(RepoPath("src/providers/github/mod.rs".into()), true),
            progress(RepoPath("src/providers/gitlab/mod.rs".into()), false),
            progress(RepoPath("tests/picker_render.rs".into()), false),
            progress(RepoPath("README.md".into()), false),
        ]),
        editor: None,
        pending_submit: None,
        updated_at: OffsetDateTime::now_utc(),
    };
    let mut state = AppState::new(provider, session);
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![
            rendered(
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled("if", Style::default().fg(Color::Rgb(0xbe, 0x84, 0xff))),
                    Span::raw(" columns.show_branch {"),
                ]),
                0,
                Some(position(&path, DiffSide::Left, 548)),
                Some(position(&path, DiffSide::Right, 548)),
            ),
            rendered(
                Line::styled(
                    "-        text.push_str(\"WHEN\");",
                    Style::default()
                        .fg(Color::Rgb(0xff, 0xc7, 0xc7))
                        .bg(Color::Rgb(0x5c, 0x21, 0x24)),
                ),
                1,
                Some(position(&path, DiffSide::Left, 549)),
                None,
            ),
            rendered(
                Line::styled(
                    "+        text.push_str(&pad_cell(\"WHEN\", columns.when_width));",
                    Style::default()
                        .fg(Color::Rgb(0xd8, 0xff, 0xdf))
                        .bg(Color::Rgb(0x1c, 0x44, 0x28)),
                ),
                2,
                None,
                Some(position(&path, DiffSide::Right, 549)),
            ),
            rendered(
                Line::styled(
                    "+        text.push_str(\"STATUS\");",
                    Style::default()
                        .fg(Color::Rgb(0xd8, 0xff, 0xdf))
                        .bg(Color::Rgb(0x1c, 0x44, 0x28)),
                ),
                3,
                None,
                Some(position(&path, DiffSide::Right, 550)),
            ),
            rendered(
                Line::raw("    }"),
                4,
                Some(position(&path, DiffSide::Left, 550)),
                Some(position(&path, DiffSide::Right, 551)),
            ),
        ],
    });
    state.terminal_width = 140;
    refresh_display_rows(&mut state);
    state
}

fn changed_file(
    path: RepoPath,
    status: FileStatus,
    additions: u32,
    deletions: u32,
    patch: &str,
) -> ChangedFile {
    ChangedFile {
        path,
        previous_path: None,
        status,
        additions,
        deletions,
        patch: PatchAvailability::Available(patch.into()),
        base_blob: Some("base-blob".into()),
        head_blob: Some("head-blob".into()),
        remotely_reviewed: Some(false),
    }
}

fn progress(path: RepoPath, reviewed: bool) -> (RepoPath, FileProgress) {
    (
        path.clone(),
        FileProgress {
            identity: ContentIdentity {
                path,
                base_blob: Some("base-blob".into()),
                head_blob: Some("head-blob".into()),
            },
            reviewed,
            reviewed_hunks: if reviewed {
                [0].into_iter().collect()
            } else {
                Default::default()
            },
            sync: ReviewSync::Synced,
        },
    )
}

fn rendered(
    text: Line<'static>,
    row_index: usize,
    left: Option<DiffPosition>,
    right: Option<DiffPosition>,
) -> RenderedRow {
    RenderedRow {
        text,
        binding: RowBinding {
            row_index,
            left,
            right,
        },
    }
}

fn position(path: &RepoPath, side: DiffSide, line: u32) -> DiffPosition {
    DiffPosition {
        path: path.clone(),
        side,
        line,
        hunk: 0,
        old_line: None,
        new_line: None,
    }
}

fn svg(buffer: &Buffer, title: &str, width: u16, height: u16) -> String {
    let cell_width = 9.4;
    let cell_height = 19.0;
    let margin = 18.0;
    let chrome = 42.0;
    let screen_width = f64::from(width) * cell_width;
    let screen_height = f64::from(height) * cell_height;
    let total_width = screen_width + margin * 2.0;
    let total_height = screen_height + chrome + margin;
    let mut out = String::new();
    write!(
        out,
        "<svg viewBox=\"0 0 {total_width:.1} {total_height:.1}\" xmlns=\"http://www.w3.org/2000/svg\">\n\
<style>text{{font-family:monospace;font-size:15px;white-space:pre}}</style>\n\
<rect width=\"{total_width:.1}\" height=\"{total_height:.1}\" rx=\"12\" fill=\"#171a21\"/>\n\
<circle cx=\"22\" cy=\"21\" r=\"6\" fill=\"#ff5f57\"/><circle cx=\"42\" cy=\"21\" r=\"6\" fill=\"#febc2e\"/><circle cx=\"62\" cy=\"21\" r=\"6\" fill=\"#28c840\"/>\n\
<text x=\"{:.1}\" y=\"26\" text-anchor=\"middle\" fill=\"#c5c8c6\" font-size=\"15\" font-weight=\"700\">{}</text>\n\
<rect x=\"{margin:.1}\" y=\"{chrome:.1}\" width=\"{screen_width:.1}\" height=\"{screen_height:.1}\" fill=\"#0a0c10\"/>\n",
        total_width / 2.0,
        xml(title),
    )
    .unwrap();

    for y in 0..height {
        let mut x = 0;
        while x < width {
            let (_, background, _) = colors(buffer.cell((x, y)).unwrap());
            let mut end = x + 1;
            while end < width && colors(buffer.cell((end, y)).unwrap()).1 == background {
                end += 1;
            }
            if background != "#0a0c10" {
                writeln!(
                    out,
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{cell_height:.1}\" fill=\"{background}\"/>",
                    margin + f64::from(x) * cell_width,
                    chrome + f64::from(y) * cell_height,
                    f64::from(end - x) * cell_width,
                )
                .unwrap();
            }
            x = end;
        }
    }

    for y in 0..height {
        for x in 0..width {
            let cell = buffer.cell((x, y)).unwrap();
            let symbol = cell.symbol();
            if symbol.trim().is_empty() {
                continue;
            }
            let (foreground, _, style) = colors(cell);
            let weight = if style.add_modifier.contains(Modifier::BOLD) {
                "700"
            } else {
                "400"
            };
            let opacity = if style.add_modifier.contains(Modifier::DIM) {
                "0.65"
            } else {
                "1"
            };
            writeln!(
                out,
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{foreground}\" font-weight=\"{weight}\" opacity=\"{opacity}\">{}</text>",
                margin + f64::from(x) * cell_width,
                chrome + f64::from(y) * cell_height + 15.0,
                xml(symbol),
            )
            .unwrap();
        }
    }
    out.push_str("</svg>\n");
    out
}

fn colors(cell: &ratatui::buffer::Cell) -> (&'static str, &'static str, Style) {
    let style = cell.style();
    let mut foreground = color(style.fg.unwrap_or(theme::FG));
    let mut background = color(style.bg.unwrap_or(theme::BG));
    if style.add_modifier.contains(Modifier::REVERSED) {
        std::mem::swap(&mut foreground, &mut background);
    }
    (foreground, background, style)
}

fn color(color: Color) -> &'static str {
    match color {
        Color::Reset => "#f0f3f6",
        Color::Black => "#000000",
        Color::Red | Color::LightRed => "#ff6a69",
        Color::Green | Color::LightGreen => "#26cd4d",
        Color::Yellow | Color::LightYellow => "#f0b72f",
        Color::Blue | Color::LightBlue => "#0089aa",
        Color::Magenta | Color::LightMagenta => "#c79bff",
        Color::Cyan | Color::LightCyan => "#4cacc3",
        Color::Gray | Color::DarkGray => "#737a83",
        Color::White => "#f0f3f6",
        Color::Rgb(0x0a, 0x0c, 0x10) => "#0a0c10",
        Color::Rgb(0xf0, 0xf3, 0xf6) => "#f0f3f6",
        Color::Rgb(0x73, 0x7a, 0x83) => "#737a83",
        Color::Rgb(0x40, 0x44, 0x4b) => "#40444b",
        Color::Rgb(0x24, 0x27, 0x2b) => "#24272b",
        Color::Rgb(0x00, 0x89, 0xaa) => "#0089aa",
        Color::Rgb(0x4c, 0xac, 0xc3) => "#4cacc3",
        Color::Rgb(0x26, 0xcd, 0x4d) => "#26cd4d",
        Color::Rgb(0xff, 0x6a, 0x69) => "#ff6a69",
        Color::Rgb(0xf0, 0xb7, 0x2f) => "#f0b72f",
        Color::Rgb(0xc7, 0x9b, 0xff) => "#c79bff",
        Color::Rgb(0x2a, 0x33, 0x52) => "#2a3352",
        Color::Rgb(0x27, 0x2b, 0x33) => "#272b33",
        Color::Rgb(0x14, 0x3d, 0x79) => "#143d79",
        Color::Rgb(0x1c, 0x44, 0x28) => "#1c4428",
        Color::Rgb(0x5c, 0x21, 0x24) => "#5c2124",
        Color::Rgb(0xbe, 0x84, 0xff) => "#be84ff",
        Color::Rgb(0xff, 0xc7, 0xc7) => "#ffc7c7",
        Color::Rgb(0xd8, 0xff, 0xdf) => "#d8ffdf",
        Color::Rgb(_, _, _) | Color::Indexed(_) => "#f0f3f6",
    }
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
