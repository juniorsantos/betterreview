use std::collections::BTreeMap;

use betterreview::{
    domain::ChangeRequestSummary,
    tui::{
        picker::{PickerItem, PickerState, age, render},
        theme,
    },
};
use ratatui::{Terminal, backend::TestBackend};
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
    }
}

fn item(number: u64, author: &str, branch: &str, draft: bool, current_branch: bool) -> PickerItem {
    PickerItem {
        summary: summary(number, author, branch, draft),
        has_session: false,
        current_branch,
    }
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
    }
}

fn screen(picker: &PickerState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| render(frame, picker)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..30)
        .map(|y| {
            (0..100)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn renders_items_with_pin_and_metadata() {
    let picker = state(vec![item(42, "jsjunior", "feature/login", true, false)], 0);

    let screen = screen(&picker);

    assert!(screen.contains("#42"));
    assert!(screen.contains("@jsjunior"));
    assert!(screen.contains("feature/login"));
    assert!(screen.contains("[draft]"));
}

#[test]
fn renders_the_pin_marker_for_the_current_branch_item() {
    let picker = state(
        vec![
            item(42, "jsjunior", "feature/login", false, true),
            item(43, "dev", "other", false, false),
        ],
        0,
    );

    let screen = screen(&picker);
    let pinned_row = screen
        .lines()
        .find(|line| line.contains("#42"))
        .expect("pinned row rendered");

    assert!(pinned_row.contains('●'));
}

#[test]
fn highlight_covers_the_full_line() {
    let picker = state(vec![item(1, "dev", "main", false, false)], 0);

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| render(frame, &picker)).unwrap();
    let buffer = terminal.backend().buffer();

    assert_eq!(
        buffer.cell((97, 1)).unwrap().style().bg,
        Some(theme::CURSOR_LINE)
    );
}

#[test]
fn age_formats_minutes_hours_days() {
    let now = OffsetDateTime::UNIX_EPOCH + Duration::hours(1000);

    assert_eq!(age(now, now - Duration::seconds(30)), "agora");
    assert_eq!(age(now, now - Duration::minutes(5)), "5m");
    assert_eq!(age(now, now - Duration::hours(3)), "3h");
    assert_eq!(age(now, now - Duration::days(2)), "2d");
}
