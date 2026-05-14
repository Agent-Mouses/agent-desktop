use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{Value, json};

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let text = adapter.get_clipboard()?;
    Ok(json!({ "text": text }))
}
