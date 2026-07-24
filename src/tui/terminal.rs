use std::{io, sync::Arc, time::Duration};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::{
    app::{
        AppAction, AppEvent, AppFocus, AppState, CommentEntry, DisplayRow, QuitChoice, Runtime,
        update,
    },
    domain::ReviewOutcome,
    providers::{DraftBody, NewDraftComment},
};

use super::{EditorState, KeyMap, render};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    Quit,
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
        terminal
            .draw(|frame| render(frame, &app))
            .map_err(TuiError::Draw)?;
        if app.quit_requested {
            return Ok(ExitReason::Quit);
        }

        let event = tokio::select! {
            _ = tick.tick() => Some(AppEvent::Tick),
            result = result_rx.recv() => result.map(|result| AppEvent::EffectFinished(Box::new(result))),
            terminal_event = events.next() => match terminal_event {
                Some(Ok(Event::Key(key))) if is_interrupt(key) => return Ok(ExitReason::Interrupted),
                Some(Ok(Event::Key(key))) => handle_key(&mut app, &mut keymap, key),
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
            (KeyCode::Tab, _) => modal.selected_field = (modal.selected_field + 1) % 2,
            (KeyCode::BackTab, _) => modal.selected_field = (modal.selected_field + 1) % 2,
            (KeyCode::Up | KeyCode::Left, _) if modal.selected_field == 1 => {
                modal.outcome = previous_outcome(modal.outcome);
            }
            (KeyCode::Down | KeyCode::Right, _) if modal.selected_field == 1 => {
                modal.outcome = next_outcome(modal.outcome);
            }
            (KeyCode::Backspace, _) if modal.selected_field == 0 => {
                modal.summary.pop();
            }
            (KeyCode::Char(value), KeyModifiers::NONE | KeyModifiers::SHIFT)
                if modal.selected_field == 0 =>
            {
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
