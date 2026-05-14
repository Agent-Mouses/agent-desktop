use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    commands::{helpers::resolve_app_pid, helpers::validate_ref_id, search_text},
    error::{AppError, ErrorCode},
    node::AccessibilityNode,
    notification::NotificationFilter,
    refs::RefMap,
    refs_store::RefStore,
    snapshot,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub struct WaitArgs {
    pub ms: Option<u64>,
    pub element: Option<String>,
    pub snapshot_id: Option<String>,
    pub window: Option<String>,
    pub text: Option<String>,
    pub timeout_ms: u64,
    pub menu: bool,
    pub menu_closed: bool,
    pub notification: bool,
    pub app: Option<String>,
}

pub fn execute(args: WaitArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    validate_wait_mode(&args)?;

    if let Some(ms) = args.ms {
        std::thread::sleep(Duration::from_millis(ms));
        return Ok(json!({ "waited_ms": ms }));
    }

    if args.menu || args.menu_closed {
        let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
        let start = Instant::now();
        adapter
            .wait_for_menu(pid, args.menu, args.timeout_ms)
            .map_err(AppError::Adapter)?;
        let elapsed = start.elapsed().as_millis();
        return Ok(json!({ "found": true, "elapsed_ms": elapsed }));
    }

    if args.notification {
        return wait_for_notification(&args, adapter);
    }

    if let Some(ref_id) = args.element {
        validate_ref_id(&ref_id)?;
        return wait_for_element(ref_id, args.snapshot_id, args.timeout_ms, adapter);
    }

    if let Some(title) = args.window {
        return wait_for_window(title, args.timeout_ms, adapter);
    }

    if let Some(text) = args.text {
        return wait_for_text(text, args.app, args.timeout_ms, adapter);
    }

    Err(AppError::invalid_input(
        "Provide a duration (ms), --menu, --notification, --element <ref>, --window <title>, or --text <text>",
    ))
}

fn validate_wait_mode(args: &WaitArgs) -> Result<(), AppError> {
    let selected = [
        args.ms.is_some(),
        args.element.is_some(),
        args.window.is_some(),
        args.text.is_some() && !args.notification,
        args.menu,
        args.menu_closed,
        args.notification,
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if selected <= 1 {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "wait accepts exactly one mode",
        "Use one of: ms, --element, --window, --text, --menu, --menu-closed, or --notification.",
    ))
}

fn wait_for_element(
    ref_id: String,
    snapshot_id: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let store = RefStore::new()?;
    let fixed_refmap = match snapshot_id.as_deref() {
        Some(id) => Some(store.load_snapshot(id)?),
        None => None,
    };
    let mut latest_cache = if fixed_refmap.is_none() {
        Some(LatestRefCache::new(&store)?)
    } else {
        None
    };

    if fixed_refmap
        .as_ref()
        .is_some_and(|refmap| refmap.get(&ref_id).is_none())
    {
        return Err(AppError::invalid_input_with_suggestion(
            format!("Ref {ref_id} is not present in the requested snapshot"),
            "Use a ref returned by that snapshot_id, or omit --snapshot to wait against the latest refmap.",
        ));
    }

    loop {
        let entry = fixed_refmap
            .as_ref()
            .and_then(|r| r.get(&ref_id).cloned())
            .or_else(|| latest_cache.as_ref().and_then(|c| c.entry(&ref_id)));
        if let Some(entry) = entry {
            match adapter.resolve_element(&entry) {
                Ok(handle) => {
                    let _ = adapter.release_handle(&handle);
                    let elapsed = start.elapsed().as_millis();
                    return Ok(json!({ "found": true, "ref": ref_id, "elapsed_ms": elapsed }));
                }
                Err(err) if fixed_refmap.is_none() && err.code == ErrorCode::StaleRef => {
                    if let Some(cache) = latest_cache.as_mut() {
                        cache.refresh_if_due();
                    }
                }
                Err(_) => {}
            }
        } else if let Some(cache) = latest_cache.as_mut() {
            cache.refresh_if_due();
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!("Element {ref_id} not found within {timeout_ms}ms"),
            )));
        }
        std::thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

struct LatestRefCache<'a> {
    store: &'a RefStore,
    snapshot_id: Option<String>,
    refmap: RefMap,
    last_refresh: Instant,
}

impl<'a> LatestRefCache<'a> {
    fn new(store: &'a RefStore) -> Result<Self, AppError> {
        let snapshot_id = store.latest_snapshot_id();
        let refmap = if let Some(id) = snapshot_id.as_deref() {
            store.load_snapshot(id)?
        } else {
            store.load_latest()?
        };
        Ok(Self {
            store,
            snapshot_id,
            refmap,
            last_refresh: Instant::now() - Duration::from_millis(500),
        })
    }

    fn entry(&self, ref_id: &str) -> Option<crate::refs::RefEntry> {
        self.refmap.get(ref_id).cloned()
    }

    fn refresh_if_due(&mut self) {
        if self.last_refresh.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_refresh = Instant::now();
        if let Some(snapshot_id) = self.store.latest_snapshot_id() {
            if self.snapshot_id.as_deref() == Some(snapshot_id.as_str()) {
                return;
            }
            if let Ok(refmap) = self.store.load_snapshot(&snapshot_id) {
                self.snapshot_id = Some(snapshot_id);
                self.refmap = refmap;
            }
        } else if let Ok(refmap) = self.store.load_latest() {
            self.refmap = refmap;
            self.snapshot_id = self.store.latest_snapshot_id();
        }
    }
}

fn wait_for_window(
    title: String,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };

    loop {
        if let Ok(windows) = adapter.list_windows(&filter) {
            if let Some(win) = windows.into_iter().find(|w| w.title.contains(&title)) {
                let elapsed = start.elapsed().as_millis();
                return Ok(json!({ "found": true, "window": win, "elapsed_ms": elapsed }));
            }
        }

        if start.elapsed() >= timeout {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!("Window with title '{title}' not found within {timeout_ms}ms"),
            )));
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_text(
    text: String,
    app: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let opts = crate::adapter::TreeOptions::default();
    let normalized_text = search_text::normalize(&text);

    loop {
        if let Ok(result) = snapshot::build(adapter, &opts, app.as_deref(), None) {
            if let Some(found) = find_text_in_tree(&result.tree, &normalized_text) {
                let snapshot_id = RefStore::new()?.save_new_snapshot(&result.refmap)?;
                let elapsed = start.elapsed().as_millis();
                return Ok(json!({
                    "found": true,
                    "text": text,
                    "ref": found.ref_id,
                    "role": found.role,
                    "snapshot_id": snapshot_id,
                    "elapsed_ms": elapsed
                }));
            }
        }

        if start.elapsed() >= timeout {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!("Text '{text}' not found within {timeout_ms}ms"),
            )));
        }

        std::thread::sleep(Duration::from_millis(200));
    }
}

struct TextMatch {
    ref_id: Option<String>,
    role: String,
}

fn wait_for_notification(
    args: &WaitArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app: args.app.clone(),
        text: args.text.clone(),
        ..Default::default()
    };
    let baseline = adapter
        .list_notifications(&filter)
        .map_err(AppError::Adapter)?;
    let baseline_indices: std::collections::HashSet<usize> =
        baseline.iter().map(|n| n.index).collect();
    let interval = Duration::from_millis(500);
    let deadline = Instant::now() + Duration::from_millis(args.timeout_ms);
    let start = Instant::now();

    loop {
        if Instant::now() > deadline {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!("No new notification within {}ms", args.timeout_ms),
            )));
        }
        let current = adapter
            .list_notifications(&filter)
            .map_err(AppError::Adapter)?;
        let Some(notif) = current
            .iter()
            .find(|n| !baseline_indices.contains(&n.index))
        else {
            std::thread::sleep(interval);
            continue;
        };
        let elapsed = start.elapsed().as_millis();
        return Ok(json!({
            "condition": "notification",
            "matched": true,
            "notification": notif,
            "elapsed_ms": elapsed,
        }));
    }
}

fn find_text_in_tree(node: &AccessibilityNode, text_lower: &str) -> Option<TextMatch> {
    if search_text::node_contains(node, text_lower) {
        return Some(TextMatch {
            ref_id: node.ref_id.clone(),
            role: node.role.clone(),
        });
    }

    for child in &node.children {
        if let Some(found) = find_text_in_tree(child, text_lower) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;
