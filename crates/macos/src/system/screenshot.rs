use agent_desktop_core::{
    adapter::{ImageBuffer, ImageFormat},
    error::{AdapterError, ErrorCode},
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const SCREENCAPTURE: &str = "/usr/sbin/screencapture";
    #[cfg(not(test))]
    const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(5);
    #[cfg(test)]
    const SCREENSHOT_TIMEOUT: Duration = Duration::from_millis(20);
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn capture(window_id: Option<u32>) -> Result<ImageBuffer, AdapterError> {
        let temp = TempPng::new()?;
        let mut command = Command::new(SCREENCAPTURE);
        command.args(["-x", "-t", "png"]);

        if let Some(wid) = window_id {
            command.args(["-l", &wid.to_string()]);
        }

        command.arg(temp.path());
        let output = run_screencapture(&mut command)?;

        if !output.status.success() {
            return Err(map_screencapture_error(&output));
        }

        read_png(temp.path())
    }

    pub fn capture_app(pid: i32) -> Result<ImageBuffer, AdapterError> {
        tracing::debug!("system: screenshot app pid={pid}");
        capture(find_cg_window_id_for_pid(pid))
    }

    pub fn capture_screen(_idx: usize) -> Result<ImageBuffer, AdapterError> {
        tracing::debug!("system: screenshot screen");
        capture(None)
    }

    struct TempPng {
        dir: PathBuf,
        path: PathBuf,
    }

    impl TempPng {
        fn new() -> Result<Self, AdapterError> {
            let mut dir = std::env::temp_dir();
            dir.push(format!("agent-desktop-screenshot-{}", unique_suffix()));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .map_err(|e| AdapterError::internal(format!("create screenshot temp dir: {e}")))?;
            let path = dir.join("capture.png");
            Ok(Self { dir, path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempPng {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_dir(&self.dir);
        }
    }

    fn unique_suffix() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{}-{nanos}-{seq}", std::process::id())
    }

    fn run_screencapture(command: &mut Command) -> Result<Output, AdapterError> {
        crate::system::process::run_with_timeout(command, "screencapture", SCREENSHOT_TIMEOUT)
    }

    fn read_png(path: &Path) -> Result<ImageBuffer, AdapterError> {
        let data = std::fs::read(path)
            .map_err(|e| AdapterError::internal(format!("read screenshot: {e}")))?;
        let (width, height) = png_dimensions(&data);
        Ok(ImageBuffer {
            data,
            format: ImageFormat::Png,
            width,
            height,
        })
    }

    fn map_screencapture_error(output: &Output) -> AdapterError {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{stderr}\n{stdout}");
        let lower = combined.to_lowercase();
        if lower.contains("screen recording")
            || lower.contains("not authorized")
            || lower.contains("permission")
            || lower.contains("denied")
        {
            return AdapterError::new(ErrorCode::PermDenied, "Screen Recording permission denied")
                .with_suggestion(
                    "Open System Settings > Privacy & Security > Screen Recording and add the app that launches agent-desktop. If macOS lists the built binary separately, add that binary too.",
                )
                .with_platform_detail(combined.trim());
        }

        let detail = combined.trim();
        let detail = if detail.is_empty() {
            "screencapture produced no diagnostic output"
        } else {
            detail
        };
        AdapterError::internal("screencapture exited with error").with_platform_detail(detail)
    }

    fn png_dimensions(data: &[u8]) -> (u32, u32) {
        if data.len() < 24 {
            return (0, 0);
        }
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        (w, h)
    }

    fn find_cg_window_id_for_pid(pid: i32) -> Option<u32> {
        use core_foundation::{
            array::CFArray,
            base::{CFType, CFTypeRef, TCFType},
            dictionary::CFDictionary,
            number::CFNumber,
            string::CFString,
        };

        unsafe extern "C" {
            fn CGWindowListCopyWindowInfo(option: u32, window_id: u32) -> CFTypeRef;
        }

        let info_ref = unsafe { CGWindowListCopyWindowInfo(17, 0) };
        if info_ref.is_null() {
            return None;
        }

        let array = unsafe { CFArray::<CFType>::wrap_under_create_rule(info_ref as _) };

        let mut best_id: Option<u32> = None;
        let mut best_area: f64 = 0.0;

        for item in array.iter() {
            let dict = unsafe {
                CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                    item.as_concrete_TypeRef() as _
                )
            };

            let int_field = |key: &str| -> Option<i32> {
                let k = CFString::new(key);
                dict.find(&k).and_then(|v| {
                    let n = unsafe { CFNumber::wrap_under_get_rule(v.as_concrete_TypeRef() as _) };
                    n.to_i32()
                })
            };

            if int_field("kCGWindowOwnerPID") != Some(pid) {
                continue;
            }
            if int_field("kCGWindowLayer").unwrap_or(99) != 0 {
                continue;
            }

            let wid = match int_field("kCGWindowNumber") {
                Some(n) => n as u32,
                None => continue,
            };

            let bounds_key = CFString::new("kCGWindowBounds");
            let area = if let Some(bounds_val) = dict.find(&bounds_key) {
                let bounds_dict = unsafe {
                    CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                        bounds_val.as_concrete_TypeRef() as _,
                    )
                };
                let w = bounds_dict.find(CFString::new("Width")).and_then(|v| {
                    let n = unsafe { CFNumber::wrap_under_get_rule(v.as_concrete_TypeRef() as _) };
                    n.to_f64()
                });
                let h = bounds_dict.find(CFString::new("Height")).and_then(|v| {
                    let n = unsafe { CFNumber::wrap_under_get_rule(v.as_concrete_TypeRef() as _) };
                    n.to_f64()
                });
                w.unwrap_or(0.0) * h.unwrap_or(0.0)
            } else {
                0.0
            };

            if area > best_area {
                best_area = area;
                best_id = Some(wid);
            }
        }

        best_id
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        fn output(stderr: &str) -> Output {
            Output {
                status: ExitStatus::from_raw(1 << 8),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            }
        }

        #[test]
        fn maps_screen_recording_error_to_permission_denied() {
            let err = map_screencapture_error(&output("Screen Recording is not authorized"));

            assert_eq!(err.code, ErrorCode::PermDenied);
            assert!(err.suggestion.is_some());
        }

        #[test]
        fn maps_unknown_screencapture_error_to_internal() {
            let err = map_screencapture_error(&output("display server unavailable"));

            assert_eq!(err.code, ErrorCode::Internal);
            assert_eq!(
                err.platform_detail.as_deref(),
                Some("display server unavailable")
            );
        }

        #[test]
        fn run_screencapture_kills_timed_out_process() {
            let mut command = Command::new("/bin/sleep");
            command.arg("10");

            let err = run_screencapture(&mut command).unwrap_err();

            assert_eq!(err.code, ErrorCode::Timeout);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn capture_app(_pid: i32) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_app"))
    }

    pub fn capture_screen(_idx: usize) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_screen"))
    }
}

pub use imp::{capture_app, capture_screen};
