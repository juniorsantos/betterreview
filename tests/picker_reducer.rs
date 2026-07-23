use betterreview::{
    domain::{
        ChangeRequestKey, ChangeRequestSummary, CommitOid, ProviderCapabilities, ProviderKind,
        ProviderSnapshot,
    },
    tui::picker::{PickerCommand, PickerEvent, PickerItem, PickerState, update},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn summary(number: u64, branch: &str) -> ChangeRequestSummary {
    ChangeRequestSummary {
        number,
        title: format!("PR {number}"),
        author: "dev".into(),
        source_branch: branch.into(),
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
        draft: false,
        web_url: format!("https://example.com/{number}"),
    }
}

fn item(number: u64, current_branch: bool) -> PickerItem {
    PickerItem {
        summary: summary(number, "feature"),
        has_session: false,
        current_branch,
    }
}

fn key(number: u64) -> ChangeRequestKey {
    ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "example.com".into(),
        repository: "owner/repo".into(),
        number,
    }
}

fn provider_snapshot(number: u64) -> ProviderSnapshot {
    ProviderSnapshot {
        key: key(number),
        title: "Review".into(),
        author: "dev".into(),
        web_url: "https://example.com".into(),
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        files: Vec::new(),
        threads: Vec::new(),
        drafts: Vec::new(),
        capabilities: ProviderCapabilities::all_supported(),
    }
}

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn new_pins_the_current_branch_item_first() {
    let items = vec![item(1, false), item(2, true), item(3, false)];

    let state = PickerState::new(items);

    assert_eq!(state.items[0].summary.number, 2);
    assert_eq!(state.items[1].summary.number, 1);
    assert_eq!(state.items[2].summary.number, 3);
    assert_eq!(state.highlight, 0);
}

#[test]
fn tick_prefetches_the_highlighted_item_once() {
    let mut state = PickerState::new(vec![item(1, true), item(2, false)]);

    let commands = update(&mut state, PickerEvent::Tick);
    assert_eq!(commands, vec![PickerCommand::StartPrefetch(1)]);
    assert_eq!(state.loading, Some(1));

    let commands = update(&mut state, PickerEvent::Tick);
    assert!(commands.is_empty());
}

#[test]
fn moving_highlight_prefetches_the_new_item_on_next_tick() {
    let mut state = PickerState::new(vec![item(1, true), item(2, false)]);

    update(&mut state, PickerEvent::Tick);
    assert_eq!(state.loading, Some(1));

    update(&mut state, PickerEvent::Key(key_event(KeyCode::Char('j'))));
    assert_eq!(state.highlight, 1);

    let commands = update(&mut state, PickerEvent::Tick);
    assert_eq!(commands, vec![PickerCommand::StartPrefetch(2)]);
    assert_eq!(state.loading, Some(2));
}

#[test]
fn enter_with_cache_hit_chooses_immediately() {
    let mut state = PickerState::new(vec![item(1, true)]);
    state.cache.insert(1, provider_snapshot(1));

    let commands = update(&mut state, PickerEvent::Key(key_event(KeyCode::Enter)));

    assert!(commands.is_empty());
    assert_eq!(state.chosen, Some((1, Some(provider_snapshot(1)))));
}

#[test]
fn enter_without_cache_waits_for_the_inflight_load() {
    let mut state = PickerState::new(vec![item(1, true)]);

    let commands = update(&mut state, PickerEvent::Key(key_event(KeyCode::Enter)));
    assert_eq!(commands, vec![PickerCommand::StartPrefetch(1)]);
    assert_eq!(state.waiting, Some(1));
    assert_eq!(state.loading, Some(1));

    let commands = update(
        &mut state,
        PickerEvent::Loaded {
            number: 1,
            result: Ok(provider_snapshot(1)),
        },
    );

    assert!(commands.is_empty());
    assert_eq!(state.chosen, Some((1, Some(provider_snapshot(1)))));
}

#[test]
fn load_error_surfaces_only_when_entering_the_item() {
    let mut state = PickerState::new(vec![item(1, true)]);

    update(&mut state, PickerEvent::Tick);
    assert_eq!(state.loading, Some(1));

    update(
        &mut state,
        PickerEvent::Loaded {
            number: 1,
            result: Err("boom".into()),
        },
    );
    assert!(state.error_banner.is_none());
    assert_eq!(state.errors.get(&1), Some(&"boom".to_string()));
    assert_eq!(state.loading, None);

    let commands = update(&mut state, PickerEvent::Key(key_event(KeyCode::Enter)));
    assert_eq!(commands, vec![PickerCommand::StartPrefetch(1)]);
    assert!(!state.errors.contains_key(&1));
    assert_eq!(state.waiting, Some(1));

    update(
        &mut state,
        PickerEvent::Loaded {
            number: 1,
            result: Err("boom again".into()),
        },
    );
    assert_eq!(state.waiting, None);
    assert_eq!(state.error_banner, Some("boom again".to_string()));
}

#[test]
fn q_quits_and_r_reloads() {
    let mut state = PickerState::new(vec![item(1, true)]);

    update(&mut state, PickerEvent::Key(key_event(KeyCode::Char('q'))));
    assert!(state.quit);

    let mut esc_state = PickerState::new(vec![item(1, true)]);
    update(&mut esc_state, PickerEvent::Key(key_event(KeyCode::Esc)));
    assert!(esc_state.quit);

    let mut reload_state = PickerState::new(vec![item(1, true)]);
    let commands = update(
        &mut reload_state,
        PickerEvent::Key(key_event(KeyCode::Char('r'))),
    );
    assert_eq!(commands, vec![PickerCommand::ReloadList]);
}
