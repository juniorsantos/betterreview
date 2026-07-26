use std::collections::{BTreeMap, BTreeSet};

use crate::{
    diff::{ParsedFileDiff, RenderedDiff},
    domain::{DiffLayout, DraftId, ProviderSnapshot, RepoPath, ReviewOutcome, ThreadId},
    state::SessionSnapshot,
};

use super::DisplayRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppFocus {
    Files,
    Diff,
    Threads,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionModal {
    pub summary: String,
    pub outcome: ReviewOutcome,
}

pub struct AppState {
    pub provider: ProviderSnapshot,
    pub session: SessionSnapshot,
    pub parsed_diff: Option<ParsedFileDiff>,
    pub rendered_diff: Option<RenderedDiff>,
    pub focus: AppFocus,
    pub active_file_index: usize,
    pub selection_anchor: Option<usize>,
    pub editor_open: bool,
    pub editor_suggestion: bool,
    pub thread_panel_open: bool,
    pub submission_modal: Option<SubmissionModal>,
    pub notices: Vec<String>,
    pub notice_ttl: u8,
    pub error_banner: Option<String>,
    pub busy_operations: BTreeSet<u64>,
    pub next_request_id: u64,
    pub terminal_width: u16,
    pub dirty: bool,
    pub quit_requested: bool,
    pub return_to_picker: bool,
    pub quit_dialog: bool,
    pub quit_selected: usize,
    pub help_visible: bool,
    pub files_expanded: bool,
    pub files_hidden: bool,
    pub collapsed_dirs: BTreeSet<String>,
    pub display_cursor: usize,
    pub comments_hidden: bool,
    pub enter_file_at_end: bool,
    pub pending_labels: BTreeMap<u64, &'static str>,
    pub spinner_frame: usize,
    pub display_rows: Vec<DisplayRow>,
    pub editing_draft: Option<DraftId>,
    pub replying_thread: Option<ThreadId>,
    pub delete_dialog: Option<DraftId>,
    pub delete_selected: usize,
    /// The in-progress query while typing after `/`; `None` once confirmed
    /// (`Enter`) or canceled (`Esc`). Not persisted.
    pub search_input: Option<String>,
    /// The confirmed query driving `n`/`N` navigation and the status bar's
    /// active-search display; `None` when no search is active. Not persisted.
    pub search_query: Option<String>,
    /// File contents at the head revision, cached per path once fetched via
    /// `LoadFileContext` so a gap can be expanded instantly the next time
    /// it's toggled. Lines are split on `\n`, indexed from 0 (new-file line
    /// `n` is `file_contexts[path][n - 1]`).
    pub file_contexts: BTreeMap<RepoPath, Vec<String>>,
    /// Gap keys — the new-file line number right after which the gap sits —
    /// that are currently expanded into `Context` rows for the active file.
    /// Cleared whenever the active file changes.
    pub expanded_gaps: BTreeSet<u32>,
    /// The gap key awaiting its `LoadFileContext` result, so the response can
    /// be folded straight into `expanded_gaps` once the fetch completes.
    pub pending_gap: Option<u32>,
    pub hunk_totals: BTreeMap<RepoPath, u32>,
    pub diff_layout: DiffLayout,
    pub split_focus: Option<crate::tui::SplitSide>,
    pub wrap_lines: bool,
    pub tab_width: usize,
    pub flagged_files: std::collections::BTreeSet<crate::domain::RepoPath>,
}

impl AppState {
    pub fn hunk_total(&self, path: &RepoPath) -> u32 {
        self.hunk_totals.get(path).copied().unwrap_or(0)
    }

    pub fn active_hunk_total(&self) -> u32 {
        self.provider
            .files
            .get(self.active_file_index)
            .map(|file| self.hunk_total(&file.path))
            .unwrap_or(0)
    }

    pub fn refresh_hunk_totals(&mut self) {
        self.hunk_totals = self
            .provider
            .files
            .iter()
            .map(|file| (file.path.clone(), crate::diff::count_hunks(file)))
            .collect();
        self.flagged_files = self
            .provider
            .files
            .iter()
            .filter(|file| match &file.patch {
                crate::domain::PatchAvailability::Available(patch) => {
                    crate::diff::has_confusables(patch)
                }
                _ => false,
            })
            .map(|file| file.path.clone())
            .collect();
    }

    pub fn new(provider: ProviderSnapshot, session: SessionSnapshot) -> Self {
        let editor_open = session.editor.is_some();
        let active_file_index = session
            .active_file
            .as_ref()
            .and_then(|path| provider.files.iter().position(|file| &file.path == path))
            .unwrap_or(0);
        let mut state = Self {
            provider,
            session,
            parsed_diff: None,
            rendered_diff: None,
            focus: AppFocus::Diff,
            active_file_index,
            selection_anchor: None,
            editor_open,
            editor_suggestion: false,
            thread_panel_open: false,
            submission_modal: None,
            notices: Vec::new(),
            notice_ttl: 0,
            error_banner: None,
            busy_operations: BTreeSet::new(),
            next_request_id: 1,
            terminal_width: 100,
            dirty: false,
            quit_requested: false,
            return_to_picker: false,
            quit_dialog: false,
            quit_selected: 0,
            help_visible: false,
            files_expanded: false,
            files_hidden: false,
            collapsed_dirs: BTreeSet::new(),
            display_cursor: 0,
            comments_hidden: false,
            enter_file_at_end: false,
            pending_labels: BTreeMap::new(),
            spinner_frame: 0,
            display_rows: Vec::new(),
            editing_draft: None,
            replying_thread: None,
            delete_dialog: None,
            delete_selected: 0,
            search_input: None,
            search_query: None,
            file_contexts: BTreeMap::new(),
            expanded_gaps: BTreeSet::new(),
            pending_gap: None,
            hunk_totals: BTreeMap::new(),
            diff_layout: DiffLayout::default(),
            split_focus: None,
            wrap_lines: false,
            tab_width: 4,
            flagged_files: Default::default(),
        };
        state.refresh_hunk_totals();
        state
    }
}
