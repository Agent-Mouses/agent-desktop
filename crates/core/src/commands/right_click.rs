use crate::{
    action::{Action, ActionRequest},
    adapter::{PlatformAdapter, SnapshotSurface, TreeOptions, WindowFilter},
    commands::helpers::{RefArgs, resolve_ref},
    error::AppError,
    refs::RefEntry,
    snapshot,
};
use serde_json::{Value, json};

pub fn execute(args: RefArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, handle) = resolve_ref(&args.ref_id, args.snapshot_id.as_deref(), adapter)?;
    let result =
        adapter.execute_action(handle.handle(), ActionRequest::headless(Action::RightClick))?;
    let mut response = serde_json::to_value(&result)?;

    std::thread::sleep(std::time::Duration::from_millis(200));

    let opts = TreeOptions {
        interactive_only: true,
        surface: SnapshotSurface::Menu,
        ..Default::default()
    };
    let probe_app = probe_app_name(adapter, &entry);
    match snapshot::run(adapter, &opts, probe_app.as_deref(), None) {
        Ok(snap) => match serde_json::to_value(&snap.tree) {
            Ok(menu_json) => {
                response["menu"] = menu_json;
                if let Some(snapshot_id) = snap.snapshot_id {
                    response["menu_snapshot_id"] = json!(snapshot_id);
                }
            }
            Err(err) => {
                response["menu_probe"] = json!({
                    "ok": false,
                    "error": {
                        "code": "INTERNAL",
                        "message": err.to_string(),
                    }
                })
            }
        },
        Err(err) => response["menu_probe"] = probe_error_json(&err),
    }

    Ok(response)
}

fn probe_app_name(adapter: &dyn PlatformAdapter, entry: &RefEntry) -> Option<String> {
    if entry.source_app.is_some() {
        return entry.source_app.clone();
    }
    adapter
        .list_windows(&WindowFilter {
            focused_only: false,
            app: None,
        })
        .ok()
        .and_then(|windows| {
            windows
                .into_iter()
                .find(|window| window.pid == entry.pid)
                .map(|window| window.app)
        })
}

fn probe_error_json(err: &AppError) -> Value {
    if err.code() == "ELEMENT_NOT_FOUND" {
        return json!({
            "ok": false,
            "error": {
                "code": "ELEMENT_NOT_FOUND",
                "message": "Right-click action was accepted, but no menu accessibility tree was exposed for capture.",
                "suggestion": "Use 'snapshot --surface menu' only when the app exposes the context menu through accessibility."
            }
        });
    }

    let mut error = json!({
        "code": err.code(),
        "message": err.to_string(),
    });
    if let Some(suggestion) = err.suggestion() {
        error["suggestion"] = json!(suggestion);
    }
    json!({
        "ok": false,
        "error": error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        action::ActionResult,
        adapter::NativeHandle,
        error::{AdapterError, ErrorCode},
        node::WindowInfo,
        refs::{RefEntry, RefMap},
        refs_store::RefStore,
        refs_test_support::HomeGuard,
    };

    struct ProbeFailingAdapter {
        tree_error: Option<ErrorCode>,
    }

    impl PlatformAdapter for ProbeFailingAdapter {
        fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
            Ok(NativeHandle::null())
        }

        fn execute_action(
            &self,
            _handle: &NativeHandle,
            _request: ActionRequest,
        ) -> Result<ActionResult, AdapterError> {
            Ok(ActionResult::new("right_click"))
        }

        fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
            if filter.app.is_some() && self.tree_error.is_none() {
                return Err(AdapterError::new(
                    ErrorCode::WindowNotFound,
                    "menu probe failed",
                ));
            }
            if filter.focused_only {
                return Err(AdapterError::new(
                    ErrorCode::WindowNotFound,
                    "no focused menu",
                ));
            }
            Ok(vec![WindowInfo {
                id: "w1".into(),
                title: "Main".into(),
                app: "TargetApp".into(),
                pid: 7,
                bounds: None,
                is_focused: true,
            }])
        }

        fn get_tree(
            &self,
            _win: &WindowInfo,
            _opts: &TreeOptions,
        ) -> Result<crate::node::AccessibilityNode, AdapterError> {
            if let Some(code) = self.tree_error.clone() {
                return Err(AdapterError::new(code, "menu tree unavailable"));
            }
            Ok(crate::node::AccessibilityNode {
                ref_id: None,
                role: "menu".into(),
                name: None,
                value: None,
                description: None,
                hint: None,
                states: Vec::new(),
                available_actions: Vec::new(),
                bounds: None,
                children_count: None,
                children: Vec::new(),
            })
        }
    }

    fn save_refmap(source_app: Option<String>) -> String {
        let mut refmap = RefMap::new();
        refmap.allocate(RefEntry {
            pid: 7,
            role: "button".into(),
            name: Some("Open".into()),
            value: None,
            states: Vec::new(),
            bounds: None,
            bounds_hash: None,
            available_actions: vec!["RightClick".into()],
            source_app,
            source_window_title: None,
            root_ref: None,
            path: smallvec::SmallVec::new(),
        });
        RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
    }

    #[test]
    fn returns_action_success_when_menu_probe_fails() {
        let _guard = HomeGuard::new();
        let snapshot_id = save_refmap(None);

        let value = execute(
            RefArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id),
            },
            &ProbeFailingAdapter { tree_error: None },
        )
        .unwrap();

        assert_eq!(value["action"], "right_click");
        assert_eq!(value["menu_probe"]["ok"], false);
        assert_eq!(value["menu_probe"]["error"]["code"], "WINDOW_NOT_FOUND");
    }

    #[test]
    fn element_not_found_menu_probe_uses_right_click_specific_guidance() {
        let _guard = HomeGuard::new();
        let snapshot_id = save_refmap(Some("TargetApp".into()));

        let value = execute(
            RefArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id),
            },
            &ProbeFailingAdapter {
                tree_error: Some(ErrorCode::ElementNotFound),
            },
        )
        .unwrap();

        assert_eq!(value["action"], "right_click");
        assert_eq!(value["menu_probe"]["ok"], false);
        assert_eq!(value["menu_probe"]["error"]["code"], "ELEMENT_NOT_FOUND");
        assert!(
            value["menu_probe"]["error"]["suggestion"]
                .as_str()
                .unwrap()
                .contains("snapshot --surface menu")
        );
    }
}
