use agent_desktop_core::{adapter::WindowFilter, error::AdapterError, node::WindowInfo};

pub(crate) fn list_windows_impl(filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::system::app_inventory::list_windows(filter))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = filter;
        Err(AdapterError::not_supported("list_windows"))
    }
}
