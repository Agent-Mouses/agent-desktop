use agent_desktop_core::{
    adapter::WindowFilter,
    node::{AppInfo, WindowInfo},
};
use std::time::Duration;

pub(crate) fn visible_apps() -> Vec<AppInfo> {
    let mut seen_pids = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for dict in cg_window::window_dictionaries() {
        let Some(layer) = cg_window::int_field(&dict, "kCGWindowLayer") else {
            continue;
        };
        if layer != 0 {
            continue;
        }

        let Some(pid) = cg_window::int_field(&dict, "kCGWindowOwnerPID").map(|pid| pid as i32)
        else {
            continue;
        };
        if pid <= 0 || !seen_pids.insert(pid) {
            continue;
        }

        let Some(name) = cg_window::string_field(&dict, "kCGWindowOwnerName") else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        apps.push(AppInfo {
            name,
            pid,
            bundle_id: None,
        });
    }

    apps
}

pub(crate) fn list_windows(
    filter: &WindowFilter,
    pid_for_app_name: impl Fn(&str) -> Option<i32>,
) -> Vec<WindowInfo> {
    for attempt in 0..3 {
        let windows = visible_windows_once(filter);
        if !windows.is_empty() || attempt == 2 {
            if !windows.is_empty() {
                return windows;
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    if let Some(app_name) = filter.app.as_deref() {
        pid_for_app_name(app_name)
            .and_then(|pid| ax_window_for_app(app_name, pid))
            .filter(|window| !filter.focused_only || window.is_focused)
            .into_iter()
            .collect()
    } else {
        Vec::new()
    }
}

fn visible_windows_once(filter: &WindowFilter) -> Vec<WindowInfo> {
    let app_filter = filter.app.as_deref().unwrap_or("").to_ascii_lowercase();
    let mut candidates = Vec::new();

    for dict in cg_window::window_dictionaries() {
        let Some(layer) = cg_window::int_field(&dict, "kCGWindowLayer") else {
            continue;
        };
        if layer != 0 {
            continue;
        }

        let Some(app_name) = cg_window::string_field(&dict, "kCGWindowOwnerName") else {
            continue;
        };
        if app_name.is_empty() || !matches_app_filter(&app_name, &app_filter) {
            continue;
        }

        let title = cg_window::string_field(&dict, "kCGWindowName")
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| app_name.clone());
        let pid = cg_window::int_field(&dict, "kCGWindowOwnerPID").unwrap_or(0) as i32;
        let window_number = cg_window::int_field(&dict, "kCGWindowNumber").unwrap_or(0);

        candidates.push((app_name, title, pid, window_number));
    }

    windows_from_candidates(candidates, filter.focused_only)
}

fn windows_from_candidates(
    candidates: Vec<(String, String, i32, i64)>,
    focused_only: bool,
) -> Vec<WindowInfo> {
    let mut title_counts = std::collections::HashMap::new();
    for (_, title, pid, _) in &candidates {
        *title_counts.entry((*pid, title.clone())).or_insert(0) += 1;
    }

    let mut focus_cache = std::collections::HashMap::new();
    let mut windows = Vec::new();
    let mut focused_seen = false;

    for (app_name, title, pid, window_number) in candidates {
        let title_count = title_counts
            .get(&(pid, title.clone()))
            .copied()
            .unwrap_or(0);
        let identity = focus_cache
            .entry(pid)
            .or_insert_with(|| focused_window_identity(pid));
        let is_focused =
            !focused_seen && matches_focused_window(&title, window_number, identity, title_count);
        if focused_only && !is_focused {
            continue;
        }
        focused_seen |= is_focused;

        windows.push(WindowInfo {
            id: format!("w-{window_number}"),
            title,
            app: app_name,
            pid,
            bounds: None,
            is_focused,
        });
    }

    windows
}

#[cfg(test)]
fn apps_from_windows(windows: &[WindowInfo]) -> Vec<AppInfo> {
    let mut seen_pids = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for window in windows {
        if window.pid > 0 && seen_pids.insert(window.pid) {
            apps.push(AppInfo {
                name: window.app.clone(),
                pid: window.pid,
                bundle_id: None,
            });
        }
    }

    apps
}

fn matches_app_filter(app_name: &str, app_filter: &str) -> bool {
    app_filter.is_empty() || app_name.to_ascii_lowercase().contains(app_filter)
}

fn ax_window_for_app(app_name: &str, pid: i32) -> Option<WindowInfo> {
    let app = crate::tree::element_for_pid(pid);
    let window = crate::tree::copy_element_attr(&app, "AXFocusedWindow")
        .or_else(|| crate::tree::copy_element_attr(&app, "AXMainWindow"))
        .or_else(|| {
            crate::tree::copy_ax_array(&app, "AXWindows")
                .and_then(|windows| windows.into_iter().next())
        })?;
    if crate::tree::copy_string_attr(&window, "AXRole").as_deref() != Some("AXWindow") {
        return None;
    }
    let title =
        crate::tree::copy_string_attr(&window, "AXTitle").unwrap_or_else(|| app_name.into());
    let window_number = crate::tree::copy_i64_attr(&window, "AXWindowNumber").unwrap_or(0);
    let is_focused = crate::tree::copy_bool_attr(&app, "AXFrontmost") == Some(true);
    Some(WindowInfo {
        id: format!("w-{window_number}"),
        title,
        app: app_name.to_string(),
        pid,
        bounds: None,
        is_focused,
    })
}

type FocusedWindowIdentity = Option<(Option<String>, Option<i64>)>;

fn focused_window_identity(pid: i32) -> FocusedWindowIdentity {
    let app = crate::tree::element_for_pid(pid);
    if crate::tree::copy_bool_attr(&app, "AXFrontmost") != Some(true) {
        return None;
    }
    let window = crate::tree::copy_element_attr(&app, "AXFocusedWindow")?;
    Some((
        crate::tree::copy_string_attr(&window, "AXTitle"),
        crate::tree::copy_i64_attr(&window, "AXWindowNumber"),
    ))
}

fn matches_focused_window(
    title: &str,
    window_number: i64,
    identity: &FocusedWindowIdentity,
    same_title_count: usize,
) -> bool {
    let Some((focused_title, focused_number)) = identity else {
        return false;
    };
    if let Some(number) = focused_number {
        return *number == window_number;
    }
    focused_title.as_deref() == Some(title) && same_title_count == 1
}

mod cg_window {
    use core_foundation::{base::CFType, dictionary::CFDictionary, string::CFString};

    type WindowDictionary = CFDictionary<CFString, CFType>;

    pub(super) fn window_dictionaries() -> Vec<WindowDictionary> {
        use crate::cf_type::borrowed_cf_dictionary;
        use core_graphics::display::CGDisplay;
        use core_graphics::window::{
            kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
        };

        let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let Some(array) = CGDisplay::window_list_info(options, None) else {
            return Vec::new();
        };

        array
            .get_all_values()
            .into_iter()
            .filter_map(|raw| borrowed_cf_dictionary(raw as core_foundation::base::CFTypeRef))
            .collect()
    }

    pub(super) fn int_field(dict: &WindowDictionary, key: &str) -> Option<i64> {
        use crate::cf_type::borrowed_cf_number;
        use core_foundation::base::TCFType;

        let key = CFString::new(key);
        dict.find(&key)
            .and_then(|value| borrowed_cf_number(value.as_concrete_TypeRef()))
            .and_then(|number| number.to_i64())
    }

    pub(super) fn string_field(dict: &WindowDictionary, key: &str) -> Option<String> {
        use crate::cf_type::borrowed_cf_string;
        use core_foundation::base::TCFType;

        let key = CFString::new(key);
        dict.find(&key)
            .and_then(|value| borrowed_cf_string(value.as_concrete_TypeRef()))
            .map(|value| value.to_string())
    }
}

#[cfg(test)]
#[path = "window_inventory_tests.rs"]
mod tests;
