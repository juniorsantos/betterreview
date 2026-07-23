use std::collections::BTreeSet;

use crate::{
    diff::{ParsedFileDiff, RenderedDiff},
    domain::{ProviderSnapshot, ReviewOutcome},
    state::SessionSnapshot,
};

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
    pub selected_field: usize,
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
    pub error_banner: Option<String>,
    pub busy_operations: BTreeSet<u64>,
    pub next_request_id: u64,
    pub terminal_width: u16,
    pub dirty: bool,
    pub quit_requested: bool,
    pub quit_dialog: bool,
    pub quit_selected: usize,
    pub help_visible: bool,
    pub files_expanded: bool,
    pub collapsed_dirs: BTreeSet<String>,
}

impl AppState {
    pub fn new(provider: ProviderSnapshot, session: SessionSnapshot) -> Self {
        let editor_open = session.editor.is_some();
        let active_file_index = session
            .active_file
            .as_ref()
            .and_then(|path| provider.files.iter().position(|file| &file.path == path))
            .unwrap_or(0);
        Self {
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
            error_banner: None,
            busy_operations: BTreeSet::new(),
            next_request_id: 1,
            terminal_width: 100,
            dirty: false,
            quit_requested: false,
            quit_dialog: false,
            quit_selected: 0,
            help_visible: false,
            files_expanded: false,
            collapsed_dirs: BTreeSet::new(),
        }
    }
}
