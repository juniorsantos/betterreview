# Terminal Viewport Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make file and diff navigation stop at their boundaries while keeping the selected item visible.

**Architecture:** Keep selection ownership in the reducer and clamp direct file movement there. Add one pure TUI viewport helper, then have both panels derive a bounded visible window from their current selected index and inner height.

**Tech Stack:** Rust 1.88, Ratatui 0.30.2, Cargo integration tests, Insta snapshots

## Global Constraints

- `j` and Down stop at the last item; `k` and Up stop at the first item.
- The file explorer and diff always display their selected item.
- Changing files resets the diff cursor and viewport to the first row.
- Existing shortcuts and session schema remain compatible.
- Do not add explanatory comments to production code.

---

### Task 1: Clamp direct file navigation

**Files:**
- Modify: `src/app/reducer.rs`
- Test: `tests/app_reducer.rs`

**Interfaces:**
- Consumes: `move_index(current: usize, delta: i32, count: usize) -> usize`
- Produces: `navigate_by` that returns no effects when movement is already at a boundary

- [ ] **Step 1: Write the failing boundary test**

```rust
#[test]
fn direct_file_navigation_stops_at_both_boundaries() {
    let mut state = app_with_reviewed_pattern([false; 4]);

    let effects = update(&mut state, AppEvent::Action(AppAction::PreviousFile));
    assert_eq!(state.active_file_index, 0);
    assert!(effects.is_empty());

    state.active_file_index = 3;
    state.session.active_file = Some(RepoPath("src/file_3.rs".into()));
    let effects = update(&mut state, AppEvent::Action(AppAction::NextFile));
    assert_eq!(state.active_file_index, 3);
    assert!(effects.is_empty());
}
```

- [ ] **Step 2: Run it and verify RED**

Run: `cargo test --test app_reducer direct_file_navigation_stops_at_both_boundaries`

Expected: FAIL because previous navigation wraps from index `0` to index `3`.

- [ ] **Step 3: Implement clamped direct navigation**

```rust
fn navigate_by(state: &mut AppState, delta: i32) -> Vec<EffectEnvelope> {
    let count = state.provider.files.len();
    if count == 0 {
        return Vec::new();
    }
    let index = move_index(state.active_file_index, delta, count);
    if index == state.active_file_index {
        return Vec::new();
    }
    activate_file(state, index)
}
```

- [ ] **Step 4: Run reducer tests and verify GREEN**

Run: `cargo test --test app_reducer`

Expected: all reducer tests pass, including cyclic unreviewed search.

### Task 2: Keep panel selections inside the viewport

**Files:**
- Create: `src/tui/viewport.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/widgets/files.rs`
- Modify: `src/tui/widgets/diff.rs`
- Test: `tests/tui_navigation.rs`
- Update: `tests/snapshots/tui_navigation__wide_120x36.snap`
- Update: `tests/snapshots/tui_navigation__medium_80x24.snap`
- Update: `tests/snapshots/tui_navigation__narrow_50x16.snap`

**Interfaces:**
- Produces: `viewport::start(selected: usize, total: usize, visible: usize) -> usize`
- Consumes: active file index, diff cursor row, total item count, and panel inner height

- [ ] **Step 1: Write failing long-content rendering tests**

```rust
#[test]
fn file_panel_scrolls_to_keep_the_active_file_visible() {
    let mut state = app_with_long_content();
    state.focus = AppFocus::Files;
    state.active_file_index = 15;
    let rendered = screen(&state, 80, 12);
    assert!(rendered.contains("src/file_15.rs"));
    assert!(!rendered.contains("src/file_0.rs"));
}

#[test]
fn diff_panel_scrolls_to_keep_the_cursor_visible() {
    let mut state = app_with_long_content();
    state.session.cursor_row = 15;
    let rendered = screen(&state, 80, 12);
    assert!(rendered.contains("diff-row-15"));
    assert!(!rendered.contains("diff-row-00"));
}
```

The `app_with_long_content` fixture extends `app()` to 20 one-line files and replaces `rendered_diff` with 20 `RenderedRow` values named `diff-row-00` through `diff-row-19`.

- [ ] **Step 2: Run them and verify RED**

Run: `cargo test --test tui_navigation scrolls_to_keep -- --nocapture`

Expected: both tests fail because each panel renders from offset zero.

- [ ] **Step 3: Add the bounded viewport helper**

Create `src/tui/viewport.rs`:

```rust
pub(super) fn start(selected: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    selected
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible))
}
```

Add `mod viewport;` to `src/tui/mod.rs`.

- [ ] **Step 4: Apply the derived window to the file explorer**

Import `viewport` and add these iterator operations between `.enumerate()` and `.map(...)` in `files.rs`:

```rust
.skip(viewport::start(
    state.active_file_index,
    state.provider.files.len(),
    area.height.saturating_sub(2) as usize,
))
.take(area.height.saturating_sub(2) as usize)
```

- [ ] **Step 5: Apply the derived offset to the diff**

Import `viewport` in `diff.rs`, then replace the fixed session offset with:

```rust
let visible = area.height.saturating_sub(2) as usize;
let start = viewport::start(state.session.cursor_row, lines.len(), visible);
let scroll = u16::try_from(start).unwrap_or(u16::MAX);
```

Render the paragraph with `.scroll((scroll, 0))`.

- [ ] **Step 6: Run focused tests and update snapshots**

Run: `cargo test --test tui_navigation scrolls_to_keep -- --nocapture`

Expected: both viewport tests pass.

Run: `INSTA_UPDATE=always cargo test --test tui_navigation`

Expected: all TUI navigation tests pass and snapshot changes are limited to the intended viewport behavior.

### Task 3: Verify and publish

**Files:**
- Verify all files changed in Tasks 1 and 2
- Add: `docs/superpowers/plans/2026-07-23-terminal-viewport-navigation.md`

**Interfaces:**
- Consumes: completed reducer and viewport behavior
- Produces: release binary and updated `feature/betterreview` branch

- [ ] **Step 1: Run complete verification**

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
git diff --check
```

Expected: every command exits with status `0`.

- [ ] **Step 2: Validate the real PR session**

Run: `BETTERREVIEW_STATE_DIR=/tmp/betterreview-viewport-navigation-20260723 target/release/betterreview https://github.com/juniorsantos/betterreview/pull/1`

Expected: both selected items remain visible and direct movement stops at both boundaries.

- [ ] **Step 3: Commit and push**

```bash
git add src/tui src/app/reducer.rs tests/app_reducer.rs tests/tui_navigation.rs tests/snapshots docs/superpowers/plans/2026-07-23-terminal-viewport-navigation.md
git commit -m "fix: keep terminal navigation in view"
git push origin feature/betterreview
```
