use crate::{
    action::ElementState, adapter::PlatformAdapter, commands::helpers::resolve_ref,
    error::AppError, refs::RefEntry,
};
use serde_json::{Value, json};

pub struct IsArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub property: IsProperty,
}

pub enum IsProperty {
    Visible,
    Enabled,
    Checked,
    Focused,
    Expanded,
}

/// State is read live when the platform supports it, then falls back to snapshot state.
pub fn execute(args: IsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, handle) = resolve_ref(&args.ref_id, args.snapshot_id.as_deref(), adapter)?;
    let state = adapter
        .get_live_state(handle.handle())
        .ok()
        .flatten()
        .unwrap_or_else(|| state_from_ref_entry(&entry));

    let prop_name = match args.property {
        IsProperty::Visible => "visible",
        IsProperty::Enabled => "enabled",
        IsProperty::Checked => "checked",
        IsProperty::Focused => "focused",
        IsProperty::Expanded => "expanded",
    };

    let applicable = is_applicable(&args.property, &entry, &state);

    let result = match args.property {
        IsProperty::Visible => !has_state(&state, "hidden"),
        IsProperty::Enabled => !has_state(&state, "disabled"),
        IsProperty::Checked => has_state(&state, "checked"),
        IsProperty::Focused => has_state(&state, "focused"),
        IsProperty::Expanded => has_state(&state, "expanded"),
    };

    Ok(
        json!({ "property": prop_name, "ref": args.ref_id, "result": result, "applicable": applicable }),
    )
}

fn state_from_ref_entry(entry: &RefEntry) -> ElementState {
    ElementState {
        role: entry.role.clone(),
        states: entry.states.clone(),
        value: entry.value.clone(),
    }
}

fn has_state(state: &ElementState, name: &str) -> bool {
    state.states.iter().any(|s| s == name)
}

fn is_applicable(property: &IsProperty, entry: &RefEntry, state: &ElementState) -> bool {
    match property {
        IsProperty::Visible | IsProperty::Enabled | IsProperty::Focused => true,
        IsProperty::Checked => {
            crate::roles::is_toggleable_role(&entry.role)
                || has_state(state, "checked")
                || has_available_action(entry, "Toggle")
                || has_available_action(entry, "Check")
                || has_available_action(entry, "Uncheck")
        }
        IsProperty::Expanded => {
            crate::roles::is_expandable_role(&entry.role)
                || has_state(state, "expanded")
                || has_available_action(entry, "Expand")
                || has_available_action(entry, "Collapse")
        }
    }
}

fn has_available_action(entry: &RefEntry, action: &str) -> bool {
    entry.available_actions.iter().any(|a| a == action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::NativeHandle, error::AdapterError, refs::RefMap, refs_store::RefStore,
        refs_test_support::HomeGuard,
    };
    use std::sync::Mutex;

    struct LiveStateAdapter {
        state: Mutex<Option<ElementState>>,
    }

    impl PlatformAdapter for LiveStateAdapter {
        fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
            Ok(NativeHandle::null())
        }

        fn get_live_state(
            &self,
            _handle: &NativeHandle,
        ) -> Result<Option<ElementState>, AdapterError> {
            Ok(self.state.lock().unwrap().clone())
        }
    }

    fn save_entry(entry: RefEntry) -> String {
        let mut refmap = RefMap::new();
        refmap.allocate(entry);
        RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
    }

    fn entry(states: Vec<String>, value: Option<&str>, actions: Vec<&str>) -> RefEntry {
        RefEntry {
            pid: 1,
            role: "checkbox".into(),
            name: Some("Target".into()),
            value: value.map(str::to_string),
            states,
            bounds: None,
            bounds_hash: None,
            available_actions: actions.into_iter().map(str::to_string).collect(),
            source_app: None,
            source_window_title: None,
            root_ref: None,
            path: smallvec::SmallVec::new(),
        }
    }

    #[test]
    fn checked_uses_live_canonical_state() {
        let _guard = HomeGuard::new();
        let snapshot_id = save_entry(entry(vec![], None, vec!["Toggle"]));
        let adapter = LiveStateAdapter {
            state: Mutex::new(Some(ElementState {
                role: "checkbox".into(),
                states: vec!["checked".into()],
                value: Some("1".into()),
            })),
        };

        let result = execute(
            IsArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id),
                property: IsProperty::Checked,
            },
            &adapter,
        )
        .unwrap();

        assert_eq!(result["result"], true);
        assert_eq!(result["applicable"], true);
    }

    #[test]
    fn checked_does_not_infer_platform_values_in_core() {
        let _guard = HomeGuard::new();
        let snapshot_id = save_entry(entry(vec![], Some("1"), vec!["Toggle"]));
        let adapter = LiveStateAdapter {
            state: Mutex::new(None),
        };

        let result = execute(
            IsArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id),
                property: IsProperty::Checked,
            },
            &adapter,
        )
        .unwrap();

        assert_eq!(result["result"], false);
        assert_eq!(result["applicable"], true);
    }

    #[test]
    fn checked_falls_back_to_snapshot_state_when_live_state_is_missing() {
        let _guard = HomeGuard::new();
        let snapshot_id = save_entry(entry(vec!["checked".into()], None, vec!["Toggle"]));
        let adapter = LiveStateAdapter {
            state: Mutex::new(None),
        };

        let result = execute(
            IsArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id),
                property: IsProperty::Checked,
            },
            &adapter,
        )
        .unwrap();

        assert_eq!(result["result"], true);
        assert_eq!(result["applicable"], true);
    }

    #[test]
    fn basic_state_properties_use_live_state() {
        let _guard = HomeGuard::new();
        let snapshot_id = save_entry(entry(vec![], None, vec![]));
        let adapter = LiveStateAdapter {
            state: Mutex::new(Some(ElementState {
                role: "button".into(),
                states: vec!["focused".into(), "expanded".into()],
                value: None,
            })),
        };

        for (property, expected) in [
            (IsProperty::Visible, true),
            (IsProperty::Enabled, true),
            (IsProperty::Focused, true),
            (IsProperty::Expanded, true),
        ] {
            let result = execute(
                IsArgs {
                    ref_id: "@e1".into(),
                    snapshot_id: Some(snapshot_id.clone()),
                    property,
                },
                &adapter,
            )
            .unwrap();

            assert_eq!(result["result"], expected);
            assert_eq!(result["applicable"], true);
        }
    }

    #[test]
    fn action_availability_makes_toggle_and_expand_applicable() {
        let _guard = HomeGuard::new();
        let snapshot_id = save_entry(RefEntry {
            pid: 1,
            role: "cell".into(),
            name: Some("Disclosure".into()),
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec!["Check".into(), "Expand".into()],
            source_app: None,
            source_window_title: None,
            root_ref: None,
            path: smallvec::SmallVec::new(),
        });
        let adapter = LiveStateAdapter {
            state: Mutex::new(None),
        };

        for property in [IsProperty::Checked, IsProperty::Expanded] {
            let result = execute(
                IsArgs {
                    ref_id: "@e1".into(),
                    snapshot_id: Some(snapshot_id.clone()),
                    property,
                },
                &adapter,
            )
            .unwrap();

            assert_eq!(result["applicable"], true);
        }
    }
}
