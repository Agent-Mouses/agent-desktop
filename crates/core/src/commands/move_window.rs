use crate::{
    action::WindowOp, adapter::PlatformAdapter, commands::helpers::resolve_window_for_app,
    error::AppError,
};
use serde_json::{Value, json};

pub struct MoveWindowArgs {
    pub app: Option<String>,
    pub x: f64,
    pub y: f64,
}

pub fn execute(args: MoveWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let win = resolve_window_for_app(args.app.as_deref(), adapter)?;
    adapter.window_op(
        &win,
        WindowOp::Move {
            x: args.x,
            y: args.y,
        },
    )?;
    Ok(json!({ "moved": true, "x": args.x, "y": args.y }))
}
