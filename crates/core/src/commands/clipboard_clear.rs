use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{Value, json};

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    adapter.clear_clipboard()?;
    Ok(json!({ "cleared": true }))
}
