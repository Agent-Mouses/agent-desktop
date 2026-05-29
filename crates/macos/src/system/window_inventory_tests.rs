use super::*;
use crate::system::cg_window::WindowRecord;

#[test]
fn apps_from_window_records_deduplicates_by_pid() {
    let apps = apps_from_window_records(&[
        record("Finder", 10, "Window 1", 1),
        record("Finder", 10, "Window 2", 2),
    ]);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Finder");
}

#[test]
fn apps_from_window_records_keeps_same_name_with_distinct_pids() {
    let apps = apps_from_window_records(&[
        record("Preview", 10, "Preview", 10),
        record("Preview", 11, "Preview", 11),
    ]);

    assert_eq!(apps.len(), 2);
}

#[test]
fn matches_app_filter_accepts_case_insensitive_substring() {
    assert!(matches_app_filter("Docker Desktop", "docker"));
    assert!(!matches_app_filter("Finder", "docker"));
}

#[test]
fn retry_empty_skips_known_missing_app_filter() {
    let filter = WindowFilter {
        app: Some("Missing".to_string()),
        focused_only: false,
    };

    assert!(!should_retry_empty(&filter, None));
}

#[test]
fn retry_empty_allows_known_app_filter_for_ax_race() {
    let filter = WindowFilter {
        app: Some("Mail".to_string()),
        focused_only: false,
    };

    assert!(should_retry_empty(&filter, Some(42)));
}

#[test]
fn windows_from_records_marks_single_focused_window_once() {
    let windows = windows_from_records_with_focus(
        vec![
            record("Mail", 10, "Inbox", 1),
            record("Mail", 10, "Inbox", 2),
        ],
        false,
        |_| Some((Some("Inbox".to_string()), Some(2))),
    );

    assert!(!windows[0].is_focused);
    assert!(windows[1].is_focused);
}

#[test]
fn windows_from_records_focus_only_filters_unfocused_windows() {
    let windows = windows_from_records_with_focus(
        vec![
            record("Mail", 10, "Inbox", 1),
            record("Mail", 10, "Sent", 2),
        ],
        true,
        |_| Some((Some("Sent".to_string()), Some(2))),
    );

    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].title, "Sent");
}

#[test]
fn matches_focused_window_uses_window_number_when_available() {
    let identity = Some((Some("Other".to_string()), Some(7)));

    assert!(matches_focused_window("Inbox", 7, &identity, 3));
    assert!(!matches_focused_window("Inbox", 8, &identity, 1));
}

#[test]
fn matches_focused_window_uses_unique_title_without_window_number() {
    let identity = Some((Some("Inbox".to_string()), None));

    assert!(matches_focused_window("Inbox", 0, &identity, 1));
    assert!(!matches_focused_window("Inbox", 0, &identity, 2));
    assert!(!matches_focused_window("Sent", 0, &identity, 1));
}

fn record(app_name: &str, pid: i32, title: &str, window_number: i64) -> WindowRecord {
    WindowRecord {
        app_name: app_name.to_string(),
        pid,
        title: Some(title.to_string()),
        window_number,
        area: 100.0,
    }
}
