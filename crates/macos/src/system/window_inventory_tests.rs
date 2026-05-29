use super::*;

fn window(app: &str, pid: i32) -> WindowInfo {
    WindowInfo {
        id: format!("w-{pid}"),
        title: app.to_string(),
        app: app.to_string(),
        pid,
        bounds: None,
        is_focused: false,
    }
}

#[test]
fn apps_from_windows_deduplicates_by_pid() {
    let apps = apps_from_windows(&[window("Finder", 10), window("Finder", 10)]);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Finder");
}

#[test]
fn apps_from_windows_keeps_same_name_with_distinct_pids() {
    let apps = apps_from_windows(&[window("Preview", 10), window("Preview", 11)]);

    assert_eq!(apps.len(), 2);
}

#[test]
fn matches_app_filter_accepts_case_insensitive_substring() {
    assert!(matches_app_filter("Docker Desktop", "docker"));
    assert!(!matches_app_filter("Finder", "docker"));
}
