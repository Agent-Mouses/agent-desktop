use super::*;

fn app(name: &str, pid: i32) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid,
        bundle_id: None,
    }
}

fn app_with_bundle(name: &str, pid: i32, bundle_id: &str) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid,
        bundle_id: Some(bundle_id.to_string()),
    }
}

#[test]
fn merge_apps_does_not_duplicate_same_pid_with_different_name() {
    let mut apps = vec![app("Preview", 42)];

    merge_apps(&mut apps, vec![app("Preview Helper", 42)]);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Preview");
}

#[test]
fn merge_apps_adds_bundle_id_for_existing_pid() {
    let mut apps = vec![app("Preview", 42)];

    merge_apps(
        &mut apps,
        vec![app_with_bundle("Preview Helper", 42, "com.apple.Preview")],
    );

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].bundle_id.as_deref(), Some("com.apple.Preview"));
}

#[test]
fn merge_apps_keeps_distinct_pids_with_same_name() {
    let mut apps = vec![app("Terminal", 10)];

    merge_apps(&mut apps, vec![app("Terminal", 11)]);

    assert_eq!(apps.len(), 2);
    assert_eq!(apps[1].pid, 11);
}

#[test]
fn find_pid_in_apps_prefers_exact_case_insensitive_match() {
    let apps = vec![app("Finder Helper", 10), app("Finder", 11)];

    assert_eq!(find_pid_in_apps(&apps, "finder"), Some(11));
}

#[test]
fn find_pid_in_apps_falls_back_to_contains_match() {
    let apps = vec![app("Preview", 10), app("Docker Desktop", 11)];

    assert_eq!(find_pid_in_apps(&apps, "Docker"), Some(11));
}

#[test]
fn sort_apps_orders_by_name_then_pid() {
    let mut apps = vec![app("Terminal", 3), app("Finder", 2), app("Finder", 1)];

    sort_apps(&mut apps);

    assert_eq!(
        apps.iter().map(|app| app.pid).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}
