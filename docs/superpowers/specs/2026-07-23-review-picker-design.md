# Review picker with prefetch — design

Date: 2026-07-23
Status: approved direction; spec pending user review

## Problem

Opening a review requires pasting a PR/MR URL. Running `betterreview` inside a
repo only works when the current branch already has an open PR; otherwise it
fails with "no review". The user wants to run `betterreview` inside any git
repo, see the open PRs/MRs, pick one, and have it already downloading while
they choose.

## Decisions (validated with the user)

- Initial screen: current-branch PR/MR highlighted at the top when it exists,
  followed by the list of all other open PRs/MRs of the repo.
- Prefetch: the highlighted item starts downloading immediately; moving the
  highlight and resting ~300ms switches the prefetch (previous one aborted).
  Enter uses whatever is ready, or waits on the in-flight download.
- Explicit targets (`betterreview <url|number>`) keep today's direct flow.

## Architecture

### Provider contract

New method on `ReviewProvider`:

```rust
async fn list_open(
    &self,
    host: &str,
    repository: &str,
) -> Result<Vec<ChangeRequestSummary>, ProviderError>;
```

`ChangeRequestSummary` (new, in `domain`): `number`, `title`, `author`,
`source_branch`, `updated_at`, `draft: bool`, `web_url`.

- GitHub: one GraphQL query (`pullRequests(states: OPEN, first: 50, orderBy:
  UPDATED_AT desc)` with `headRefName`, `isDraft`, `updatedAt`, `author`).
- GitLab: one REST call
  (`projects/:id/merge_requests?state=opened&order_by=updated_at&per_page=50`).
- One page (50) is enough for the picker; no pagination in v1. If the repo has
  more, the footer shows "showing 50 most recent".

### Launch flow (entrypoint)

`LaunchRequest::Review` without an explicit target no longer requires the
current branch to resolve to a PR:

1. Resolve context as today (cwd → provider, host, repository, branch).
2. Run in parallel: `Doctor::check` and `provider.list_open`.
3. If doctor fails → same error as today. If the list is empty → "no open
   reviews" message (exit).
4. Open the picker screen. The current branch's PR (matched by
   `source_branch == branch`) is highlighted at the top when present,
   otherwise the first item is highlighted.
5. Selecting an item runs today's `launch_key` flow, reusing the prefetched
   snapshot when available (skip `provider.load`, keep session restore, doctor
   NOT re-run).

Explicit target (URL/number) and `resume`/`sessions` keep the current paths
untouched.

### Picker screen (new TUI mode)

New module `src/tui/picker.rs` with its own small state + event loop, running
before `AppState` exists (same pattern as the main loop: crossterm
`EventStream` + tick).

State: `items: Vec<PickerItem>` (summary + `has_session: bool` +
`is_current_branch: bool`), `highlight: usize`, `prefetch: PrefetchState`,
`error: Option<String>`.

Keys: `j`/`k`/arrows navigate, `Enter` opens, `r` reloads the list, `q`/Esc
quits. Footer shows the key hints.

Row format: `#42  fix: title…  @author  branch  2h  [resume] [draft]` with the
current-branch row pinned first and visually marked.

### Prefetch

- `PrefetchState` holds the target number, a `tokio::task::JoinHandle` and an
  abort handle; results land in a `BTreeMap<u64, ProviderSnapshot>` cache via
  an mpsc channel back to the picker loop (same channel pattern as
  `tui::run`).
- Highlight change arms a 300ms debounce (tick-based, no extra timer thread);
  when it fires and the highlighted number has no cache entry and no in-flight
  task, the previous task is aborted and a new `provider.load` task spawns.
- Enter: cache hit → open immediately; in-flight for that number → spinner in
  the row + open when it lands; otherwise spawn load and wait (same spinner).
- Prefetch errors are stored per number and only surface if the user presses
  Enter on that item (footer error + automatic retry on Enter). Navigation
  never shows transient prefetch errors.
- Session save on open: unchanged (`launch_key` already handles it). A
  prefetched snapshot may be up to a few minutes stale; acceptable for v1
  because `RefreshSnapshot` (`r` inside the review) already exists and head
  drift is detected on submit.

### Error handling

- List failure: picker shows the error with `r` to retry (doctor guidance
  preserved verbatim when it is a dependency/auth problem).
- Prefetch failure: retry on Enter as above; aborted tasks are ignored.
- Repo without detectable provider/remote: same `ContextError` as today.

## Testing

- `provider_github_read` / `provider_gitlab_read`: `list_open` fixtures →
  summaries mapped correctly (branch, draft, updated_at ordering).
- New `tests/picker.rs`: pure-state tests for highlight movement, debounce
  arming, prefetch cache/abort bookkeeping (fake load futures), Enter
  behavior on hit/in-flight/error, current-branch pinning, and `has_session`
  marking (fake session store listing).
- Entrypoint test: no-target launch inside a repo with open PRs reaches the
  picker path; explicit target bypasses it.

## Out of scope (v1)

- Pagination beyond 50 items; filters ("only mine", "needs my review");
  cross-repo listing; the visual restyle (Etapa 3 handles theme/layout).
