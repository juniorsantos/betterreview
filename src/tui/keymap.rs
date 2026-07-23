use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::AppAction;

#[derive(Debug, Default)]
pub struct KeyMap {
    prefix: Option<char>,
}

impl KeyMap {
    pub fn feed(&mut self, event: KeyEvent) -> Option<AppAction> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        if let Some(prefix) = self.prefix.take() {
            return match (prefix, event.code) {
                (']', KeyCode::Char('f')) => Some(AppAction::NextFile),
                ('[', KeyCode::Char('f')) => Some(AppAction::PreviousFile),
                (']', KeyCode::Char('u')) => Some(AppAction::NextUnreviewed),
                ('[', KeyCode::Char('u')) => Some(AppAction::PreviousUnreviewed),
                _ => key_to_action(event),
            };
        }
        match event.code {
            KeyCode::Char(prefix @ (']' | '[')) => {
                self.prefix = Some(prefix);
                None
            }
            _ => key_to_action(event),
        }
    }
}

pub fn key_to_action(event: KeyEvent) -> Option<AppAction> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let action = match (event.code, event.modifiers) {
        (KeyCode::Tab, KeyModifiers::NONE) => Some(AppAction::FocusNext),
        (KeyCode::BackTab, _) => Some(AppAction::FocusPrevious),
        (KeyCode::Char('h') | KeyCode::Left, KeyModifiers::NONE) => Some(AppAction::FocusPrevious),
        (KeyCode::Char('l') | KeyCode::Right, KeyModifiers::NONE) => Some(AppAction::FocusNext),
        (KeyCode::Char('j') | KeyCode::Down, KeyModifiers::NONE) => Some(AppAction::MoveCursor(1)),
        (KeyCode::Char('k') | KeyCode::Up, KeyModifiers::NONE) => Some(AppAction::MoveCursor(-1)),
        (KeyCode::Char('m'), KeyModifiers::NONE) => Some(AppAction::ToggleReviewed),
        (KeyCode::Char('v'), KeyModifiers::NONE) => Some(AppAction::ToggleSelection),
        (KeyCode::Char('c'), KeyModifiers::NONE) => Some(AppAction::OpenComment),
        (KeyCode::Char('s'), KeyModifiers::NONE) => Some(AppAction::OpenSuggestion),
        (KeyCode::Char('t'), KeyModifiers::NONE) => Some(AppAction::OpenThreads),
        (KeyCode::Char('e'), KeyModifiers::NONE) => Some(AppAction::ToggleFilesPanel),
        (KeyCode::Char('R'), _) => Some(AppAction::OpenSubmit),
        (KeyCode::Char('r'), KeyModifiers::NONE) => Some(AppAction::Refresh),
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(AppAction::Quit),
        (KeyCode::Char('?'), _) => Some(AppAction::ToggleHelp),
        _ => None,
    };
    if event.kind == KeyEventKind::Repeat
        && !matches!(
            action,
            Some(AppAction::MoveCursor(_) | AppAction::FocusNext | AppAction::FocusPrevious)
        )
    {
        None
    } else {
        action
    }
}
