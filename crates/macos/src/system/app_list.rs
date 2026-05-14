use agent_desktop_core::{
    adapter::WindowFilter,
    error::AdapterError,
    node::{AppInfo, WindowInfo},
};
#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
const PS_TIMEOUT: Duration = Duration::from_secs(2);

pub fn list_apps_impl() -> Result<Vec<AppInfo>, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        let mut apps = apps_from_cg_windows();
        let windows = crate::system::window_list::list_windows_impl(&WindowFilter {
            focused_only: false,
            app: None,
        })
        .unwrap_or_default();
        merge_apps(&mut apps, apps_from_windows(windows));

        merge_apps(&mut apps, running_apps_from_workspace());

        merge_apps(&mut apps, running_apps_from_process_table());

        apps.sort_by(|a, b| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
                .then_with(|| a.pid.cmp(&b.pid))
        });
        Ok(apps)
    }
    #[cfg(not(target_os = "macos"))]
    Err(AdapterError::not_supported("list_apps"))
}

pub fn apps_from_windows(windows: Vec<WindowInfo>) -> Vec<AppInfo> {
    let mut seen_pids = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for window in windows {
        if seen_pids.insert(window.pid) {
            apps.push(AppInfo {
                name: window.app,
                pid: window.pid,
                bundle_id: None,
            });
        }
    }

    apps
}

#[cfg(target_os = "macos")]
pub(crate) fn pid_for_app_name(app_name: &str) -> Option<i32> {
    let apps = app_sources();
    find_pid_in_apps(&apps, app_name)
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

#[cfg(target_os = "macos")]
fn app_sources() -> Vec<AppInfo> {
    let mut apps = apps_from_cg_windows();
    merge_apps(&mut apps, running_apps_from_workspace());
    merge_apps(&mut apps, running_apps_from_process_table());
    apps
}

#[cfg(target_os = "macos")]
fn running_apps_from_workspace() -> Vec<AppInfo> {
    use core_foundation::{base::TCFType, string::CFString};
    use std::ffi::c_void;

    type Id = *mut c_void;
    type Class = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        fn objc_getClass(name: *const core::ffi::c_char) -> Class;
        fn sel_registerName(name: *const core::ffi::c_char) -> Sel;
        fn objc_msgSend(receiver: Id, sel: Sel, ...) -> Id;
    }

    unsafe fn ns_string(id: Id) -> Option<String> {
        unsafe {
            if id.is_null() {
                return None;
            }
            Some(
                CFString::wrap_under_get_rule(id as core_foundation_sys::string::CFStringRef)
                    .to_string(),
            )
        }
    }

    unsafe {
        if !appkit_loaded() {
            return Vec::new();
        }

        let workspace_cls = objc_getClass(c"NSWorkspace".as_ptr());
        if workspace_cls.is_null() {
            return Vec::new();
        }

        let shared_sel = sel_registerName(c"sharedWorkspace".as_ptr());
        let send_class: unsafe extern "C" fn(Class, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let workspace = send_class(workspace_cls, shared_sel);
        if workspace.is_null() {
            return Vec::new();
        }

        let running_sel = sel_registerName(c"runningApplications".as_ptr());
        let send_id: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let running = send_id(workspace, running_sel);
        if running.is_null() {
            return Vec::new();
        }

        let count_sel = sel_registerName(c"count".as_ptr());
        let send_count: unsafe extern "C" fn(Id, Sel) -> usize =
            std::mem::transmute(objc_msgSend as *const c_void);
        let count = send_count(running, count_sel);

        let object_sel = sel_registerName(c"objectAtIndex:".as_ptr());
        let send_object: unsafe extern "C" fn(Id, Sel, usize) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let policy_sel = sel_registerName(c"activationPolicy".as_ptr());
        let send_policy: unsafe extern "C" fn(Id, Sel) -> isize =
            std::mem::transmute(objc_msgSend as *const c_void);
        let pid_sel = sel_registerName(c"processIdentifier".as_ptr());
        let send_pid: unsafe extern "C" fn(Id, Sel) -> i32 =
            std::mem::transmute(objc_msgSend as *const c_void);
        let name_sel = sel_registerName(c"localizedName".as_ptr());
        let bundle_sel = sel_registerName(c"bundleIdentifier".as_ptr());

        let mut seen_pids = std::collections::HashSet::new();
        let mut apps = Vec::new();
        for idx in 0..count {
            let app = send_object(running, object_sel, idx);
            if app.is_null() || send_policy(app, policy_sel) != 0 {
                continue;
            }

            let pid = send_pid(app, pid_sel);
            if pid <= 0 || !seen_pids.insert(pid) {
                continue;
            }

            let name = ns_string(send_id(app, name_sel));
            if let Some(name) = name {
                apps.push(AppInfo {
                    name,
                    pid,
                    bundle_id: ns_string(send_id(app, bundle_sel)),
                });
            }
        }

        apps
    }
}

#[cfg(target_os = "macos")]
fn appkit_loaded() -> bool {
    use std::ffi::c_void;

    type Id = *mut c_void;

    unsafe extern "C" {
        fn dlopen(filename: *const core::ffi::c_char, flag: i32) -> Id;
    }

    static APPKIT_LOADED: OnceLock<bool> = OnceLock::new();
    *APPKIT_LOADED.get_or_init(|| unsafe {
        !dlopen(
            c"/System/Library/Frameworks/AppKit.framework/AppKit".as_ptr(),
            1,
        )
        .is_null()
    })
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

#[cfg(target_os = "macos")]
fn apps_from_cg_windows() -> Vec<AppInfo> {
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        dictionary::CFDictionary,
        string::CFString,
    };

    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, window_id: u32) -> CFTypeRef;
    }

    let info_ref = unsafe { CGWindowListCopyWindowInfo(17, 0) };
    if info_ref.is_null() {
        return Vec::new();
    }

    let array = unsafe { CFArray::<CFType>::wrap_under_create_rule(info_ref as _) };
    let mut seen_pids = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for item in array.iter() {
        let dict = unsafe {
            CFDictionary::<CFString, CFType>::wrap_under_get_rule(item.as_concrete_TypeRef() as _)
        };
        let Some(layer) = cg_int_field(&dict, "kCGWindowLayer") else {
            continue;
        };
        if layer != 0 {
            continue;
        }
        let Some(pid) = cg_int_field(&dict, "kCGWindowOwnerPID").map(|p| p as i32) else {
            continue;
        };
        if !seen_pids.insert(pid) {
            continue;
        }
        let Some(name) = cg_string_field(&dict, "kCGWindowOwnerName") else {
            continue;
        };
        apps.push(AppInfo {
            name,
            pid,
            bundle_id: None,
        });
    }

    apps
}

#[cfg(target_os = "macos")]
fn cg_int_field(
    dict: &core_foundation::dictionary::CFDictionary<
        core_foundation::string::CFString,
        core_foundation::base::CFType,
    >,
    key: &str,
) -> Option<i64> {
    let key = core_foundation::string::CFString::new(key);
    dict.find(&key).and_then(|value| {
        let number = unsafe {
            core_foundation::number::CFNumber::wrap_under_get_rule(value.as_concrete_TypeRef() as _)
        };
        number.to_i64()
    })
}

#[cfg(target_os = "macos")]
fn cg_string_field(
    dict: &core_foundation::dictionary::CFDictionary<
        core_foundation::string::CFString,
        core_foundation::base::CFType,
    >,
    key: &str,
) -> Option<String> {
    let key = core_foundation::string::CFString::new(key);
    dict.find(&key).map(|value| unsafe {
        core_foundation::string::CFString::wrap_under_get_rule(value.as_concrete_TypeRef() as _)
            .to_string()
    })
}

#[cfg(target_os = "macos")]
fn running_apps_from_process_table() -> Vec<AppInfo> {
    let mut command = std::process::Command::new("/bin/ps");
    command.args(["-axo", "pid=,comm="]);
    let output = match crate::system::process::run_with_timeout(&mut command, "ps", PS_TIMEOUT) {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut seen_pids = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for line in text.lines() {
        let line = line.trim_start();
        let mut fields = line.splitn(2, char::is_whitespace);
        let Some(pid_text) = fields.next() else {
            continue;
        };
        let Some(command) = fields.next().map(str::trim) else {
            continue;
        };
        let Ok(pid) = pid_text.parse::<i32>() else {
            continue;
        };
        let Some(name) = app_name_from_command(command) else {
            continue;
        };
        if seen_pids.insert(pid) {
            apps.push(AppInfo {
                name,
                pid,
                bundle_id: None,
            });
        }
    }

    apps
}

#[cfg(target_os = "macos")]
fn app_name_from_command(command: &str) -> Option<String> {
    if command.contains("/Contents/Frameworks/")
        || command.contains("/Contents/PlugIns/")
        || command.contains("/XPCServices/")
        || command.contains(".appex/")
    {
        return None;
    }

    let marker = ".app/Contents/MacOS";
    let marker_start = command.find(marker)?;
    let app_path = &command[..marker_start + ".app".len()];
    let app_name = app_path.rsplit('/').next()?.strip_suffix(".app")?;
    if app_name.is_empty() {
        None
    } else {
        Some(app_name.to_string())
    }
}

#[cfg(test)]
#[path = "app_list_tests.rs"]
mod tests;
