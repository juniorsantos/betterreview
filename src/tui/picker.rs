use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::{
    domain::{ChangeRequestKey, ChangeRequestSummary, ProviderKind, ProviderSnapshot},
    providers::ReviewProvider,
};

use super::{TuiError, theme};

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
    ListFailed(String),
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
        PickerEvent::ListFailed(message) => {
            state.error_banner = Some(message);
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
            let target = (state.highlight + 1).min(state.items.len().saturating_sub(1));
            move_highlight(state, target);
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let target = state.highlight.saturating_sub(1);
            move_highlight(state, target);
            Vec::new()
        }
        KeyCode::Enter => enter_update(state),
        _ => Vec::new(),
    }
}

/// Moves the highlight to `target`, cancelling any pending `Enter` so a
/// prefetch that finishes later for the previously highlighted item does not
/// auto-open it out from under the user.
fn move_highlight(state: &mut PickerState, target: usize) {
    if target != state.highlight {
        state.highlight = target;
        state.waiting = None;
        state.error_banner = None;
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
    state.errors.clear();
}

/// Maps freshly-listed change request summaries into `PickerItem`s, marking
/// which one matches the current branch and which ones already have a
/// terminal session. Pinning the current-branch item is handled separately
/// by `pin_current_branch`.
pub fn mark_items(
    list: Vec<ChangeRequestSummary>,
    branch: Option<&str>,
    sessions: &BTreeSet<u64>,
) -> Vec<PickerItem> {
    list.into_iter()
        .map(|summary| {
            let current_branch = branch == Some(summary.source_branch.as_str());
            let has_session = sessions.contains(&summary.number);
            PickerItem {
                summary,
                has_session,
                current_branch,
            }
        })
        .collect()
}

/// Renders the review picker screen: header, item list, prefetch status and
/// key hints.
pub fn render(frame: &mut Frame, state: &PickerState) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BG).fg(theme::FG)),
        area,
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(Paragraph::new(title_line(state)), rows[0]);
    render_list(frame, rows[1], state);
    frame.render_widget(Paragraph::new(status_line(state)), rows[2]);
    frame.render_widget(Paragraph::new(footer_line(state)), rows[3]);
}

fn title_line(state: &PickerState) -> Line<'static> {
    let text = match state
        .items
        .first()
        .and_then(|item| repo_from_url(&item.summary.web_url))
    {
        Some(repo) => format!(" Reviews abertos — {repo}"),
        None => " Reviews abertos".to_string(),
    };
    Line::raw(text)
}

/// Best-effort `owner/repo` extraction from a change request's web URL.
fn repo_from_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let mut segments = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty());
    let owner = segments.next()?;
    let repo = segments.next()?;
    Some(format!("{owner}/{repo}"))
}

fn render_list(frame: &mut Frame, area: Rect, state: &PickerState) {
    let now = OffsetDateTime::now_utc();
    let width = area.width as usize;
    let lines: Vec<Line> = state
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| item_line(item, now, width, index == state.highlight))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn item_line(
    item: &PickerItem,
    now: OffsetDateTime,
    width: usize,
    highlighted: bool,
) -> Line<'static> {
    let marker = if item.current_branch { "●" } else { "▸" };
    let marker_style = if item.current_branch {
        Style::default().fg(theme::ACCENT)
    } else {
        Style::default()
    };
    let prefix = format!(" #{} ", item.summary.number);
    let suffix = format!(
        "  @{}  {}  {}",
        item.summary.author,
        item.summary.source_branch,
        age(now, item.summary.updated_at)
    );

    let mut tail_spans = vec![Span::raw(suffix)];
    if item.summary.draft {
        tail_spans.push(Span::styled(" [draft]", Style::default().fg(theme::MUTED)));
    }
    if item.has_session {
        tail_spans.push(Span::styled(
            " [sessão]",
            Style::default().fg(theme::WARNING),
        ));
    }

    let fixed_width = 1
        + prefix.chars().count()
        + tail_spans
            .iter()
            .map(|span| span.content.chars().count())
            .sum::<usize>();
    let title_budget = width.saturating_sub(fixed_width).max(1);
    let title = truncate_title(&item.summary.title, title_budget);

    let mut spans = vec![
        Span::styled(marker, marker_style),
        Span::raw(prefix),
        Span::raw(title),
    ];
    spans.extend(tail_spans);
    let mut line = Line::from(spans);

    if highlighted {
        // Pad so the background reaches the panel's right edge.
        let text_width = line.width();
        if text_width < width {
            line.spans.push(Span::raw(" ".repeat(width - text_width)));
        }
        line = line.style(
            Style::default()
                .bg(theme::CURSOR_LINE)
                .add_modifier(Modifier::BOLD),
        );
    }
    line
}

fn truncate_title(title: &str, budget: usize) -> String {
    if title.chars().count() <= budget {
        return title.to_owned();
    }
    let mut shown: String = title.chars().take(budget.saturating_sub(1)).collect();
    shown.push('…');
    shown
}

fn status_line(state: &PickerState) -> Line<'static> {
    if let Some(message) = &state.error_banner {
        return Line::styled(format!(" {message}"), Style::default().fg(theme::DANGER));
    }
    if let Some(number) = state.loading {
        return Line::styled(
            format!(" baixando #{number}…"),
            Style::default().fg(theme::MUTED),
        );
    }
    if let Some(item) = state.items.get(state.highlight)
        && state.cache.contains_key(&item.summary.number)
    {
        return Line::styled(
            format!(" #{} pronto", item.summary.number),
            Style::default().fg(theme::SUCCESS),
        );
    }
    Line::raw("")
}

fn footer_line(state: &PickerState) -> Line<'static> {
    let mut text = String::from(" j/k mover  Enter abrir  r recarregar  q sair");
    if state.items.len() == 50 {
        text.push_str("  (50 mais recentes)");
    }
    Line::styled(text, Style::default().fg(theme::MUTED))
}

/// Formats the time elapsed between `updated` and `now` as a short,
/// human-readable age: `"agora"`, minutes, hours, or days.
pub fn age(now: OffsetDateTime, updated: OffsetDateTime) -> String {
    let seconds = (now - updated).whole_seconds().max(0);
    if seconds < 60 {
        "agora".to_string()
    } else if seconds < 60 * 60 {
        format!("{}m", seconds / 60)
    } else if seconds < 24 * 60 * 60 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}

/// Result of running the review picker screen to completion.
// The `Open` variant is intentionally not boxed: this is the public shape
// mandated by the picker spec and consumed by later tasks as-is.
#[allow(clippy::large_enum_variant)]
pub enum PickerOutcome {
    Quit,
    Open {
        number: u64,
        snapshot: Option<ProviderSnapshot>,
    },
}

/// Everything the picker's async loop needs beyond the pure `PickerState`:
/// the provider to prefetch and re-list change requests from, and the
/// context used to mark freshly-listed items.
pub struct PickerSource {
    pub provider: Arc<dyn ReviewProvider>,
    pub kind: ProviderKind,
    pub host: String,
    pub repository: String,
    pub branch: Option<String>,
    pub sessions: BTreeSet<u64>,
}

fn key_for(source: &PickerSource, number: u64) -> ChangeRequestKey {
    ChangeRequestKey {
        provider: source.kind,
        host: source.host.clone(),
        repository: source.repository.clone(),
        number,
    }
}

/// Drives the review picker screen: renders `state`, prefetches the
/// highlighted change request on a background task, reloads the open list
/// on request, and returns once the user quits or chooses one to open.
pub async fn run(
    terminal: &mut ratatui::DefaultTerminal,
    mut state: PickerState,
    source: PickerSource,
) -> Result<PickerOutcome, TuiError> {
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(300));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PickerEvent>();
    let mut prefetch: Option<(u64, tokio::task::JoinHandle<()>)> = None;

    loop {
        terminal
            .draw(|frame| render(frame, &state))
            .map_err(TuiError::Draw)?;

        let event = tokio::select! {
            _ = tick.tick() => Some(PickerEvent::Tick),
            event = event_rx.recv() => event,
            terminal_event = events.next() => match terminal_event {
                Some(Ok(Event::Key(key))) if is_interrupt(key) => return Ok(PickerOutcome::Quit),
                Some(Ok(Event::Key(key))) => Some(PickerEvent::Key(key)),
                Some(Ok(_)) => None,
                Some(Err(error)) => return Err(TuiError::Event(error)),
                None => return Ok(PickerOutcome::Quit),
            },
        };
        let Some(event) = event else {
            continue;
        };

        for command in update(&mut state, event) {
            dispatch_command(command, &source, &event_tx, &mut prefetch);
        }

        if state.quit {
            return Ok(PickerOutcome::Quit);
        }
        if let Some((number, snapshot)) = state.chosen.take() {
            return Ok(PickerOutcome::Open { number, snapshot });
        }
    }
}

/// Spawns the background task behind a `PickerCommand`, sending its result
/// back over `event_tx`. Prefetch tasks for a change request that is
/// already loading (or already loaded) are left alone; any other in-flight
/// prefetch is aborted first.
fn dispatch_command(
    command: PickerCommand,
    source: &PickerSource,
    event_tx: &mpsc::UnboundedSender<PickerEvent>,
    prefetch: &mut Option<(u64, tokio::task::JoinHandle<()>)>,
) {
    match command {
        PickerCommand::StartPrefetch(number) => {
            if let Some((current, handle)) = prefetch.as_ref()
                && *current == number
                && !handle.is_finished()
            {
                return;
            }
            if let Some((_, handle)) = prefetch.take() {
                handle.abort();
            }
            let provider = source.provider.clone();
            let key = key_for(source, number);
            let event_tx = event_tx.clone();
            let handle = tokio::spawn(async move {
                let result = provider.load(&key).await;
                let _ = event_tx.send(PickerEvent::Loaded {
                    number,
                    result: result.map_err(|error| error.to_string()),
                });
            });
            *prefetch = Some((number, handle));
        }
        PickerCommand::ReloadList => {
            let provider = source.provider.clone();
            let host = source.host.clone();
            let repository = source.repository.clone();
            let branch = source.branch.clone();
            let sessions = source.sessions.clone();
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let event = match provider.list_open(&host, &repository).await {
                    Ok(list) => PickerEvent::ListReloaded {
                        items: mark_items(list, branch.as_deref(), &sessions),
                    },
                    Err(error) => PickerEvent::ListFailed(error.to_string()),
                };
                let _ = event_tx.send(event);
            });
        }
    }
}

fn is_interrupt(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}
