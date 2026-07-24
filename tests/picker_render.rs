use std::collections::BTreeMap;

use betterreview::{
    domain::ChangeRequestSummary,
    tui::{
        picker::{PickerItem, PickerState, age, render},
        theme,
    },
};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use time::{Duration, OffsetDateTime};

fn summary(number: u64, author: &str, branch: &str, draft: bool) -> ChangeRequestSummary {
    ChangeRequestSummary {
        number,
        title: format!("Title for #{number}"),
        author: author.into(),
        source_branch: branch.into(),
        updated_at: OffsetDateTime::UNIX_EPOCH,
        draft,
        web_url: format!("https://github.com/owner/repo/pull/{number}"),
        description: String::new(),
    }
}

fn item(number: u64, author: &str, branch: &str, draft: bool, current_branch: bool) -> PickerItem {
    PickerItem {
        summary: summary(number, author, branch, draft),
        has_session: false,
        current_branch,
    }
}

fn item_with_description(number: u64, author: &str, branch: &str, description: &str) -> PickerItem {
    let mut picker_item = item(number, author, branch, false, false);
    picker_item.summary.description = description.into();
    picker_item
}

fn state(items: Vec<PickerItem>, highlight: usize) -> PickerState {
    PickerState {
        items,
        highlight,
        cache: BTreeMap::new(),
        errors: BTreeMap::new(),
        loading: None,
        waiting: None,
        error_banner: None,
        quit: false,
        chosen: None,
        repository: String::new(),
        detail_scroll: 0,
        focus_detail: false,
    }
}

fn draw(picker: &PickerState) -> Terminal<TestBackend> {
    draw_sized(picker, 100, 30)
}

fn draw_sized(picker: &PickerState, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, picker)).unwrap();
    terminal
}

fn screen_sized(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn screen(terminal: &Terminal<TestBackend>) -> String {
    screen_sized(terminal, 100, 30)
}

fn lines(terminal: &Terminal<TestBackend>) -> Vec<String> {
    screen(terminal).lines().map(str::to_owned).collect()
}

fn char_offset(haystack: &str, needle: &str) -> Option<usize> {
    let byte_offset = haystack.find(needle)?;
    Some(haystack[..byte_offset].chars().count())
}

#[test]
fn renders_items_with_pin_and_metadata() {
    let picker = state(vec![item(42, "jsjunior", "feature/login", true, false)], 0);

    let terminal = draw(&picker);
    let screen = screen(&terminal);

    assert!(screen.contains("#42"));
    assert!(screen.contains("@jsjunior"));
    assert!(screen.contains("feature/login"));
    assert!(screen.contains("draft"));
}

#[test]
fn renders_the_current_branch_dot_before_the_author() {
    let picker = state(
        vec![
            item(42, "jsjunior", "feature/login", false, true),
            item(43, "dev", "other", false, false),
        ],
        0,
    );

    let screen = screen(&draw(&picker));
    let pinned_row = screen
        .lines()
        .find(|line| line.contains("#42"))
        .expect("pinned row rendered");

    assert!(pinned_row.contains('●'));
    assert!(pinned_row.contains("você"));
}

#[test]
fn header_shows_a_reverse_video_app_chip_and_the_repository() {
    let mut picker = state(vec![item(1, "dev", "main", false, false)], 0);
    picker.repository = "group/sub/api".into();

    let terminal = draw(&picker);
    let screen = screen(&terminal);
    let buffer = terminal.backend().buffer();

    assert!(screen.contains("betterreview"));
    assert!(screen.contains("group/sub/api"));

    let header_row = screen.lines().next().unwrap();
    let chip_start = char_offset(header_row, "betterreview").unwrap();
    let cell = buffer.cell((chip_start as u16, 0)).unwrap();
    assert_eq!(cell.style().fg, Some(theme::ACCENT));
    assert!(cell.style().add_modifier.contains(Modifier::REVERSED));
}

#[test]
fn header_shows_the_version_chip_reverse_video_on_the_right() {
    let picker = state(vec![item(1, "dev", "main", false, false)], 0);

    let terminal = draw(&picker);
    let screen = screen(&terminal);
    let buffer = terminal.backend().buffer();

    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let header_row = screen.lines().next().unwrap();
    let chip_start = char_offset(header_row, &version).expect("version chip present");
    let cell = buffer.cell((chip_start as u16, 0)).unwrap();
    assert_eq!(cell.style().fg, Some(theme::MUTED));
    assert!(cell.style().add_modifier.contains(Modifier::REVERSED));
}

#[test]
fn list_panel_has_a_rounded_border_and_a_title() {
    let picker = state(vec![item(1, "dev", "main", false, false)], 0);

    let screen = screen(&draw(&picker));

    assert!(screen.contains('╭'), "expected a rounded top-left corner");
    assert!(screen.contains("[0] Revisões abertas"));
}

#[test]
fn list_panel_shows_the_table_header() {
    let picker = state(vec![item(1, "dev", "main", false, false)], 0);

    let screen = screen(&draw(&picker));

    assert!(screen.contains("PR"));
    assert!(screen.contains("TÍTULO"));
    assert!(screen.contains("AUTOR"));
    assert!(screen.contains("BRANCH"));
    assert!(screen.contains("QUANDO"));
}

#[test]
fn item_rows_share_the_same_author_column_start() {
    let picker = state(
        vec![
            item(1, "ann", "main", false, false),
            item(2, "longname", "other", false, false),
        ],
        0,
    );

    let row_lines = lines(&draw(&picker));
    let row_one = row_lines
        .iter()
        .find(|line| line.contains("#1"))
        .expect("first row rendered");
    let row_two = row_lines
        .iter()
        .find(|line| line.contains("#2"))
        .expect("second row rendered");

    let author_one = char_offset(row_one, "@ann").expect("author one present");
    let author_two = char_offset(row_two, "@longname").expect("author two present");
    assert_eq!(author_one, author_two);
}

#[test]
fn narrow_terminal_hides_the_branch_column() {
    let picker = state(
        vec![item(1, "dev", "distinctive-branch-name", false, false)],
        0,
    );

    // Only inspect the list panel's rows: the description panel below it
    // shows the branch regardless of width, so scanning the whole screen
    // would give a false negative.
    let terminal = draw_sized(&picker, 60, 30);
    let list_lines: Vec<String> = screen_sized(&terminal, 60, 30)
        .lines()
        .take_while(|line| !line.contains("[1] Descrição da revisão"))
        .map(str::to_owned)
        .collect();
    let list_screen = list_lines.join("\n");

    assert!(!list_screen.contains("distinctive-branch-name"));
    assert!(list_screen.contains("@dev"));
}

#[test]
fn selected_row_gets_the_marker_and_the_selection_background() {
    let picker = state(
        vec![
            item(1, "dev", "main", false, false),
            item(2, "dev", "other", false, false),
        ],
        0,
    );

    let terminal = draw(&picker);
    let screen = screen(&terminal);
    let buffer = terminal.backend().buffer();

    assert!(screen.contains("▶ #1"));
    assert!(!screen.contains("▶ #2"));

    let row_lines = lines(&terminal);
    let selected_row = row_lines
        .iter()
        .position(|line| line.contains("▶ #1"))
        .expect("selected row rendered");
    let marker_col = char_offset(&row_lines[selected_row], "▶").unwrap();
    // Somewhere past the marker, well inside the panel, the row's
    // background must be the selection color, spanning to the inner edge.
    let cell = buffer
        .cell(((marker_col + 40) as u16, selected_row as u16))
        .unwrap();
    assert_eq!(cell.bg, theme::SELECTION);

    // The unselected row carries a plain two-space indent, no marker.
    let unselected_row = row_lines
        .iter()
        .position(|line| line.contains("#2"))
        .expect("unselected row rendered");
    assert!(
        row_lines[unselected_row].starts_with("│   #2")
            || row_lines[unselected_row]
                .trim_start_matches('│')
                .starts_with("  #2")
    );
}

#[test]
fn panel_shows_the_open_review_counter() {
    let picker = state(
        vec![
            item(1, "dev", "main", false, false),
            item(2, "dev", "other", false, false),
        ],
        0,
    );

    let screen = screen(&draw(&picker));

    assert!(screen.contains("2 reviews abertos"));
}

#[test]
fn panel_shows_the_recent_cap_when_the_list_is_full() {
    let items: Vec<PickerItem> = (0..50)
        .map(|number| item(number, "dev", "main", false, false))
        .collect();
    let picker = state(items, 0);

    let screen = screen(&draw(&picker));

    assert!(screen.contains("50 mais recentes"));
}

#[test]
fn detail_panel_shows_the_highlighted_items_description() {
    let picker = state(
        vec![
            item_with_description(1, "dev", "main", "Body of the first review."),
            item_with_description(2, "dev", "other", "Body of the second review."),
        ],
        1,
    );

    let screen = screen(&draw(&picker));

    assert!(screen.contains("[1] Descrição da revisão"));
    assert!(screen.contains("Body of the second review."));
    assert!(!screen.contains("Body of the first review."));
}

#[test]
fn detail_panel_shows_an_empty_description_placeholder() {
    let picker = state(vec![item(1, "dev", "main", false, false)], 0);

    let screen = screen(&draw(&picker));

    assert!(screen.contains("sem descrição"));
}

#[test]
fn detail_panel_is_hidden_on_very_short_terminals() {
    let picker = state(
        vec![item_with_description(1, "dev", "main", "hidden body text")],
        0,
    );

    let terminal = draw_sized(&picker, 100, 13);
    let screen = screen_sized(&terminal, 100, 13);

    assert!(!screen.contains("[1] Descrição da revisão"));
    assert!(!screen.contains("hidden body text"));
}

#[test]
fn tab_moves_the_accent_border_to_the_focused_panel() {
    let mut picker = state(vec![item(1, "dev", "main", false, false)], 0);
    picker.focus_detail = true;

    let terminal = draw(&picker);
    let screen = screen(&terminal);
    let buffer = terminal.backend().buffer();

    let list_border_row = screen
        .lines()
        .position(|line| line.contains("[0] Revisões abertas"))
        .expect("list panel title rendered");
    let list_border_col = char_offset(screen.lines().nth(list_border_row).unwrap(), "╭").unwrap();
    let list_cell = buffer
        .cell((list_border_col as u16, list_border_row as u16))
        .unwrap();
    assert_eq!(list_cell.style().fg, Some(theme::BORDER));

    let detail_border_row = screen
        .lines()
        .position(|line| line.contains("[1] Descrição da revisão"))
        .expect("detail panel title rendered");
    let detail_border_col =
        char_offset(screen.lines().nth(detail_border_row).unwrap(), "╭").unwrap();
    let detail_cell = buffer
        .cell((detail_border_col as u16, detail_border_row as u16))
        .unwrap();
    assert_eq!(detail_cell.style().fg, Some(theme::ACCENT));
}

#[test]
fn status_line_shows_flat_hints_on_the_right_with_accent_keys() {
    let picker = state(vec![item(1, "dev", "main", false, false)], 0);

    let terminal = draw(&picker);
    let screen = screen(&terminal);
    let buffer = terminal.backend().buffer();

    let status_row = screen.lines().last().unwrap();
    assert!(status_row.contains("mover"));
    assert!(status_row.contains("foco"));
    assert!(status_row.contains("abrir"));
    assert!(status_row.contains("recarregar"));
    assert!(status_row.contains("sair"));

    let key_col = char_offset(status_row, "j/k").expect("j/k hint present");
    let cell = buffer.cell((key_col as u16, 29)).unwrap();
    assert_eq!(cell.style().fg, Some(theme::ACCENT));
    assert!(cell.style().add_modifier.contains(Modifier::BOLD));
}

#[test]
fn status_error_replaces_the_whole_line_in_danger() {
    let mut picker = state(vec![item(1, "dev", "main", false, false)], 0);
    picker.error_banner = Some("falha ao listar reviews".into());

    let terminal = draw(&picker);
    let screen = screen(&terminal);

    let status_row = screen.lines().last().unwrap();
    assert!(status_row.contains("falha ao listar reviews"));
}

#[test]
fn age_formats_minutes_hours_days() {
    let now = OffsetDateTime::UNIX_EPOCH + Duration::hours(1000);

    assert_eq!(age(now, now - Duration::seconds(30)), "agora");
    assert_eq!(age(now, now - Duration::minutes(5)), "5m");
    assert_eq!(age(now, now - Duration::hours(3)), "3h");
    assert_eq!(age(now, now - Duration::days(2)), "2d");
}
