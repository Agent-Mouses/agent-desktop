use crate::{
    action::WindowOp, adapter::PlatformAdapter, commands::helpers::resolve_window_for_app,
    error::AppError,
};
use serde_json::{Value, json};

pub struct ResizeWindowArgs {
    pub app: Option<String>,
    pub width: f64,
    pub height: f64,
}

pub fn execute(args: ResizeWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let win = resolve_window_for_app(args.app.as_deref(), adapter)?;
    adapter.window_op(
        &win,
        WindowOp::Resize {
            width: args.width,
            height: args.height,
        },
    )?;
    Ok(json!({ "resized": true, "width": args.width, "height": args.height }))
}
