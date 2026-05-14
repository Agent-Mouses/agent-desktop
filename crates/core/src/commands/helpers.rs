use crate::{
    action::{ActionRequest, WindowOp},
    adapter::{PlatformAdapter, WindowFilter},
    commands::resolved_element::ResolvedElement,
    error::AppError,
    node::WindowInfo,
    refs::RefEntry,
    refs_store::RefStore,
    window_lookup,
};
use serde_json::{Value, json};

pub struct AppArgs {
    pub app: Option<String>,
}

pub struct RefArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
}

pub fn resolve_ref<'a>(
    ref_id: &str,
    snapshot_id: Option<&str>,
    adapter: &'a dyn PlatformAdapter,
) -> Result<(RefEntry, ResolvedElement<'a>), AppError> {
    validate_ref_id(ref_id)?;
    let store = RefStore::new()?;
    let refmap = store.load(snapshot_id).map_err(|e| {
        tracing::debug!("refmap load failed: {e}");
        AppError::stale_ref(ref_id)
    })?;
    let entry = refmap
        .get(ref_id)
        .ok_or_else(|| AppError::stale_ref(ref_id))?
        .clone();
    tracing::debug!(
        "resolve: {} -> pid={} role={} name={:?}",
        ref_id,
        entry.pid,
        entry.role,
        entry.name.as_deref().unwrap_or("(none)")
    );
    let handle = adapter.resolve_element(&entry)?;
    tracing::debug!("resolve: {} resolved successfully", ref_id);
    Ok((entry, ResolvedElement::new(adapter, handle)))
}

pub fn validate_ref_id(ref_id: &str) -> Result<(), AppError> {
    let valid = ref_id.starts_with("@e")
        && ref_id.len() >= 3
        && ref_id.len() <= 12
        && ref_id[2..].chars().all(|c| c.is_ascii_digit())
        && ref_id[2..].parse::<u32>().is_ok_and(|n| n > 0);
    if !valid {
        return Err(AppError::invalid_input(format!(
            "Invalid ref_id '{ref_id}': must match @e{{N}} where N is a positive integer"
        )));
    }
    Ok(())
}

pub fn resolve_app_pid(app: Option<&str>, adapter: &dyn PlatformAdapter) -> Result<i32, AppError> {
    if let Some(name) = app {
        let apps = adapter.list_apps()?;
        apps.into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.pid)
            .ok_or_else(|| AppError::invalid_input(format!("App '{name}' not found")))
    } else {
        let filter = WindowFilter {
            focused_only: true,
            app: None,
        };
        let windows = adapter.list_windows(&filter)?;
        windows
            .first()
            .map(|w| w.pid)
            .ok_or_else(|| AppError::invalid_input("No focused window. Use --app to specify."))
    }
}

pub fn execute_ref_action(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
) -> Result<Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, args.snapshot_id.as_deref(), adapter)?;
    let result = adapter.execute_action(handle.handle(), request)?;
    Ok(serde_json::to_value(result)?)
}

pub fn window_op_command(
    args: AppArgs,
    adapter: &dyn PlatformAdapter,
    op: WindowOp,
    response_key: &'static str,
) -> Result<Value, AppError> {
    let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
    let win = match find_window_for_pid(pid, adapter) {
        Ok(win) => win,
        Err(_) if matches!(op, WindowOp::Restore) => WindowInfo {
            id: String::new(),
            title: String::new(),
            app: args.app.unwrap_or_default(),
            pid,
            bounds: None,
            is_focused: false,
        },
        Err(err) => return Err(err),
    };
    adapter.window_op(&win, op)?;
    Ok(json!({ response_key: true }))
}

pub fn find_window_for_pid(
    pid: i32,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    window_lookup::find_window_for_pid(pid, adapter)
}

pub fn resolve_window_for_app(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    let pid = resolve_app_pid(app, adapter)?;
    find_window_for_pid(pid, adapter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, ActionResult, InteractionPolicy};
    use crate::adapter::NativeHandle;
    use crate::error::{AdapterError, ErrorCode};
    use crate::node::AppInfo;
    use crate::refs::RefMap;
    use crate::refs_test_support::HomeGuard;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct ReleaseCountingAdapter {
        releases: AtomicU32,
    }

    impl PlatformAdapter for ReleaseCountingAdapter {
        fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
            Ok(NativeHandle::null())
        }

        fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
            self.releases.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct RecordingAdapter {
        request: Mutex<Option<ActionRequest>>,
    }

    impl PlatformAdapter for RecordingAdapter {
        fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
            Ok(NativeHandle::null())
        }

        fn execute_action(
            &self,
            _handle: &NativeHandle,
            request: ActionRequest,
        ) -> Result<ActionResult, AdapterError> {
            *self.request.lock().unwrap() = Some(request);
            Ok(ActionResult::new("ok"))
        }
    }

    struct RestoreWithoutWindowAdapter {
        op_count: AtomicU32,
    }

    impl PlatformAdapter for RestoreWithoutWindowAdapter {
        fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
            Ok(vec![AppInfo {
                name: "TextEdit".into(),
                pid: 42,
                bundle_id: None,
            }])
        }

        fn list_windows(
            &self,
            _filter: &crate::adapter::WindowFilter,
        ) -> Result<Vec<WindowInfo>, AdapterError> {
            Err(AdapterError::new(ErrorCode::WindowNotFound, "no windows"))
        }

        fn window_op(&self, win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError> {
            assert_eq!(win.pid, 42);
            assert!(matches!(op, WindowOp::Restore));
            self.op_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn entry() -> RefEntry {
        RefEntry {
            pid: 1,
            role: "button".into(),
            name: Some("OK".into()),
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec!["Click".into()],
            source_app: None,
            source_window_title: None,
            root_ref: None,
            path: smallvec::SmallVec::new(),
        }
    }

    #[test]
    fn resolved_element_releases_handle_once_on_drop() {
        let _guard = HomeGuard::new();
        let mut refmap = RefMap::new();
        refmap.allocate(entry());
        let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
        let adapter = ReleaseCountingAdapter {
            releases: AtomicU32::new(0),
        };

        {
            let (_entry, resolved) = resolve_ref("@e1", Some(&snapshot_id), &adapter).unwrap();
            let _handle = resolved.handle();
            assert_eq!(adapter.releases.load(Ordering::SeqCst), 0);
        }

        assert_eq!(adapter.releases.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn execute_ref_action_preserves_action_and_policy() {
        let _guard = HomeGuard::new();
        let mut refmap = RefMap::new();
        refmap.allocate(entry());
        let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
        let adapter = RecordingAdapter {
            request: Mutex::new(None),
        };
        let args = RefArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
        };

        execute_ref_action(args, &adapter, ActionRequest::headless(Action::Clear)).unwrap();

        let request = adapter.request.lock().unwrap().clone().unwrap();
        assert!(matches!(request.action, Action::Clear));
        assert_eq!(request.policy, InteractionPolicy::headless());
    }

    #[test]
    fn restore_can_run_when_no_window_is_currently_listed() {
        let adapter = RestoreWithoutWindowAdapter {
            op_count: AtomicU32::new(0),
        };

        let value = window_op_command(
            AppArgs {
                app: Some("TextEdit".into()),
            },
            &adapter,
            WindowOp::Restore,
            "restored",
        )
        .unwrap();

        assert_eq!(value["restored"], true);
        assert_eq!(adapter.op_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_valid_refs() {
        assert!(validate_ref_id("@e1").is_ok());
        assert!(validate_ref_id("@e14").is_ok());
        assert!(validate_ref_id("@e999").is_ok());
    }

    #[test]
    fn test_invalid_refs() {
        assert!(validate_ref_id("@").is_err());
        assert!(validate_ref_id("e1").is_err());
        assert!(validate_ref_id("@e").is_err());
        assert!(validate_ref_id("@e0").is_err());
        assert!(validate_ref_id("@e0abc").is_err());
        assert!(validate_ref_id("1").is_err());
        assert!(validate_ref_id("").is_err());
    }
}
