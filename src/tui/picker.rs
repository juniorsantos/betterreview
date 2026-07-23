use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::domain::{ChangeRequestSummary, ProviderSnapshot};

/// One row of the review picker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    pub summary: ChangeRequestSummary,
    pub has_session: bool,
    pub current_branch: bool,
}

/// Pure state for the review picker screen.
#[derive(Debug)]
pub struct PickerState {
    pub items: Vec<PickerItem>,
    pub highlight: usize,
    pub cache: BTreeMap<u64, ProviderSnapshot>,
    pub errors: BTreeMap<u64, String>,
    pub loading: Option<u64>,
    pub waiting: Option<u64>,
    pub error_banner: Option<String>,
    pub quit: bool,
    pub chosen: Option<(u64, Option<ProviderSnapshot>)>,
}

// The `Loaded` variant is intentionally not boxed: this is the public event
// shape mandated by the picker spec and consumed by later tasks as-is.
#[allow(clippy::large_enum_variant)]
pub enum PickerEvent {
    Key(KeyEvent),
    Tick,
    Loaded {
        number: u64,
        result: Result<ProviderSnapshot, String>,
    },
    ListReloaded {
        items: Vec<PickerItem>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerCommand {
    StartPrefetch(u64),
    ReloadList,
}

/// Moves the item flagged as the current branch to the front of the list,
/// keeping the relative order of the remaining items unchanged.
pub fn pin_current_branch(items: &mut Vec<PickerItem>) {
    if let Some(index) = items.iter().position(|item| item.current_branch)
        && index != 0
    {
        let pinned = items.remove(index);
        items.insert(0, pinned);
    }
}

impl PickerState {
    pub fn new(mut items: Vec<PickerItem>) -> Self {
        pin_current_branch(&mut items);
        Self {
            items,
            highlight: 0,
            cache: BTreeMap::new(),
            errors: BTreeMap::new(),
            loading: None,
            waiting: None,
            error_banner: None,
            quit: false,
            chosen: None,
        }
    }
}

pub fn update(state: &mut PickerState, event: PickerEvent) -> Vec<PickerCommand> {
    match event {
        PickerEvent::Key(key) => key_update(state, key),
        PickerEvent::Tick => tick_update(state),
        PickerEvent::Loaded { number, result } => loaded_update(state, number, result),
        PickerEvent::ListReloaded { items } => {
            reload_update(state, items);
            Vec::new()
        }
    }
}

fn key_update(state: &mut PickerState, key: KeyEvent) -> Vec<PickerCommand> {
    if key.kind == KeyEventKind::Release {
        return Vec::new();
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.quit = true;
            Vec::new()
        }
        KeyCode::Char('r') => vec![PickerCommand::ReloadList],
        KeyCode::Char('j') | KeyCode::Down => {
            state.highlight = (state.highlight + 1).min(state.items.len().saturating_sub(1));
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.highlight = state.highlight.saturating_sub(1);
            Vec::new()
        }
        KeyCode::Enter => enter_update(state),
        _ => Vec::new(),
    }
}

fn enter_update(state: &mut PickerState) -> Vec<PickerCommand> {
    let Some(item) = state.items.get(state.highlight) else {
        return Vec::new();
    };
    let number = item.summary.number;
    if let Some(snapshot) = state.cache.get(&number) {
        state.chosen = Some((number, Some(snapshot.clone())));
        return Vec::new();
    }
    state.errors.remove(&number);
    state.waiting = Some(number);
    state.error_banner = None;
    if state.loading != Some(number) {
        state.loading = Some(number);
        vec![PickerCommand::StartPrefetch(number)]
    } else {
        Vec::new()
    }
}

fn tick_update(state: &mut PickerState) -> Vec<PickerCommand> {
    let Some(item) = state.items.get(state.highlight) else {
        return Vec::new();
    };
    let target = item.summary.number;
    if !state.cache.contains_key(&target)
        && !state.errors.contains_key(&target)
        && state.loading != Some(target)
    {
        state.loading = Some(target);
        vec![PickerCommand::StartPrefetch(target)]
    } else {
        Vec::new()
    }
}

fn loaded_update(
    state: &mut PickerState,
    number: u64,
    result: Result<ProviderSnapshot, String>,
) -> Vec<PickerCommand> {
    if state.loading == Some(number) {
        state.loading = None;
    }
    match result {
        Ok(snapshot) => {
            state.cache.insert(number, snapshot.clone());
            if state.waiting == Some(number) {
                state.chosen = Some((number, Some(snapshot)));
            }
        }
        Err(message) => {
            state.errors.insert(number, message.clone());
            if state.waiting == Some(number) {
                state.waiting = None;
                state.error_banner = Some(message);
            }
        }
    }
    Vec::new()
}

fn reload_update(state: &mut PickerState, mut items: Vec<PickerItem>) {
    pin_current_branch(&mut items);
    state.items = items;
    state.highlight = state.highlight.min(state.items.len().saturating_sub(1));
}
