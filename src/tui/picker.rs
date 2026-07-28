use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use crossterm::event::{
    Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use futures_util::StreamExt;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::{
    domain::{ChangeRequestKey, ChangeRequestSummary, ProviderKind, ProviderSnapshot},
    providers::ReviewProvider,
};

use super::{
    TuiError,
    text::{display_width, truncate_to_width},
    theme,
    widgets::{header, status},
};

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
    pub repository: String,
    /// Vertical scroll offset of the description panel's body, reset
    /// whenever the highlighted item changes.
    pub detail_scroll: u16,
    /// `true` when keyboard focus is on the description panel (`[1]`)
    /// instead of the list panel (`[0]`).
    pub focus_detail: bool,
    /// `true` when the terminal is tall enough for `render` to draw the
    /// description panel (mirrors `area.height >= DETAIL_HIDE_THRESHOLD`).
    /// The reducer has no access to the terminal size, so `run`'s draw loop
    /// refreshes this every iteration before dispatching events; it keeps
    /// `Tab`/`1` from focusing a panel that isn't actually on screen.
    pub detail_visible: bool,
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
    /// A left click on list row `index` (0-based into `items`, already
    /// resolved from screen coordinates by `run`'s row math). Highlights it,
    /// or opens it (like `Enter`) when it was already highlighted.
    ClickList(usize),
    /// A left click anywhere inside the description panel: focuses it, the
    /// same as `Tab`/`1`.
    ClickDetail,
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
    pub fn new(mut items: Vec<PickerItem>, repository: String) -> Self {
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
            repository,
            detail_scroll: 0,
            focus_detail: false,
            detail_visible: true,
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
        PickerEvent::ClickList(index) => click_list_update(state, index),
        PickerEvent::ClickDetail => {
            if state.detail_visible {
                state.focus_detail = true;
            }
            Vec::new()
        }
    }
}

/// A click on a list row moves the highlight there, same as `j`/`k` — unless
/// it was already the highlighted row, in which case the click opens it
/// (`enter_update`), mirroring a double-click/second-click-to-open pattern.
/// Clicks past the end of the list (e.g. into the row's trailing blank
/// space) are ignored.
fn click_list_update(state: &mut PickerState, index: usize) -> Vec<PickerCommand> {
    if index >= state.items.len() {
        return Vec::new();
    }
    if index == state.highlight {
        return enter_update(state);
    }
    move_highlight(state, index);
    Vec::new()
}

fn key_update(state: &mut PickerState, key: KeyEvent) -> Vec<PickerCommand> {
    if key.kind == KeyEventKind::Release {
        return Vec::new();
    }
    if !state.detail_visible {
        // The description panel isn't on screen: keep focus pinned to the
        // list and ignore the keys that would otherwise move it there.
        state.focus_detail = false;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.quit = true;
            Vec::new()
        }
        KeyCode::Char('r') => vec![PickerCommand::ReloadList],
        KeyCode::Tab if !state.detail_visible => Vec::new(),
        KeyCode::Tab => {
            state.focus_detail = !state.focus_detail;
            Vec::new()
        }
        KeyCode::Char('0') => {
            state.focus_detail = false;
            Vec::new()
        }
        KeyCode::Char('1') if !state.detail_visible => Vec::new(),
        KeyCode::Char('1') => {
            state.focus_detail = true;
            Vec::new()
        }
        KeyCode::Char('j') | KeyCode::Down if state.focus_detail => {
            state.detail_scroll = state.detail_scroll.saturating_add(1);
            Vec::new()
        }
        KeyCode::Char('k') | KeyCode::Up if state.focus_detail => {
            state.detail_scroll = state.detail_scroll.saturating_sub(1);
            Vec::new()
        }
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
/// auto-open it out from under the user, and resetting the description
/// panel's scroll so it starts at the top of the newly highlighted item.
fn move_highlight(state: &mut PickerState, target: usize) {
    if target != state.highlight {
        state.highlight = target;
        state.waiting = None;
        state.error_banner = None;
        state.detail_scroll = 0;
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
    if items.is_empty() {
        state.error_banner = Some("no open reviews".into());
        return;
    }
    pin_current_branch(&mut items);
    let highlighted_number = state
        .items
        .get(state.highlight)
        .map(|item| item.summary.number);
    state.items = items;
    state.highlight = highlighted_number
        .and_then(|number| {
            state
                .items
                .iter()
                .position(|item| item.summary.number == number)
        })
        .unwrap_or(0);
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

/// Right-side hints for the picker's flat status bar (transversal rule 1).
const PICKER_HINTS: [(&str, &str); 5] = [
    ("j/k", "move"),
    ("⇥", "focus"),
    ("↵", "open"),
    ("r", "reload"),
    ("q", "quit"),
];

/// Below this many total rows the description panel is hidden entirely and
/// the list panel takes the whole body.
const DETAIL_HIDE_THRESHOLD: u16 = 14;
/// The list panel keeps at least this many rows (header/blank rows, a
/// couple of items, and the counter) before the description panel is
/// allowed to grow past its ~40% share.
const LIST_MIN_HEIGHT: u16 = 8;

/// Renders the review picker screen: borderless chip header, the rounded
/// list and description panels, and a borderless flat status line.
pub fn render(frame: &mut Frame, state: &PickerState) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme::canvas()), area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let middle = if state.repository.is_empty() {
        " ".to_string()
    } else {
        format!(" {} ", state.repository)
    };
    frame.render_widget(
        Paragraph::new(header::chip_line(&middle, area.width)),
        rows[0],
    );

    if area.height >= DETAIL_HIDE_THRESHOLD {
        let body = rows[2];
        let list_height = ((body.height as u32 * 60) / 100)
            .max(LIST_MIN_HEIGHT as u32)
            .min(body.height as u32) as u16;
        let detail_height = body.height - list_height;
        let panels = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(list_height),
                Constraint::Length(detail_height),
            ])
            .split(body);
        render_panel(frame, panels[0], state, !state.focus_detail);
        render_detail(frame, panels[1], state, state.focus_detail);
    } else {
        render_panel(frame, rows[2], state, !state.focus_detail);
    }

    frame.render_widget(Paragraph::new(status_line(state, rows[3].width)), rows[3]);
}

fn panel_border_style(focused: bool) -> Style {
    Style::default().fg(if focused {
        theme::ACCENT
    } else {
        theme::BORDER
    })
}

/// Draws the rounded list panel: a blank line, the column header row, a
/// blank line, the item rows, and the reviews-open counter as the last
/// inner row. Border is ACCENT when `focused`, BORDER otherwise.
fn render_panel(frame: &mut Frame, area: Rect, state: &PickerState, focused: bool) {
    let block = Block::default()
        .padding(ratatui::widgets::Padding::horizontal(1))
        .title(" [0] Open reviews ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }
    let items_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height - 1,
    };
    let counter_area = Rect {
        x: inner.x,
        y: inner.y + inner.height - 1,
        width: inner.width,
        height: 1,
    };

    render_list(frame, items_area, state);
    frame.render_widget(
        Paragraph::new(counter_line(state, inner.width)),
        counter_area,
    );
}

fn counter_line(state: &PickerState, width: u16) -> Line<'static> {
    let text = if state.items.len() == 50 {
        "50 mais recentes".to_string()
    } else {
        format!("{} open reviews", state.items.len())
    };
    let used = display_width(&text) + 1;
    let pad = (width as usize).saturating_sub(used);
    Line::styled(
        format!("{}{text} ", " ".repeat(pad)),
        Style::default().fg(theme::MUTED),
    )
}

/// Column widths shared by the header row and every item row, so the
/// AUTOR/BRANCH columns start at the same offset on every line.
#[derive(Clone, Copy)]
struct Columns {
    show_author: bool,
    show_branch: bool,
    title_width: usize,
}

const CURSOR_WIDTH: usize = 2;
const PR_WIDTH: usize = 7;
const AUTHOR_DOT_WIDTH: usize = 2;
const AUTHOR_TEXT_WIDTH: usize = 14;
const AUTHOR_WIDTH: usize = AUTHOR_DOT_WIDTH + AUTHOR_TEXT_WIDTH;
const BRANCH_WIDTH: usize = 20;
const WHEN_WIDTH: usize = 8;
/// Room reserved after WHEN for the ragged " draft"/" session" badges, so
/// they never get clipped by the panel's right edge.
const BADGE_RESERVE: usize = 14;
/// Below this inner width the BRANCH column is dropped so the title keeps
/// room to breathe.
const NARROW_BRANCH_THRESHOLD: usize = 70;
/// Below this inner width the AUTOR column is dropped too.
const NARROW_AUTHOR_THRESHOLD: usize = 50;

fn columns_for(width: usize) -> Columns {
    let show_branch = width >= NARROW_BRANCH_THRESHOLD;
    let show_author = width >= NARROW_AUTHOR_THRESHOLD;
    let mut reserved = CURSOR_WIDTH + PR_WIDTH + WHEN_WIDTH + BADGE_RESERVE;
    if show_author {
        reserved += AUTHOR_WIDTH;
    }
    if show_branch {
        reserved += BRANCH_WIDTH;
    }
    let title_width = width.saturating_sub(reserved).max(4);
    Columns {
        show_author,
        show_branch,
        title_width,
    }
}

fn render_list(frame: &mut Frame, area: Rect, state: &PickerState) {
    let now = OffsetDateTime::now_utc();
    let columns = columns_for(area.width as usize);
    let mut lines = vec![Line::raw(""), header_line(columns), Line::raw("")];
    lines.extend(state.items.iter().enumerate().map(|(index, item)| {
        item_line(
            item,
            now,
            columns,
            area.width as usize,
            index == state.highlight,
        )
    }));
    // Keep the highlighted row inside the visible window (50 items easily
    // exceed the panel height).
    let visible = area.height as usize;
    let start = super::viewport::start(3 + state.highlight, lines.len(), visible);
    let scroll = u16::try_from(start).unwrap_or(u16::MAX);
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

/// The `PR TITLE AUTHOR BRANCH WHEN` column header: MUTED BOLD uppercase,
/// aligned to the same column widths as the item rows below it.
fn header_line(columns: Columns) -> Line<'static> {
    let style = Style::default()
        .fg(theme::MUTED)
        .add_modifier(Modifier::BOLD);
    let mut text = format!(
        "  {}{}",
        pad_cell("PR", PR_WIDTH),
        pad_cell("TITLE", columns.title_width)
    );
    if columns.show_author {
        text.push_str(&pad_cell("AUTHOR", AUTHOR_WIDTH));
    }
    if columns.show_branch {
        text.push_str(&pad_cell("BRANCH", BRANCH_WIDTH));
    }
    text.push_str("WHEN");
    Line::styled(text, style)
}

/// One row of the list: `▶ ` marker + selection background when
/// highlighted (transversal rule 2), otherwise a plain two-space indent.
/// Number BOLD, title FG (truncated with `…`), AUTOR/BRANCH/QUANDO MUTED,
/// `●` ACCENT before the author for the current-branch item, and badges
/// after WHEN: `draft` MUTED, `session` WARNING.
fn item_line(
    item: &PickerItem,
    now: OffsetDateTime,
    columns: Columns,
    row_width: usize,
    highlighted: bool,
) -> Line<'static> {
    let cursor = if highlighted { "▶ " } else { "  " };
    let pr = pad_cell(&format!("#{}", item.summary.number), PR_WIDTH);
    let title = pad_cell(&item.summary.title, columns.title_width);

    let mut spans = vec![
        Span::raw(cursor),
        Span::styled(pr, Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(title, Style::default().fg(theme::FG)),
    ];

    if columns.show_author {
        if item.current_branch {
            spans.push(Span::styled("● ", Style::default().fg(theme::ACCENT)));
            spans.push(Span::styled(
                pad_cell("you", AUTHOR_TEXT_WIDTH),
                Style::default().fg(theme::MUTED),
            ));
        } else {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                pad_cell(&format!("@{}", item.summary.author), AUTHOR_TEXT_WIDTH),
                Style::default().fg(theme::MUTED),
            ));
        }
    }
    if columns.show_branch {
        spans.push(Span::styled(
            pad_cell(&item.summary.source_branch, BRANCH_WIDTH),
            Style::default().fg(theme::MUTED),
        ));
    }
    spans.push(Span::styled(
        age(now, item.summary.updated_at),
        Style::default().fg(theme::MUTED),
    ));
    if item.summary.draft {
        spans.push(Span::styled(" draft", Style::default().fg(theme::MUTED)));
    }
    if item.has_session {
        spans.push(Span::styled(
            " session",
            Style::default().fg(theme::WARNING),
        ));
    }
    if item.summary.reviewed_current_head() {
        spans.push(Span::styled(
            " ✓ reviewed",
            Style::default().fg(theme::SUCCESS),
        ));
    }

    let mut line = Line::from(spans);

    if highlighted {
        // Pad so the background reaches the panel's inner right edge (the
        // full row width the Paragraph is rendered into), not just the
        // nominal column layout — the QUANDO/badge tail is ragged and
        // otherwise falls short of the edge.
        let text_width = line.width();
        if text_width < row_width {
            line.spans
                .push(Span::raw(" ".repeat(row_width - text_width)));
        }
        line = line.style(
            Style::default()
                .bg(theme::SELECTION)
                .add_modifier(Modifier::BOLD),
        );
    }
    line
}

fn truncate_title(title: &str, budget: usize) -> String {
    if display_width(title) <= budget {
        return title.to_owned();
    }
    let mut shown = truncate_to_width(title, budget.saturating_sub(1));
    shown.push('…');
    shown
}

/// Truncates `text` to `width` columns (appending `…` when it overflows)
/// and pads it with trailing spaces so it always occupies exactly `width`
/// columns — the building block for aligned table columns.
fn pad_cell(text: &str, width: usize) -> String {
    // Always keep at least one trailing space as the column gap, so a
    // full-width value never glues onto its neighbor.
    let truncated = truncate_title(text, width.saturating_sub(1));
    let used = display_width(&truncated);
    format!("{truncated}{}", " ".repeat(width.saturating_sub(used)))
}

/// Draws the rounded description panel: title + `#N · aberto`, the
/// `@autor · branch` line, a blank line, then the (possibly scrolled)
/// description body. Border is ACCENT when `focused`, BORDER otherwise.
fn render_detail(frame: &mut Frame, area: Rect, state: &PickerState, focused: bool) {
    let block = Block::default()
        .padding(ratatui::widgets::Padding::horizontal(1))
        .title(" [1] Description — ⇥/1 focus · j/k scroll ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }
    let Some(item) = state.items.get(state.highlight) else {
        return;
    };

    let slots = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let width = inner.width as usize;
    let meta = format!("#{} · aberto", item.summary.number);
    frame.render_widget(
        Paragraph::new(title_meta_line(&item.summary.title, &meta, width)),
        slots[0],
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("@{} · {}", item.summary.author, item.summary.source_branch),
            Style::default().fg(theme::MUTED),
        )),
        slots[1],
    );

    let body_area = slots[3];
    if body_area.height == 0 || body_area.width == 0 {
        return;
    }
    if item.summary.description.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(
                "no description",
                Style::default().fg(theme::MUTED),
            )),
            body_area,
        );
        return;
    }
    let paragraph = Paragraph::new(item.summary.description.clone())
        .style(Style::default().fg(theme::FG))
        .wrap(Wrap { trim: false });
    let total_lines = wrapped_line_count(&item.summary.description, body_area.width as usize);
    let max_scroll = total_lines.saturating_sub(body_area.height);
    let scroll = state.detail_scroll.min(max_scroll);
    frame.render_widget(paragraph.scroll((scroll, 0)), body_area);
}

/// Approximates how many rows `text` occupies once word-wrapped to `width`
/// columns — used only to clamp the description panel's scroll offset to
/// the content height, so an approximation (no hard mid-word breaking) is
/// good enough.
fn wrapped_line_count(text: &str, width: usize) -> u16 {
    let width = width.max(1);
    let mut total = 0usize;
    for paragraph in text.split('\n') {
        if paragraph.trim().is_empty() {
            total += 1;
            continue;
        }
        let mut current = 0usize;
        let mut lines_in_paragraph = 1usize;
        for word in paragraph.split_whitespace() {
            let word_len = display_width(word);
            if current == 0 {
                current = word_len;
            } else if current + 1 + word_len <= width {
                current += 1 + word_len;
            } else {
                lines_in_paragraph += 1;
                current = word_len;
            }
        }
        total += lines_in_paragraph;
    }
    total.max(1) as u16
}

/// Builds `left` in BOLD followed by `right` in MUTED, right-aligned within
/// `width` columns (falling back to a single space gap when `left` and
/// `right` together don't fit).
fn title_meta_line(left: &str, right: &str, width: usize) -> Line<'static> {
    let left_span = Span::styled(
        left.to_owned(),
        Style::default().add_modifier(Modifier::BOLD),
    );
    let right_span = Span::styled(right.to_owned(), Style::default().fg(theme::MUTED));
    let used = display_width(left) + display_width(right);
    let pad = width.saturating_sub(used).max(1);
    Line::from(vec![left_span, Span::raw(" ".repeat(pad)), right_span])
}

/// Left side keeps the prefetch/error precedence; right side is the flat,
/// truncating hint list (transversal rule 1). An error replaces the whole
/// line in DANGER, same as before.
fn status_line(state: &PickerState, width: u16) -> Line<'static> {
    if let Some(message) = &state.error_banner {
        return Line::styled(format!(" {message}"), Style::default().fg(theme::DANGER));
    }
    let left = if let Some(number) = state.loading {
        Line::styled(
            format!(" baixando #{number}…"),
            Style::default().fg(theme::ACCENT),
        )
    } else if let Some(item) = state.items.get(state.highlight)
        && state.cache.contains_key(&item.summary.number)
    {
        Line::styled(
            format!(" #{} pronto", item.summary.number),
            Style::default().fg(theme::SUCCESS),
        )
    } else {
        Line::raw("")
    };
    status::flat_line(left, &PICKER_HINTS, width)
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
        let terminal_height = terminal.size().map(|size| size.height).unwrap_or(0);
        state.detail_visible = terminal_height >= DETAIL_HIDE_THRESHOLD;
        terminal
            .draw(|frame| render(frame, &state))
            .map_err(TuiError::Draw)?;

        let event = tokio::select! {
            _ = tick.tick() => Some(PickerEvent::Tick),
            event = event_rx.recv() => event,
            terminal_event = events.next() => match terminal_event {
                Some(Ok(Event::Key(key))) if is_interrupt(key) => return Ok(PickerOutcome::Quit),
                Some(Ok(Event::Key(key))) => Some(PickerEvent::Key(key)),
                Some(Ok(Event::Mouse(mouse))) => wheel_to_key(mouse.kind)
                    .map(PickerEvent::Key)
                    .or_else(|| {
                        terminal.size().ok().and_then(|size| {
                            click_event(&state, Rect::new(0, 0, size.width, size.height), mouse)
                        })
                    }),
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

/// Translates a mouse wheel notch into the same synthetic key the keyboard
/// would send — `j`/`k` — so it flows through the existing `key_update`
/// handling unchanged (moving the highlight, or scrolling the description
/// panel when it's focused). Any other mouse event kind is not ours to
/// interpret and is dropped.
fn wheel_to_key(kind: MouseEventKind) -> Option<KeyEvent> {
    match kind {
        MouseEventKind::ScrollDown => Some(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
        MouseEventKind::ScrollUp => Some(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)),
        _ => None,
    }
}

/// Splits `area` into the list panel rect and, when the description panel
/// is on screen (`state.detail_visible`, refreshed every frame by `run`),
/// the detail panel rect below it — the same vertical split `render`
/// performs, shared here so a click can be hit-tested against exactly the
/// geometry the frame was last drawn with.
fn panel_layout(area: Rect, state: &PickerState) -> (Rect, Option<Rect>) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let body = rows[2];
    if !state.detail_visible {
        return (body, None);
    }
    let list_height = ((body.height as u32 * 60) / 100)
        .max(LIST_MIN_HEIGHT as u32)
        .min(body.height as u32) as u16;
    let panels = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(list_height),
            Constraint::Length(body.height - list_height),
        ])
        .split(body);
    (panels[0], Some(panels[1]))
}

/// The list panel's item rows, one row of border/padding in from the panel
/// and with the trailing counter row excluded — reusing `Block::inner` so
/// this can never drift from `render_panel`'s own border/padding math.
fn list_items_rect(list_rect: Rect) -> Rect {
    let inner = Block::default()
        .padding(ratatui::widgets::Padding::horizontal(1))
        .borders(Borders::ALL)
        .inner(list_rect);
    Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    }
}

fn contains(rect: Rect, point: (u16, u16)) -> bool {
    let (x, y) = point;
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

/// Handles a left mouse-button press by hit-testing it against the panel
/// layout for `area` (recomputed here from `terminal.size()`, see
/// `panel_layout`): a click on a list row resolves to `ClickList`, a click
/// anywhere in the description panel to `ClickDetail`. Every other mouse
/// event kind, and a click landing outside both panels, is not ours to
/// interpret.
fn click_event(state: &PickerState, area: Rect, mouse: MouseEvent) -> Option<PickerEvent> {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return None;
    }
    let (list_rect, detail_rect) = panel_layout(area, state);
    let point = (mouse.column, mouse.row);
    if contains(list_rect, point) {
        return click_list_row(state, list_rect, mouse.row).map(PickerEvent::ClickList);
    }
    if let Some(detail_rect) = detail_rect
        && contains(detail_rect, point)
    {
        return Some(PickerEvent::ClickDetail);
    }
    None
}

/// Maps a click's row inside the list panel to an item index, mirroring
/// `render_list`'s "two blank rows + header" offset and scroll — `None`
/// when the click landed on the header/blank rows or past the last item.
fn click_list_row(state: &PickerState, list_rect: Rect, mouse_row: u16) -> Option<usize> {
    let items_area = list_items_rect(list_rect);
    let content_row = mouse_row.checked_sub(items_area.y)?;
    if content_row >= items_area.height {
        return None;
    }
    let total_lines = 3 + state.items.len();
    let start =
        super::viewport::start(3 + state.highlight, total_lines, items_area.height as usize);
    (content_row as usize + start)
        .checked_sub(3)
        .filter(|&index| index < state.items.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(count: u64) -> Vec<PickerItem> {
        (1..=count)
            .map(|number| PickerItem {
                summary: ChangeRequestSummary {
                    number,
                    title: format!("PR {number}"),
                    author: "dev".into(),
                    source_branch: "feature".into(),
                    updated_at: time::OffsetDateTime::UNIX_EPOCH,
                    draft: false,
                    web_url: String::new(),
                    description: String::new(),
                    head: crate::domain::CommitOid(format!("head-{number}")),
                    reviewed_head: None,
                },
                has_session: false,
                current_branch: false,
            })
            .collect()
    }

    fn left_click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// 100x30: `panel_layout` puts the list panel at `(0, 2, 100, 16)` and
    /// the description panel at `(0, 18, 100, 11)` (60% of the 27-row body,
    /// clamped to `LIST_MIN_HEIGHT`).
    const TERMINAL_SIZE: Rect = Rect::new(0, 0, 100, 30);

    #[test]
    fn clicking_an_item_row_resolves_to_its_index() {
        let state = PickerState::new(items(3), "owner/repo".into());

        // List content starts at y = 3 (2 blank/header rows follow); row 6
        // is the first item.
        let event = click_event(&state, TERMINAL_SIZE, left_click(5, 6));

        assert!(matches!(event, Some(PickerEvent::ClickList(0))));
    }

    #[test]
    fn clicking_the_column_header_row_is_ignored() {
        let state = PickerState::new(items(3), "owner/repo".into());

        // Row 4 is the "PR TITLE ..." header, not an item.
        let event = click_event(&state, TERMINAL_SIZE, left_click(5, 4));

        assert!(event.is_none());
    }

    #[test]
    fn clicking_the_description_panel_resolves_to_click_detail() {
        let state = PickerState::new(items(3), "owner/repo".into());

        let event = click_event(&state, TERMINAL_SIZE, left_click(5, 20));

        assert!(matches!(event, Some(PickerEvent::ClickDetail)));
    }

    #[test]
    fn clicking_outside_both_panels_is_ignored() {
        let state = PickerState::new(items(3), "owner/repo".into());

        let event = click_event(&state, TERMINAL_SIZE, left_click(5, 0));

        assert!(event.is_none());
    }

    #[test]
    fn a_right_click_is_not_translated() {
        let state = PickerState::new(items(3), "owner/repo".into());
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 5,
            row: 6,
            modifiers: KeyModifiers::NONE,
        };

        assert!(click_event(&state, TERMINAL_SIZE, mouse).is_none());
    }

    #[test]
    fn hidden_detail_panel_leaves_the_list_spanning_the_whole_body() {
        let mut state = PickerState::new(items(3), "owner/repo".into());
        state.detail_visible = false;

        let (list_rect, detail_rect) = panel_layout(TERMINAL_SIZE, &state);

        assert_eq!(list_rect, Rect::new(0, 2, 100, 27));
        assert!(detail_rect.is_none());
    }

    #[test]
    fn scroll_down_translates_to_a_j_key() {
        let key = wheel_to_key(MouseEventKind::ScrollDown).expect("scroll down maps to a key");
        assert_eq!(key.code, KeyCode::Char('j'));
    }

    #[test]
    fn scroll_up_translates_to_a_k_key() {
        let key = wheel_to_key(MouseEventKind::ScrollUp).expect("scroll up maps to a key");
        assert_eq!(key.code, KeyCode::Char('k'));
    }

    #[test]
    fn other_mouse_kinds_are_not_translated() {
        assert!(wheel_to_key(MouseEventKind::Moved).is_none());
        assert!(wheel_to_key(MouseEventKind::Down(crossterm::event::MouseButton::Left)).is_none());
    }
}
