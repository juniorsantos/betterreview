# Inline comments in the diff — design

Date: 2026-07-23
Status: approved direction (interaction model chosen by the user); spec pending user review

## Problem

Creating a comment marks the file in the Files panel, but the comment itself is
invisible in the diff. There is also no way to edit or delete a draft from the
TUI (providers and reducer actions already exist), and saving gives no
feedback while the provider call is in flight.

## Decisions (validated with the user)

- GitHub-style inline boxes under the anchored line, always expanded.
- The cursor navigates through comment rows too (chosen over line-scoped
  commands); `e` edit own draft, `x` delete with confirmation, `r` reply.
- Saving/editing/deleting shows an animated "saving" feedback in the status
  bar until the provider confirms.

## Architecture

### Display rows (core refactor)

New pure layer in `src/app/` (module `display.rs`):

```rust
pub enum DisplayRow {
    Diff { row: usize },                       // index into rendered_diff.rows
    Comment { anchor_row: Option<usize>, entry: CommentEntry },
}
pub enum CommentEntry {
    Draft { id: DraftId },
    Thread { thread: ThreadId, comment_index: usize },
}
pub fn build_display_rows(state: &AppState) -> Vec<DisplayRow>;
```

- Built from `rendered_diff` + `provider.threads`/`provider.drafts` filtered to
  the active file. A comment anchors to the first rendered row whose binding
  position (side + line) matches its `DiffPosition`; its rows are inserted
  immediately after the anchor row. Every display row renders exactly ONE
  terminal line (keeps viewport math simple): a comment block expands to a
  header row (`@autor` + marker) plus one row per body line, all tagged with
  the same `CommentEntry` and a `block_start: bool`; `j`/`k` stop only on
  rows with `block_start == true` (the rest are skipped). Multi-comment
  threads produce one block per comment.
- Comments whose position matches no rendered row ("outdated"/orphaned) are
  appended after the last diff row under a `(desatualizado)` group.
- `AppState.comments_hidden: bool` (key `T` toggles) short-circuits the layer:
  hidden → display rows are exactly the diff rows (current behavior).

### Cursor and navigation

- `session.cursor_row`/`scroll_row` keep meaning DIFF row indices on disk
  (session schema unchanged, restore untouched). A new in-memory
  `AppState.display_cursor: usize` walks display rows.
- `j`/`k` move `display_cursor`. When it lands on a `Diff` row, sync
  `session.cursor_row` to that diff row (dirty for session save). When on a
  `Comment` row, `session.cursor_row` keeps its last diff value.
- `v` (selection) is only accepted while on a `Diff` row; selection endpoints
  and validate_selection continue to use diff row indices (unchanged).
- Viewport scrolling uses display rows; the diff widget renders comment boxes
  between code lines: `┌ @autor ── draft? ┐ / body lines / └…┘` styled with
  `theme` (drafts: WARNING accent + "draft"; resolved threads: SUCCESS `✓`;
  cursor-on-comment: CURSOR_LINE bg full width).

### Actions on a comment row

- `e` — only when `CommentEntry::Draft` (own draft): opens the existing editor
  pre-filled with the draft body, in "edit mode" (`AppState.editing_draft:
  Option<DraftId>`); Enter dispatches `AppAction::UpdateDraft { id, body }`
  instead of CreateDraft. Esc cancels without changes.
- `x` — on a Draft row: confirmation dialog (same pattern as quit dialog:
  `▸` menu, Enter confirms, Esc cancels) then `AppAction::DeleteDraft(id)`.
  v1 does not delete published thread comments.
- `r` — on a Thread row: opens the editor in "reply mode"
  (`AppState.replying_thread: Option<ThreadId>`); Enter dispatches
  `AppAction::Reply { thread, body }`. (Key `r` currently = Refresh; while the
  cursor is on a comment row, `r` means reply; Refresh stays `r` elsewhere.)
- `c`/`s` on a comment row: no-op with notice ("mova para uma linha de código").

### Saving feedback (applies to create/update/delete/reply)

- `busy_operations` already tracks in-flight effects. The status bar gains a
  spinner (`⠋⠙⠸…` braille frames advanced on Tick) plus a label for the most
  recent pending operation: `salvando comentário…`, `atualizando…`,
  `excluindo…`, `respondendo…`, `enviando revisão…`. Label chosen from a new
  `AppState.pending_labels: BTreeMap<u64, &'static str>` (request id → label)
  filled by the reducer when scheduling those effects.
- On success the comment appears/updates/disappears inline (snapshot drafts
  and threads are already updated by the existing EffectOutcome handling —
  verify DraftCreated/DraftUpdated/Completed handlers update
  `provider.drafts`/`threads`; extend where they don't).
- On failure: existing error banner path.

### Out of scope (v1)

- Deleting/editing published (non-draft) comments; resolving from the inline
  box (stays in the `t` panel); markdown rendering inside bodies (plain text);
  comment-count badges in the Files panel (already shows `*N`).

## Testing

- `tests/display_rows.rs`: anchoring (side/line match), multi-comment threads,
  orphaned comments group, hidden toggle, empty cases.
- Navigation tests: display_cursor sync with session.cursor_row; `v` refused
  on comment rows; viewport keeps cursor visible with interleaved comments.
- Action tests (reducer): `e` opens editor pre-filled + Enter → UpdateDraft;
  `x` → confirm → DeleteDraft; `r` on thread → Reply; labels registered in
  pending_labels; spinner label appears in status render while busy.
- Render test: comment box under the anchored line, draft marker, full-width
  cursor bg on comment rows, `T` hides.
