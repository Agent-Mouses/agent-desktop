use crate::{
    action::WindowOp,
    adapter::PlatformAdapter,
    commands::helpers::{AppArgs, window_op_command},
    error::AppError,
};
use serde_json::Value;

pub fn execute(args: AppArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    window_op_command(args, adapter, WindowOp::Minimize, "minimized")
}
