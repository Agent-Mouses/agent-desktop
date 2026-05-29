use agent_desktop_core::{
    adapter::WindowFilter,
    node::{AppInfo, WindowInfo},
};

use crate::system::{process_apps, window_inventory, workspace_apps};

pub(crate) fn list_apps() -> Vec<AppInfo> {
    let mut apps = primary_apps();
    if apps.is_empty() {
        merge_apps(&mut apps, process_apps::list_apps());
    }
    sort_apps(&mut apps);
    apps
}

pub(crate) fn list_windows(filter: &WindowFilter) -> Vec<WindowInfo> {
    window_inventory::list_windows(filter, app_for_name)
}

pub(crate) fn pid_for_app_name(app_name: &str) -> Option<i32> {
    app_for_name(app_name).map(|app| app.pid)
}

pub(crate) fn app_for_name(app_name: &str) -> Option<AppInfo> {
    let apps = primary_apps();
    find_app_with_process_fallback(&apps, process_apps::list_apps(), app_name)
}

fn primary_apps() -> Vec<AppInfo> {
    merge_primary_sources(
        workspace_apps::list_apps(),
        window_inventory::visible_apps(),
    )
}

fn merge_primary_sources(workspace: Vec<AppInfo>, visible: Vec<AppInfo>) -> Vec<AppInfo> {
    let mut apps = workspace;
    merge_apps(&mut apps, visible);
    apps
}

fn find_app_with_process_fallback(
    primary: &[AppInfo],
    process: Vec<AppInfo>,
    app_name: &str,
) -> Option<AppInfo> {
    find_app_in_apps(primary, app_name).or_else(|| find_app_in_apps(&process, app_name))
}

fn merge_apps(apps: &mut Vec<AppInfo>, incoming: Vec<AppInfo>) {
    let mut seen_pids = apps
        .iter()
        .map(|app| app.pid)
        .collect::<std::collections::HashSet<_>>();

    for app in incoming {
        if seen_pids.insert(app.pid) {
            apps.push(app);
        } else if let Some(existing) = apps.iter_mut().find(|existing| existing.pid == app.pid) {
            if existing.bundle_id.is_none() {
                existing.bundle_id = app.bundle_id;
            }
        }
    }
}

fn sort_apps(apps: &mut [AppInfo]) {
    apps.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then_with(|| a.pid.cmp(&b.pid))
    });
}

fn find_app_in_apps(apps: &[AppInfo], app_name: &str) -> Option<AppInfo> {
    apps.iter()
        .find(|app| app.name.eq_ignore_ascii_case(app_name))
        .cloned()
}

#[cfg(test)]
#[path = "app_inventory_tests.rs"]
mod tests;
