use std::collections::BTreeSet;

use betterreview::{
    domain::{
        ChangeRequestKey, ChangeRequestSummary, CommitOid, ProviderCapabilities, ProviderKind,
        ProviderSnapshot,
    },
    tui::picker::{PickerCommand, PickerEvent, PickerItem, PickerState, mark_items, update},
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

    let state = PickerState::new(items, "owner/repo".into());

    assert_eq!(state.items[0].summary.number, 2);
    assert_eq!(state.items[1].summary.number, 1);
    assert_eq!(state.items[2].summary.number, 3);
    assert_eq!(state.highlight, 0);
}

#[test]
fn tick_prefetches_the_highlighted_item_once() {
    let mut state = PickerState::new(vec![item(1, true), item(2, false)], "owner/repo".into());

    let commands = update(&mut state, PickerEvent::Tick);
    assert_eq!(commands, vec![PickerCommand::StartPrefetch(1)]);
    assert_eq!(state.loading, Some(1));

    let commands = update(&mut state, PickerEvent::Tick);
    assert!(commands.is_empty());
}

#[test]
fn moving_highlight_prefetches_the_new_item_on_next_tick() {
    let mut state = PickerState::new(vec![item(1, true), item(2, false)], "owner/repo".into());

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
    let mut state = PickerState::new(vec![item(1, true)], "owner/repo".into());
    state.cache.insert(1, provider_snapshot(1));

    let commands = update(&mut state, PickerEvent::Key(key_event(KeyCode::Enter)));

    assert!(commands.is_empty());
    assert_eq!(state.chosen, Some((1, Some(provider_snapshot(1)))));
}

#[test]
fn enter_without_cache_waits_for_the_inflight_load() {
    let mut state = PickerState::new(vec![item(1, true)], "owner/repo".into());

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
fn moving_the_highlight_cancels_a_pending_enter() {
    let mut state = PickerState::new(vec![item(1, true), item(2, false)], "owner/repo".into());

    let commands = update(&mut state, PickerEvent::Key(key_event(KeyCode::Enter)));
    assert_eq!(commands, vec![PickerCommand::StartPrefetch(1)]);
    assert_eq!(state.waiting, Some(1));

    update(&mut state, PickerEvent::Key(key_event(KeyCode::Char('j'))));
    assert_eq!(state.highlight, 1);
    assert_eq!(state.waiting, None);

    let commands = update(
        &mut state,
        PickerEvent::Loaded {
            number: 1,
            result: Ok(provider_snapshot(1)),
        },
    );

    assert!(commands.is_empty());
    assert!(state.chosen.is_none());
    assert_eq!(state.cache.get(&1), Some(&provider_snapshot(1)));
}

#[test]
fn load_error_surfaces_only_when_entering_the_item() {
    let mut state = PickerState::new(vec![item(1, true)], "owner/repo".into());

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
    let mut state = PickerState::new(vec![item(1, true)], "owner/repo".into());

    update(&mut state, PickerEvent::Key(key_event(KeyCode::Char('q'))));
    assert!(state.quit);

    let mut esc_state = PickerState::new(vec![item(1, true)], "owner/repo".into());
    update(&mut esc_state, PickerEvent::Key(key_event(KeyCode::Esc)));
    assert!(esc_state.quit);

    let mut reload_state = PickerState::new(vec![item(1, true)], "owner/repo".into());
    let commands = update(
        &mut reload_state,
        PickerEvent::Key(key_event(KeyCode::Char('r'))),
    );
    assert_eq!(commands, vec![PickerCommand::ReloadList]);
}

#[test]
fn mark_items_flags_current_branch_and_session() {
    let list = vec![
        summary(1, "feature"),
        summary(2, "main"),
        summary(3, "feature"),
    ];
    let mut sessions = BTreeSet::new();
    sessions.insert(2);

    let items = mark_items(list, Some("feature"), &sessions);

    assert_eq!(items.len(), 3);
    assert!(items[0].current_branch);
    assert!(!items[0].has_session);
    assert!(!items[1].current_branch);
    assert!(items[1].has_session);
    assert!(items[2].current_branch);
    assert!(!items[2].has_session);
}

#[test]
fn mark_items_marks_nothing_when_branch_is_none() {
    let list = vec![summary(1, "feature")];
    let sessions = BTreeSet::new();

    let items = mark_items(list, None, &sessions);

    assert!(!items[0].current_branch);
    assert!(!items[0].has_session);
}

#[test]
fn reload_clears_stale_errors_so_the_item_can_reprefetch() {
    let mut state = PickerState::new(vec![item(1, true)], "owner/repo".into());

    update(&mut state, PickerEvent::Tick);
    update(
        &mut state,
        PickerEvent::Loaded {
            number: 1,
            result: Err("boom".into()),
        },
    );
    assert_eq!(state.errors.get(&1), Some(&"boom".to_string()));

    update(
        &mut state,
        PickerEvent::ListReloaded {
            items: vec![item(1, true)],
        },
    );

    let commands = update(&mut state, PickerEvent::Tick);
    assert_eq!(commands, vec![PickerCommand::StartPrefetch(1)]);
}

#[test]
fn reload_keeps_the_highlight_on_the_same_review_by_number() {
    let mut state = PickerState::new(
        vec![item(5, false), item(6, false), item(7, false)],
        "owner/repo".into(),
    );
    state.highlight = 2;
    assert_eq!(state.items[state.highlight].summary.number, 7);

    update(
        &mut state,
        PickerEvent::ListReloaded {
            items: vec![item(5, false), item(7, false), item(6, false)],
        },
    );

    assert_eq!(state.items[state.highlight].summary.number, 7);
    assert_eq!(state.highlight, 1);
}

#[test]
fn reload_falls_back_to_the_first_item_when_the_highlighted_review_is_gone() {
    let mut state = PickerState::new(
        vec![item(5, false), item(6, false), item(7, false)],
        "owner/repo".into(),
    );
    state.highlight = 2;
    assert_eq!(state.items[state.highlight].summary.number, 7);

    update(
        &mut state,
        PickerEvent::ListReloaded {
            items: vec![item(8, false), item(9, false)],
        },
    );

    assert_eq!(state.highlight, 0);
}

#[test]
fn list_failed_sets_the_error_banner() {
    let mut state = PickerState::new(vec![item(1, true)], "owner/repo".into());

    let commands = update(&mut state, PickerEvent::ListFailed("network down".into()));

    assert!(commands.is_empty());
    assert_eq!(state.error_banner, Some("network down".to_string()));
}
