use agent_desktop_core::{
    adapter::WindowFilter,
    node::{AppInfo, WindowInfo},
};

use crate::system::{process_apps, window_inventory, workspace_apps};

pub(crate) fn list_apps() -> Vec<AppInfo> {
    let mut apps = workspace_apps::list_apps();
    merge_apps(&mut apps, window_inventory::visible_apps());

    if apps.is_empty() {
        merge_apps(&mut apps, process_apps::list_apps());
    }

    sort_apps(&mut apps);
    apps
}

pub(crate) fn list_windows(filter: &WindowFilter) -> Vec<WindowInfo> {
    window_inventory::list_windows(filter, pid_for_app_name)
}

pub(crate) fn pid_for_app_name(app_name: &str) -> Option<i32> {
    let mut apps = workspace_apps::list_apps();
    merge_apps(&mut apps, window_inventory::visible_apps());

    if apps.is_empty() {
        merge_apps(&mut apps, process_apps::list_apps());
    }

    find_pid_in_apps(&apps, app_name)
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

fn find_pid_in_apps(apps: &[AppInfo], app_name: &str) -> Option<i32> {
    let wanted = app_name.to_ascii_lowercase();
    apps.iter()
        .find(|app| app.name.eq_ignore_ascii_case(app_name))
        .or_else(|| {
            apps.iter()
                .find(|app| app.name.to_ascii_lowercase().contains(&wanted))
        })
        .map(|app| app.pid)
}

#[cfg(test)]
#[path = "app_inventory_tests.rs"]
mod tests;
