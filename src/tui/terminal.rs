use std::{io, sync::Arc, time::Duration};

use crossterm::event::{
    Event, EventStream, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use futures_util::StreamExt;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::{
    app::{
        AppAction, AppEvent, AppFocus, AppState, CommentEntry, DisplayRow, QuitChoice, Runtime,
        update,
    },
    domain::ReviewOutcome,
    providers::{DraftBody, NewDraftComment},
};

use super::{
    EditorState, KeyMap,
    layout::screen_layout,
    render, viewport,
    widgets::files::{self, FilesRow},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    Quit,
    BackToPicker,
    Interrupted,
}

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal event failed: {0}")]
    Event(#[source] io::Error),
    #[error("terminal draw failed: {0}")]
    Draw(#[source] io::Error),
}

pub async fn run(
    terminal: &mut ratatui::DefaultTerminal,
    mut app: AppState,
    runtime: Arc<Runtime>,
) -> Result<ExitReason, TuiError> {
    let mut events = EventStream::new();
    let mut keymap = KeyMap::default();
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let (result_tx, mut result_rx) = mpsc::unbounded_channel();

    loop {
        if let Ok(size) = terminal.size() {
            crate::app::sync_terminal_width(&mut app, size.width);
        }
        terminal
            .draw(|frame| render(frame, &app))
            .map_err(TuiError::Draw)?;
        if app.quit_requested {
            return Ok(if app.return_to_picker {
                ExitReason::BackToPicker
            } else {
                ExitReason::Quit
            });
        }

        let event = tokio::select! {
            _ = tick.tick() => Some(AppEvent::Tick),
            result = result_rx.recv() => result.map(|result| AppEvent::EffectFinished(Box::new(result))),
            terminal_event = events.next() => match terminal_event {
                Some(Ok(Event::Key(key))) if is_interrupt(key) => return Ok(ExitReason::Interrupted),
                Some(Ok(Event::Key(key))) => handle_key(&mut app, &mut keymap, key),
                Some(Ok(Event::Mouse(mouse))) => wheel_to_event(mouse.kind).or_else(|| {
                    terminal
                        .size()
                        .ok()
                        .and_then(|size| {
                            click_event(&mut app, Rect::new(0, 0, size.width, size.height), mouse)
                        })
                        .or(Some(AppEvent::Terminal(Event::Mouse(mouse))))
                }),
                Some(Ok(event)) => Some(AppEvent::Terminal(event)),
                Some(Err(error)) => return Err(TuiError::Event(error)),
                None => return Ok(ExitReason::Interrupted),
            },
        };
        let Some(event) = event else {
            continue;
        };
        let effects = update(&mut app, event);
        for effect in effects {
            let runtime = runtime.clone();
            let result_tx = result_tx.clone();
            tokio::spawn(async move {
                let result = runtime.execute(effect).await;
                let _ = result_tx.send(result);
            });
        }
    }
}

pub fn handle_key(app: &mut AppState, keymap: &mut KeyMap, key: KeyEvent) -> Option<AppEvent> {
    if app.help_visible {
        return match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?') => {
                action(AppAction::ToggleHelp)
            }
            _ => None,
        };
    }
    if app.quit_dialog {
        const CHOICES: [QuitChoice; 3] = [
            QuitChoice::KeepSession,
            QuitChoice::DiscardEditor,
            QuitChoice::Cancel,
        ];
        return match key.code {
            KeyCode::Esc => action(AppAction::ConfirmQuit(QuitChoice::Cancel)),
            KeyCode::Char('d') => action(AppAction::ConfirmQuit(QuitChoice::DiscardEditor)),
            KeyCode::Down | KeyCode::Char('j') => {
                app.quit_selected = (app.quit_selected + 1) % CHOICES.len();
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.quit_selected = (app.quit_selected + CHOICES.len() - 1) % CHOICES.len();
                None
            }
            KeyCode::Enter => action(AppAction::ConfirmQuit(CHOICES[app.quit_selected])),
            _ => None,
        };
    }
    if app.delete_dialog.is_some() {
        return match key.code {
            KeyCode::Esc => action(AppAction::ConfirmDeleteChoice(false)),
            KeyCode::Down | KeyCode::Up | KeyCode::Char('j') | KeyCode::Char('k') => {
                app.delete_selected = (app.delete_selected + 1) % 2;
                None
            }
            KeyCode::Enter => action(AppAction::ConfirmDeleteChoice(app.delete_selected == 0)),
            _ => None,
        };
    }
    if app.editor_open {
        return editor_key(app, key);
    }
    if let Some(modal) = &mut app.submission_modal {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => return action(AppAction::CancelSubmit),
            (KeyCode::Enter, KeyModifiers::ALT) => modal.summary.push('\n'),
            (code, modifiers)
                if code == KeyCode::Enter
                    || (code == KeyCode::Char('s')
                        && modifiers.contains(KeyModifiers::CONTROL)) =>
            {
                return action(AppAction::SubmitReview {
                    summary: modal.summary.clone(),
                    outcome: modal.outcome,
                });
            }
            (KeyCode::Tab | KeyCode::Down, _) => modal.outcome = next_outcome(modal.outcome),
            (KeyCode::BackTab | KeyCode::Up, _) => modal.outcome = previous_outcome(modal.outcome),
            (KeyCode::Backspace, _) => {
                modal.summary.pop();
            }
            (KeyCode::Char(value), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                modal.summary.push(value);
            }
            _ => {}
        }
        return None;
    }
    if let Some(input) = &mut app.search_input {
        return match key.code {
            KeyCode::Esc => action(AppAction::CancelSearch),
            KeyCode::Enter => action(AppAction::ConfirmSearch),
            KeyCode::Backspace => {
                input.pop();
                None
            }
            KeyCode::Char(value)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                input.push(value);
                None
            }
            _ => None,
        };
    }
    if app.search_query.is_some() {
        match key.code {
            KeyCode::Esc => return action(AppAction::CancelSearch),
            KeyCode::Char('n') if key.modifiers == KeyModifiers::NONE => {
                return action(AppAction::SearchNext);
            }
            KeyCode::Char('N') => return action(AppAction::SearchPrevious),
            _ => {}
        }
    }
    if app.focus == AppFocus::Diff
        && key.code == KeyCode::Char('/')
        && key.modifiers == KeyModifiers::NONE
    {
        app.search_input = Some(String::new());
        return None;
    }
    if app.focus == AppFocus::Files && key.code == KeyCode::Enter {
        return action(AppAction::ToggleFold);
    }
    if app.focus == AppFocus::Diff {
        if let Some(event) = gap_row_key(app, key) {
            return Some(event);
        }
        if let Some(event) = comment_row_key(app, key) {
            return Some(event);
        }
    }
    keymap.feed(key).map(AppEvent::Action)
}

/// `z` on a `Gap` row expands it instead of folding the active directory —
/// everywhere else it falls through to the regular keymap (`ToggleFold`).
fn gap_row_key(app: &AppState, key: KeyEvent) -> Option<AppEvent> {
    if key.code != KeyCode::Char('z') || key.modifiers != KeyModifiers::NONE {
        return None;
    }
    match app.display_rows.get(app.display_cursor) {
        Some(DisplayRow::Gap { .. }) => action(AppAction::ExpandGap),
        _ => None,
    }
}

/// Resolves `e`/`x`/`r` against the comment entry under the cursor: editing
/// and deleting only make sense on a draft, replying only on a thread. Any
/// other row (or key) falls through so the caller can apply the regular
/// keymap — `e` still expands the files panel and `r` still refreshes.
fn comment_row_key(app: &AppState, key: KeyEvent) -> Option<AppEvent> {
    if key.modifiers != KeyModifiers::NONE {
        return None;
    }
    let entry = match app.display_rows.get(app.display_cursor) {
        Some(DisplayRow::Comment { entry, .. }) => Some(entry),
        _ => None,
    };
    match (key.code, entry) {
        (KeyCode::Char('e'), Some(CommentEntry::Draft { id })) => {
            action(AppAction::EditComment(id.clone()))
        }
        (KeyCode::Char('x'), Some(CommentEntry::Draft { id })) => {
            action(AppAction::DeleteComment(id.clone()))
        }
        (KeyCode::Char('r'), Some(CommentEntry::Thread { thread, .. })) => {
            action(AppAction::ReplyComment(thread.clone()))
        }
        _ => None,
    }
}

fn editor_key(app: &mut AppState, key: KeyEvent) -> Option<AppEvent> {
    let Some(snapshot) = &mut app.session.editor else {
        app.editor_open = false;
        return None;
    };
    if key.code == KeyCode::Esc {
        app.editor_open = false;
        if app.editing_draft.take().is_some() || app.replying_thread.take().is_some() {
            // Neither edit nor reply text belongs to a fresh comment draft;
            // discard it so a later `c` doesn't resurrect it unexpectedly.
            app.session.editor = None;
            app.dirty = true;
        }
        return None;
    }
    let saves = (key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::ALT))
        || (key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL));
    if saves {
        if snapshot.stale {
            app.error_banner = Some("stale editor cannot be submitted".into());
            return None;
        }
        let body = snapshot.lines.join("\n");
        if let Some(id) = app.editing_draft.clone() {
            return action(AppAction::UpdateDraft {
                id,
                body: DraftBody(body),
            });
        }
        if let Some(thread) = app.replying_thread.clone() {
            return action(AppAction::Reply {
                thread,
                body: DraftBody(body),
            });
        }
        let input = NewDraftComment {
            body: DraftBody(if app.editor_suggestion {
                "Suggested change".into()
            } else {
                body.clone()
            }),
            selection: snapshot.selection.clone(),
            suggestion: app.editor_suggestion.then_some(body),
            operation_id: uuid::Uuid::new_v4().to_string(),
        };
        return action(AppAction::CreateDraft(input));
    }
    let mut editor = EditorState {
        lines: snapshot.lines.clone(),
        row: snapshot.cursor_row,
        grapheme_col: snapshot.grapheme_col,
        read_only: snapshot.stale,
    };
    match (key.code, key.modifiers) {
        (KeyCode::Char(value), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            editor.insert_char(value)
        }
        (KeyCode::Enter, modifiers) if modifiers.contains(KeyModifiers::ALT) => {
            editor.insert_text("\n")
        }
        (KeyCode::Backspace, _) => editor.backspace(),
        (KeyCode::Left, _) => editor.move_left(),
        (KeyCode::Right, _) => editor.move_right(),
        _ => return None,
    }
    snapshot.lines = editor.lines;
    snapshot.cursor_row = editor.row;
    snapshot.grapheme_col = editor.grapheme_col;
    app.dirty = true;
    None
}

fn action(action: AppAction) -> Option<AppEvent> {
    Some(AppEvent::Action(action))
}

/// Translates a mouse wheel notch into the same cursor move a few `j`/`k`
/// presses would produce — the wheel acts on whichever panel is focused,
/// exactly like the keyboard, with no hit-testing at this level. Any other
/// mouse event kind (clicks, drags, moves) is not ours to interpret, so
/// callers fall back to forwarding it as `AppEvent::Terminal`.
fn wheel_to_event(kind: MouseEventKind) -> Option<AppEvent> {
    match kind {
        MouseEventKind::ScrollDown => Some(AppEvent::Action(AppAction::MoveCursor(3))),
        MouseEventKind::ScrollUp => Some(AppEvent::Action(AppAction::MoveCursor(-3))),
        _ => None,
    }
}

/// Handles a left mouse-button press by hit-testing it against the same
/// layout `render` last drew for `terminal_size` (recomputed here via
/// `layout::screen_layout`, since the handler has no access to the frame
/// `render` drew), focusing whichever panel it landed in and translating it
/// into the same action a key press would produce. Every other mouse event
/// kind, and a click landing outside both panels, is not ours to interpret —
/// the caller falls back to forwarding it as `AppEvent::Terminal`.
fn click_event(app: &mut AppState, terminal_size: Rect, mouse: MouseEvent) -> Option<AppEvent> {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return None;
    }
    let layout = screen_layout(terminal_size, app);
    let point = (mouse.column, mouse.row);
    if let Some(files_rect) = layout.files
        && contains(files_rect, point)
    {
        app.focus = AppFocus::Files;
        return files_click(app, files_rect, mouse.row).map(AppEvent::Action);
    }
    if contains(layout.diff, point) {
        app.focus = AppFocus::Diff;
        return diff_click(app, layout.diff, mouse.row).map(AppEvent::Action);
    }
    None
}

fn contains(rect: Rect, point: (u16, u16)) -> bool {
    let (x, y) = point;
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

/// Maps a click's row inside the files panel — one row of border above the
/// content — through `visible_rows`, the same windowed row list the widget
/// rendered, into the action selecting that file or toggling that
/// directory's fold would dispatch from the keyboard.
fn files_click(app: &AppState, rect: Rect, mouse_row: u16) -> Option<AppAction> {
    let content_row = mouse_row.checked_sub(rect.y + 1)?;
    match files::visible_rows(app, rect.height).get(content_row as usize)? {
        FilesRow::File { index, .. } => Some(AppAction::ActivateFile(*index)),
        FilesRow::Directory { path, .. } => Some(AppAction::ToggleFoldDir((*path).to_owned())),
    }
}

/// Maps a click's row inside the diff panel to a display row index, combining
/// the same border offset `files_click` uses with the viewport scroll offset
/// the diff widget's `Paragraph` was drawn with.
fn diff_click(app: &AppState, rect: Rect, mouse_row: u16) -> Option<AppAction> {
    let content_row = mouse_row.checked_sub(rect.y + 1)?;
    let visible = rect.height.saturating_sub(2) as usize;
    let start = viewport::start(app.display_cursor, app.display_rows.len(), visible);
    Some(AppAction::JumpToDisplayRow(content_row as usize + start))
}

fn is_interrupt(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn next_outcome(outcome: ReviewOutcome) -> ReviewOutcome {
    match outcome {
        ReviewOutcome::Comment => ReviewOutcome::Approve,
        ReviewOutcome::Approve => ReviewOutcome::RequestChanges,
        ReviewOutcome::RequestChanges => ReviewOutcome::Comment,
    }
}

fn previous_outcome(outcome: ReviewOutcome) -> ReviewOutcome {
    match outcome {
        ReviewOutcome::Comment => ReviewOutcome::RequestChanges,
        ReviewOutcome::Approve => ReviewOutcome::Comment,
        ReviewOutcome::RequestChanges => ReviewOutcome::Approve,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChangeRequestKey, ChangedFile, CommitOid, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
    };
    use crate::state::{SESSION_SCHEMA_VERSION, SessionSnapshot};

    fn changed_file(path: &str) -> ChangedFile {
        ChangedFile {
            path: RepoPath(path.into()),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            patch: PatchAvailability::Available("@@ -1 +1 @@\n-old\n+new\n".into()),
            base_blob: None,
            head_blob: None,
            remotely_reviewed: Some(false),
        }
    }

    /// Two directories (`a/`, `b/`) of three files, wide enough for the
    /// files column to appear in `screen_layout`, with three synthetic diff
    /// rows so a diff-panel click has something to land on.
    fn state_with_two_directories() -> AppState {
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
            files: ["a/one.rs", "a/two.rs", "b/three.rs"]
                .into_iter()
                .map(changed_file)
                .collect(),
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
        state.display_rows = vec![
            DisplayRow::Diff { row: 0 },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Diff { row: 2 },
        ];
        state
    }

    fn left_click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// 100x30, giving `screen_layout` a files column at `(0, 2, 30, 27)` and
    /// a diff panel at `(30, 2, 70, 27)` — see the row/column math in
    /// `layout::tests`.
    const TERMINAL_SIZE: Rect = Rect::new(0, 0, 100, 30);

    #[test]
    fn clicking_a_file_row_focuses_files_and_activates_it() {
        let mut app = state_with_two_directories();

        // Files content starts at y = 3 (row 2 body top + 1 border); row 4
        // is "a/one.rs" (index 0): Directory("a") then File(0).
        let event = click_event(&mut app, TERMINAL_SIZE, left_click(5, 4));

        assert_eq!(app.focus, AppFocus::Files);
        assert!(matches!(
            event,
            Some(AppEvent::Action(AppAction::ActivateFile(0)))
        ));
    }

    #[test]
    fn clicking_a_directory_row_toggles_its_fold() {
        let mut app = state_with_two_directories();

        // Row 6 is the "b" directory header (a/, a/one.rs, a/two.rs, b/).
        let event = click_event(&mut app, TERMINAL_SIZE, left_click(5, 6));

        assert!(matches!(
            event,
            Some(AppEvent::Action(AppAction::ToggleFoldDir(dir))) if dir == "b"
        ));
    }

    #[test]
    fn clicking_the_diff_panel_focuses_diff_and_jumps_to_the_row() {
        let mut app = state_with_two_directories();

        // Diff content starts at y = 3; row 4 is display row 1.
        let event = click_event(&mut app, TERMINAL_SIZE, left_click(50, 4));

        assert_eq!(app.focus, AppFocus::Diff);
        assert!(matches!(
            event,
            Some(AppEvent::Action(AppAction::JumpToDisplayRow(1)))
        ));
    }

    #[test]
    fn clicking_outside_both_panels_is_ignored() {
        let mut app = state_with_two_directories();

        let event = click_event(&mut app, TERMINAL_SIZE, left_click(5, 0));

        assert!(event.is_none());
    }

    #[test]
    fn a_right_click_is_not_translated() {
        let mut app = state_with_two_directories();
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 5,
            row: 4,
            modifiers: KeyModifiers::NONE,
        };

        assert!(click_event(&mut app, TERMINAL_SIZE, mouse).is_none());
    }

    #[test]
    fn scroll_down_moves_the_cursor_forward_by_three() {
        assert!(matches!(
            wheel_to_event(MouseEventKind::ScrollDown),
            Some(AppEvent::Action(AppAction::MoveCursor(3)))
        ));
    }

    #[test]
    fn scroll_up_moves_the_cursor_backward_by_three() {
        assert!(matches!(
            wheel_to_event(MouseEventKind::ScrollUp),
            Some(AppEvent::Action(AppAction::MoveCursor(-3)))
        ));
    }

    #[test]
    fn other_mouse_kinds_are_not_translated() {
        assert!(wheel_to_event(MouseEventKind::Moved).is_none());
        assert!(
            wheel_to_event(MouseEventKind::Down(crossterm::event::MouseButton::Left)).is_none()
        );
    }
}
