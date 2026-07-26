mod display;
mod effect;
mod event;
mod generated;
mod reducer;
mod runtime;
mod state;

pub use display::{
    CommentEntry, CommentRowKind, DisplayRow, SPLIT_MIN_DIFF_WIDTH, build_display_rows,
    commented_rows, diff_panel_width, display_rows, effective_layout, refresh_display_rows,
    search_matches, sync_terminal_width,
};
pub use effect::{AppEffect, EffectEnvelope, EffectOutcome, EffectResult, RenderedFile};
pub use event::{AppAction, AppEvent, QuitChoice};
pub use generated::is_generated;
pub(crate) use reducer::push_notice;
pub use reducer::update;
pub use runtime::Runtime;
pub use state::{AppFocus, AppState, SubmissionModal};
