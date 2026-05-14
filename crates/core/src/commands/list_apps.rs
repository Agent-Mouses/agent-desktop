use crate::{adapter::PlatformAdapter, commands::search_text, error::AppError};
use serde_json::{Value, json};

pub struct ListAppsArgs {
    pub app: Option<String>,
}

pub fn execute(args: ListAppsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let mut apps = adapter.list_apps()?;
    if let Some(app) = args.app {
        let needle = search_text::normalize(&app);
        apps.retain(|candidate| search_text::contains(&candidate.name, &needle));
    }
    Ok(json!({ "apps": apps }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{adapter::PlatformAdapter, error::AdapterError, node::AppInfo};

    struct AppsAdapter;

    impl PlatformAdapter for AppsAdapter {
        fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
            Ok(vec![
                AppInfo {
                    name: "Finder".into(),
                    pid: 1,
                    bundle_id: Some("com.apple.finder".into()),
                },
                AppInfo {
                    name: "TextEdit".into(),
                    pid: 2,
                    bundle_id: Some("com.apple.TextEdit".into()),
                },
            ])
        }
    }

    #[test]
    fn app_filter_matches_by_name_case_insensitively() {
        let value = execute(
            ListAppsArgs {
                app: Some("text".into()),
            },
            &AppsAdapter,
        )
        .unwrap();

        let apps = value["apps"].as_array().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0]["name"], "TextEdit");
    }
}
