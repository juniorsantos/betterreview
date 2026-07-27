#![allow(dead_code)]

use std::collections::BTreeMap;

use betterreview::{
    app::AppState,
    diff::{ParsedFileDiff, RenderedDiff, RenderedRow, RowBinding, parse_file_patch},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
};
use ratatui::text::Line;
use time::OffsetDateTime;

pub const HEAD: &str = "new-head";
pub const BASE: &str = "base";

pub struct FileSpec {
    path: String,
    patch: PatchAvailability,
    status: FileStatus,
    reviewed: bool,
    cached_lines: Option<usize>,
}

impl FileSpec {
    pub fn new(path: &str, patch: &str) -> Self {
        Self {
            path: path.to_owned(),
            patch: PatchAvailability::Available(patch.to_owned()),
            status: FileStatus::Modified,
            reviewed: false,
            cached_lines: None,
        }
    }

    pub fn status(mut self, status: FileStatus) -> Self {
        self.status = status;
        self
    }

    pub fn reviewed(mut self) -> Self {
        self.reviewed = true;
        self
    }

    pub fn cached_lines(mut self, lines: usize) -> Self {
        self.cached_lines = Some(lines);
        self
    }
}

pub struct Fixture {
    files: Vec<FileSpec>,
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn file(mut self, spec: FileSpec) -> Self {
        self.files.push(spec);
        self
    }

    pub fn build(self) -> AppState {
        let key = ChangeRequestKey {
            provider: ProviderKind::GitHub,
            host: "github.com".into(),
            repository: "owner/repo".into(),
            number: 10,
        };
        let changed: Vec<ChangedFile> = self
            .files
            .iter()
            .enumerate()
            .map(|(index, spec)| ChangedFile {
                path: RepoPath(spec.path.clone()),
                previous_path: None,
                status: spec.status,
                additions: 1,
                deletions: 1,
                patch: spec.patch.clone(),
                base_blob: Some(format!("base-{index}")),
                head_blob: Some(format!("head-{index}")),
                remotely_reviewed: Some(false),
            })
            .collect();
        let progress = changed
            .iter()
            .zip(&self.files)
            .map(|(file, spec)| {
                (
                    file.path.clone(),
                    FileProgress {
                        identity: ContentIdentity {
                            path: file.path.clone(),
                            base_blob: file.base_blob.clone(),
                            head_blob: file.head_blob.clone(),
                        },
                        reviewed: spec.reviewed,
                        reviewed_hunks: Default::default(),
                        sync: ReviewSync::Synced,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let active = changed.first().map(|file| file.path.clone());
        let provider = ProviderSnapshot {
            key: key.clone(),
            title: "Change".into(),
            author: "dev".into(),
            web_url: "https://github.com/owner/repo/pull/10".into(),
            base: CommitOid(BASE.into()),
            head: CommitOid(HEAD.into()),
            files: changed,
            threads: Vec::new(),
            drafts: Vec::new(),
            capabilities: ProviderCapabilities::all_supported(),
        };
        let session = SessionSnapshot {
            schema_version: SESSION_SCHEMA_VERSION,
            key,
            base: CommitOid(BASE.into()),
            head: CommitOid(HEAD.into()),
            active_file: active,
            cursor_row: 0,
            scroll_row: 0,
            files: progress,
            editor: None,
            pending_submit: None,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let mut state = AppState::new(provider, session);
        if let Some(first) = state.provider.files.first().cloned() {
            let parsed = parse(&first);
            state.rendered_diff = Some(render(&parsed));
            state.parsed_diff = Some(parsed);
        }
        for (spec, file) in self.files.iter().zip(state.provider.files.clone()) {
            if let Some(lines) = spec.cached_lines {
                state.file_contexts.insert(
                    file.path.clone(),
                    (1..=lines).map(|line| format!("line {line}")).collect(),
                );
            }
        }
        betterreview::app::refresh_display_rows(&mut state);
        state
    }
}

pub fn parse(file: &ChangedFile) -> ParsedFileDiff {
    parse_file_patch(file, &CommitOid(HEAD.into())).expect(
        "the fixture patch has to parse; a fixture the app could never produce proves nothing",
    )
}

pub fn render(parsed: &ParsedFileDiff) -> RenderedDiff {
    RenderedDiff {
        rows: parsed
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| RenderedRow {
                text: Line::raw(row.raw.clone()),
                binding: RowBinding {
                    row_index: index,
                    left: row.left.clone(),
                    right: row.right.clone(),
                },
            })
            .collect(),
    }
}
