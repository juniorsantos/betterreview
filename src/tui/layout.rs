use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::app::AppState;

/// Below this terminal width the files panel drops out of the normal
/// horizontal split and, when focused, draws as a floating overlay over the
/// diff instead (see `render`). Below the breakpoint there is no dedicated
/// files rect for the mouse to hit-test against.
const FILES_OVERLAY_BREAKPOINT: u16 = 80;

/// The content rects `render` draws the Files and Diff panels into, shared
/// with the mouse click handler so a click can be mapped back onto exactly
/// the geometry the frame was last drawn with. `files` is `None` below the
/// overlay breakpoint — the overlay isn't part of the normal split, so there
/// is nothing stable for a click to hit-test against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffColumns {
    pub left: Rect,
    pub divider: u16,
    pub right: Rect,
}

pub(crate) struct ScreenLayout {
    pub files: Option<Rect>,
    pub diff: Rect,
}

/// Splits the frame into its header / spacer / body / status rows — the
/// vertical rhythm every screen row shares.
fn vertical_rows(area: Rect) -> [Rect; 4] {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    [rows[0], rows[1], rows[2], rows[3]]
}

/// The single-line header row (`render`'s repo/PR chip).
pub(crate) fn header_row(area: Rect) -> Rect {
    vertical_rows(area)[0]
}

/// The single-line status row at the bottom of the frame.
pub(crate) fn status_row(area: Rect) -> Rect {
    vertical_rows(area)[3]
}

/// Computes where the Files and Diff panels land for `area` — the same
/// horizontal split `render` performs, extracted so both `render` and the
/// mouse handler compute it identically (the latter from `terminal.size()`,
/// with no access to the frame `render` drew).
pub(crate) fn screen_layout(area: Rect, state: &AppState) -> ScreenLayout {
    let body = vertical_rows(area)[2];
    if state.files_hidden || area.width < FILES_OVERLAY_BREAKPOINT {
        return ScreenLayout {
            files: None,
            diff: body,
        };
    }
    let files_width = if state.files_expanded { 50 } else { 30 };
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(files_width), Constraint::Min(1)])
        .split(body);
    ScreenLayout {
        files: Some(columns[0]),
        diff: columns[1],
    }
}

/// Where the two sides of a side-by-side diff land inside `area`, or `None`
/// when the diff is rendered unified. Computed here rather than inside the
/// widget so the mouse handler can hit-test a click against the same columns
/// the frame was drawn with.
pub fn diff_columns(area: Rect, state: &AppState) -> Option<DiffColumns> {
    if crate::app::effective_layout(state) != crate::domain::DiffLayout::Split
        || crate::app::diff_panel_width(state) < crate::app::SPLIT_MIN_DIFF_WIDTH
    {
        return None;
    }
    let inner = area.width.saturating_sub(4);
    let divider_width = 3u16;
    Some(match state.split_focus {
        Some(crate::tui::SplitSide::Old) => DiffColumns {
            left: Rect::new(area.x, area.y, area.width, area.height),
            divider: area.x + area.width,
            right: Rect::new(area.x + area.width, area.y, 0, area.height),
        },
        Some(crate::tui::SplitSide::New) => DiffColumns {
            left: Rect::new(area.x, area.y, 0, area.height),
            divider: area.x,
            right: Rect::new(area.x, area.y, area.width, area.height),
        },
        None => {
            let column = inner.saturating_sub(divider_width) / 2;
            let divider = area.x + column;
            DiffColumns {
                left: Rect::new(area.x, area.y, column, area.height),
                divider,
                right: Rect::new(divider + divider_width, area.y, column, area.height),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            ChangeRequestKey, CommitOid, ProviderCapabilities, ProviderKind, ProviderSnapshot,
        },
        state::{SESSION_SCHEMA_VERSION, SessionSnapshot},
    };

    fn state_with(files_expanded: bool) -> AppState {
        let key = ChangeRequestKey {
            provider: ProviderKind::GitHub,
            host: "github.com".into(),
            repository: "owner/repo".into(),
            number: 1,
        };
        let provider = ProviderSnapshot {
            key: key.clone(),
            title: String::new(),
            author: String::new(),
            web_url: String::new(),
            base: CommitOid("base".into()),
            head: CommitOid("head".into()),
            files: Vec::new(),
            threads: Vec::new(),
            drafts: Vec::new(),
            capabilities: ProviderCapabilities::all_supported(),
        };
        let session = SessionSnapshot {
            schema_version: SESSION_SCHEMA_VERSION,
            key,
            base: CommitOid("base".into()),
            head: CommitOid("head".into()),
            active_file: None,
            cursor_row: 0,
            scroll_row: 0,
            files: Default::default(),
            editor: None,
            pending_submit: None,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let mut state = AppState::new(provider, session);
        state.files_expanded = files_expanded;
        state
    }

    #[test]
    fn narrow_terminals_have_no_files_rect_and_diff_spans_the_body() {
        let area = Rect::new(0, 0, 60, 20);
        let layout = screen_layout(area, &state_with(false));

        assert!(layout.files.is_none());
        assert_eq!(layout.diff, Rect::new(0, 2, 60, 17));
    }

    #[test]
    fn medium_terminals_split_a_thirty_wide_files_column() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = screen_layout(area, &state_with(false));

        assert_eq!(layout.files, Some(Rect::new(0, 2, 30, 21)));
        assert_eq!(layout.diff, Rect::new(30, 2, 50, 21));
    }

    #[test]
    fn expanded_files_panel_widens_to_fifty_columns() {
        let area = Rect::new(0, 0, 120, 36);
        let layout = screen_layout(area, &state_with(true));

        assert_eq!(layout.files, Some(Rect::new(0, 2, 50, 33)));
        assert_eq!(layout.diff, Rect::new(50, 2, 70, 33));
    }

    #[test]
    fn header_and_status_rows_bookend_the_body() {
        let area = Rect::new(0, 0, 80, 24);

        assert_eq!(header_row(area), Rect::new(0, 0, 80, 1));
        assert_eq!(status_row(area), Rect::new(0, 23, 80, 1));
    }
}
