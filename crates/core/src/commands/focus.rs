use crate::{
    action::{Action, ActionRequest},
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action},
    error::AppError,
};
use serde_json::Value;

pub fn execute(args: RefArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    execute_ref_action(args, adapter, ActionRequest::headless(Action::SetFocus))
}
