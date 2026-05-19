use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    error::{AdapterError, AppError, ErrorCode},
    node::WindowInfo,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const FOCUS_SETTLE_TIMEOUT_MS: u64 = 750;
const FOCUS_POLL_INTERVAL_MS: u64 = 50;
const FOCUS_CONFIRMATIONS: u8 = 2;

pub struct FocusWindowArgs {
    pub window_id: Option<String>,
    pub app: Option<String>,
    pub title: Option<String>,
}

pub fn execute(args: FocusWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let filter = WindowFilter {
        focused_only: false,
        app: args.app.clone(),
    };
    let windows = adapter.list_windows(&filter)?;

    let window = if let Some(id) = &args.window_id {
        windows.into_iter().find(|w| &w.id == id)
    } else if let Some(title) = &args.title {
        windows
            .into_iter()
            .find(|w| w.title.contains(title.as_str()))
    } else if let Some(app) = &args.app {
        windows
            .into_iter()
            .find(|w| w.app.eq_ignore_ascii_case(app))
    } else {
        return Err(AppError::invalid_input(
            "Provide --window-id, --app, or --title to identify the window",
        ));
    };

    let window = window.ok_or_else(|| {
        AppError::Adapter(
            crate::error::AdapterError::new(
                crate::error::ErrorCode::WindowNotFound,
                "No matching window found",
            )
            .with_suggestion("Run 'list-windows' to see available windows and their IDs."),
        )
    })?;

    let window_id = window.id.clone();
    adapter.focus_window(&window)?;
    let focused = wait_for_focused_window(adapter, &window_id, args.app)?;
    Ok(json!({ "focused": focused }))
}

fn wait_for_focused_window(
    adapter: &dyn PlatformAdapter,
    window_id: &str,
    app: Option<String>,
) -> Result<WindowInfo, AppError> {
    wait_for_focused_window_with_poll_interval(
        adapter,
        window_id,
        app.as_deref(),
        Duration::from_millis(FOCUS_POLL_INTERVAL_MS),
    )
}

fn wait_for_focused_window_with_poll_interval(
    adapter: &dyn PlatformAdapter,
    window_id: &str,
    app: Option<&str>,
    poll_interval: Duration,
) -> Result<WindowInfo, AppError> {
    let deadline = Instant::now() + Duration::from_millis(FOCUS_SETTLE_TIMEOUT_MS);
    let mut confirmations = 0;
    loop {
        match observed_focused_window(adapter, app)? {
            Some(window) if window.id == window_id => {
                confirmations += 1;
                if confirmations >= FOCUS_CONFIRMATIONS {
                    return Ok(window);
                }
            }
            _ => {
                confirmations = 0;
            }
        }

        if Instant::now() >= deadline {
            return Err(AppError::Adapter(
                AdapterError::new(
                    ErrorCode::ActionFailed,
                    "Window focus did not settle on the requested window",
                )
                .with_suggestion("Run 'list-windows' to refresh window IDs, then retry."),
            ));
        }

        if !poll_interval.is_zero() {
            std::thread::sleep(poll_interval);
        }
    }
}

fn observed_focused_window(
    adapter: &dyn PlatformAdapter,
    app: Option<&str>,
) -> Result<Option<WindowInfo>, AppError> {
    match adapter.focused_window() {
        Ok(window) => Ok(window),
        Err(err) if err.code == ErrorCode::PlatformNotSupported => adapter
            .list_windows(&WindowFilter {
                focused_only: true,
                app: app.map(str::to_string),
            })
            .map(|windows| windows.into_iter().next())
            .map_err(AppError::Adapter),
        Err(err) => Err(AppError::Adapter(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FocusAdapter {
        windows: Vec<WindowInfo>,
        focused_windows: Mutex<Vec<WindowInfo>>,
        focused_window_calls: Mutex<u32>,
        focused_window_supported: bool,
    }

    impl PlatformAdapter for FocusAdapter {
        fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
            if filter.focused_only {
                Ok(self.focused_windows.lock().unwrap().clone())
            } else {
                Ok(self.windows.clone())
            }
        }

        fn focus_window(&self, _win: &WindowInfo) -> Result<(), AdapterError> {
            Ok(())
        }

        fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError> {
            *self.focused_window_calls.lock().unwrap() += 1;
            if !self.focused_window_supported {
                return Err(AdapterError::not_supported("focused_window"));
            }
            let mut focused = self.focused_windows.lock().unwrap();
            if focused.len() > 1 {
                Ok(Some(focused.remove(0)))
            } else {
                Ok(focused.first().cloned())
            }
        }
    }

    fn window(id: &str, focused: bool) -> WindowInfo {
        WindowInfo {
            id: id.into(),
            title: "Main".into(),
            app: "TextEdit".into(),
            pid: 42,
            bounds: None,
            is_focused: focused,
        }
    }

    #[test]
    fn reports_focused_window_after_os_confirms_focus() {
        let target = window("w1", false);
        let adapter = FocusAdapter {
            windows: vec![target.clone()],
            focused_windows: Mutex::new(vec![window("w1", true)]),
            focused_window_calls: Mutex::new(0),
            focused_window_supported: true,
        };

        let value = execute(
            FocusWindowArgs {
                window_id: Some(target.id),
                app: None,
                title: None,
            },
            &adapter,
        )
        .unwrap();

        assert_eq!(value["focused"]["id"], "w1");
        assert_eq!(value["focused"]["is_focused"], true);
        assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 2);
    }

    #[test]
    fn errors_when_focus_does_not_settle_on_requested_window() {
        let target = window("w1", false);
        let adapter = FocusAdapter {
            windows: vec![target.clone()],
            focused_windows: Mutex::new(Vec::new()),
            focused_window_calls: Mutex::new(0),
            focused_window_supported: true,
        };

        let err = execute(
            FocusWindowArgs {
                window_id: Some(target.id),
                app: None,
                title: None,
            },
            &adapter,
        )
        .unwrap_err();

        assert_eq!(err.code(), "ACTION_FAILED");
    }

    #[test]
    fn falls_back_to_focused_window_list_when_direct_observation_is_unsupported() {
        let target = window("w1", false);
        let adapter = FocusAdapter {
            windows: vec![target.clone()],
            focused_windows: Mutex::new(vec![window("w1", true)]),
            focused_window_calls: Mutex::new(0),
            focused_window_supported: false,
        };

        let value = execute(
            FocusWindowArgs {
                window_id: Some(target.id),
                app: None,
                title: None,
            },
            &adapter,
        )
        .unwrap();

        assert_eq!(value["focused"]["id"], "w1");
        assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 2);
    }

    #[test]
    fn focus_confirmation_resets_after_transient_wrong_window() {
        let target = window("w1", false);
        let adapter = FocusAdapter {
            windows: vec![target.clone()],
            focused_windows: Mutex::new(vec![
                window("w1", true),
                window("w2", true),
                window("w1", true),
                window("w1", true),
            ]),
            focused_window_calls: Mutex::new(0),
            focused_window_supported: true,
        };

        let value =
            wait_for_focused_window_with_poll_interval(&adapter, &target.id, None, Duration::ZERO)
                .unwrap();

        assert_eq!(value.id, "w1");
        assert_eq!(*adapter.focused_window_calls.lock().unwrap(), 4);
    }
}
