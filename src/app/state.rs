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
    pub thread_panel_open: bool,
    pub submission_modal: Option<SubmissionModal>,
    pub notices: Vec<String>,
    pub error_banner: Option<String>,
    pub busy_operations: BTreeSet<u64>,
    pub next_request_id: u64,
    pub terminal_width: u16,
    pub dirty: bool,
    pub quit_requested: bool,
    pub help_visible: bool,
}

impl AppState {
    pub fn new(provider: ProviderSnapshot, session: SessionSnapshot) -> Self {
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
            thread_panel_open: false,
            submission_modal: None,
            notices: Vec::new(),
            error_banner: None,
            busy_operations: BTreeSet::new(),
            next_request_id: 1,
            terminal_width: 100,
            dirty: false,
            quit_requested: false,
            help_visible: false,
        }
    }
}
