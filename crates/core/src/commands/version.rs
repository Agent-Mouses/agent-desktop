use crate::error::AppError;
use serde_json::{Value, json};

pub struct VersionArgs {
    pub json: bool,
}

pub fn execute(_args: VersionArgs) -> Result<Value, AppError> {
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "target": std::env::consts::ARCH,
        "os": std::env::consts::OS,
    }))
}
