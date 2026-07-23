use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use time::OffsetDateTime;

use crate::domain::{ChangeRequestSummary, ProviderSnapshot};

use super::theme;

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
