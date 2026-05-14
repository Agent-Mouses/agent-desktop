use crate::{
    PermissionReport,
    adapter::PlatformAdapter,
    commands::permissions::{self, PermissionsArgs},
    error::AppError,
    refs_store::RefStore,
};
use serde_json::{Value, json};

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let report = adapter.permission_report();
    execute_with_report(adapter, &report)
}

pub fn execute_with_report(
    adapter: &dyn PlatformAdapter,
    report: &PermissionReport,
) -> Result<Value, AppError> {
    let permissions =
        permissions::execute_with_report(PermissionsArgs { request: false }, adapter, report)?;

    let store = RefStore::new().ok();
    let ref_count = store
        .as_ref()
        .and_then(|s| s.load_latest().ok())
        .map(|m| m.len());
    let snapshot_id = store.and_then(|s| s.latest_snapshot_id());

    Ok(json!({
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "permissions": permissions,
        "snapshot_id": snapshot_id,
        "ref_count": ref_count
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PermissionState;

    struct DeniedAdapter;

    impl PlatformAdapter for DeniedAdapter {
        fn permission_report(&self) -> PermissionReport {
            PermissionReport {
                accessibility: PermissionState::Denied {
                    suggestion: "should not be used".into(),
                },
                screen_recording: PermissionState::Denied {
                    suggestion: "should not be used".into(),
                },
                automation: PermissionState::Unknown,
            }
        }
    }

    #[test]
    fn status_uses_precomputed_permission_report() {
        let report = PermissionReport {
            accessibility: PermissionState::Granted,
            screen_recording: PermissionState::Granted,
            automation: PermissionState::NotRequired,
        };

        let value = execute_with_report(&DeniedAdapter, &report).unwrap();
        let permissions = value.get("permissions").unwrap();

        assert_eq!(permissions["accessibility"]["state"], "granted");
        assert_eq!(permissions["screen_recording"]["state"], "granted");
        assert_eq!(permissions["automation"]["state"], "not_required");
    }
}
