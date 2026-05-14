use agent_desktop_core::{
    PermissionReport,
    error::{AdapterError, AppError, ErrorCode},
};

use crate::cli::Commands;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermissionNeed {
    None,
    Accessibility,
    ScreenRecording,
    AccessibilityAndScreenRecording,
}

pub(crate) fn policy_for(cmd: &Commands) -> PermissionNeed {
    use PermissionNeed::{Accessibility, AccessibilityAndScreenRecording, None, ScreenRecording};
    match cmd {
        Commands::Version(_) | Commands::Skills(_) => None,
        Commands::Status | Commands::Permissions(_) => None,
        Commands::ListWindows(_) | Commands::ListApps(_) => None,
        Commands::ClipboardGet | Commands::ClipboardSet(_) | Commands::ClipboardClear => None,
        Commands::Batch(_) => None,

        Commands::Snapshot(_)
        | Commands::Find(_)
        | Commands::ListSurfaces(_)
        | Commands::Wait(_)
        | Commands::ListNotifications(_) => Accessibility,

        Commands::Screenshot(a) if a.app.is_some() || a.window_id.is_some() => {
            AccessibilityAndScreenRecording
        }
        Commands::Screenshot(_) => ScreenRecording,

        Commands::Get(_) | Commands::Is(_) => Accessibility,

        Commands::Click(_)
        | Commands::DoubleClick(_)
        | Commands::TripleClick(_)
        | Commands::RightClick(_)
        | Commands::SetValue(_)
        | Commands::Clear(_)
        | Commands::Select(_)
        | Commands::Toggle(_)
        | Commands::Check(_)
        | Commands::Uncheck(_)
        | Commands::Expand(_)
        | Commands::Collapse(_)
        | Commands::Scroll(_)
        | Commands::ScrollTo(_) => Accessibility,

        Commands::Type(_) => Accessibility,
        Commands::Focus(_) => Accessibility,
        Commands::Press(_) | Commands::KeyDown(_) | Commands::KeyUp(_) => Accessibility,
        Commands::Hover(_)
        | Commands::Drag(_)
        | Commands::MouseMove(_)
        | Commands::MouseClick(_)
        | Commands::MouseDown(_)
        | Commands::MouseUp(_) => Accessibility,

        Commands::Launch(_)
        | Commands::CloseApp(_)
        | Commands::FocusWindow(_)
        | Commands::ResizeWindow(_)
        | Commands::MoveWindow(_)
        | Commands::Minimize(_)
        | Commands::Maximize(_)
        | Commands::Restore(_)
        | Commands::DismissNotification(_)
        | Commands::DismissAllNotifications(_)
        | Commands::NotificationAction(_) => Accessibility,
    }
}

pub(crate) fn preflight(cmd: &Commands, report: &PermissionReport) -> Result<(), AppError> {
    let permission = policy_for(cmd);
    if requires_screen_recording(permission) && report.screen_recording_denied() {
        let err = AdapterError::new(
            ErrorCode::PermDenied,
            "Screen Recording permission not granted",
        )
        .with_suggestion(
            report
                .screen_recording_suggestion()
                .unwrap_or("Grant Screen Recording permission and retry"),
        );
        return Err(AppError::Adapter(err));
    }
    Ok(())
}

fn requires_screen_recording(permission: PermissionNeed) -> bool {
    matches!(
        permission,
        PermissionNeed::ScreenRecording | PermissionNeed::AccessibilityAndScreenRecording
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::cli_args::ScreenshotArgs;
    use agent_desktop_core::{PermissionReport, PermissionState};
    use clap::CommandFactory;

    #[test]
    fn every_cli_subcommand_has_policy() {
        for subcommand in Cli::command().get_subcommands() {
            let name = subcommand.get_name();
            assert!(
                command_name_is_covered(name),
                "missing permission policy coverage for {name}"
            );
        }
    }

    fn command_name_is_covered(name: &str) -> bool {
        matches!(
            name,
            "snapshot"
                | "find"
                | "screenshot"
                | "get"
                | "is"
                | "click"
                | "double-click"
                | "triple-click"
                | "right-click"
                | "type"
                | "set-value"
                | "clear"
                | "focus"
                | "select"
                | "toggle"
                | "check"
                | "uncheck"
                | "expand"
                | "collapse"
                | "scroll"
                | "scroll-to"
                | "press"
                | "key-down"
                | "key-up"
                | "hover"
                | "drag"
                | "mouse-move"
                | "mouse-click"
                | "mouse-down"
                | "mouse-up"
                | "launch"
                | "close-app"
                | "list-windows"
                | "list-apps"
                | "focus-window"
                | "resize-window"
                | "move-window"
                | "minimize"
                | "maximize"
                | "restore"
                | "list-surfaces"
                | "list-notifications"
                | "dismiss-notification"
                | "dismiss-all-notifications"
                | "notification-action"
                | "clipboard-get"
                | "clipboard-set"
                | "clipboard-clear"
                | "wait"
                | "status"
                | "permissions"
                | "version"
                | "batch"
                | "skills"
        )
    }

    #[test]
    fn unknown_permission_does_not_mask_platform_errors() {
        let report = PermissionReport::default();
        let command = Commands::Screenshot(ScreenshotArgs {
            app: None,
            window_id: None,
            output_path: None,
        });

        assert!(preflight(&command, &report).is_ok());
    }

    #[test]
    fn screen_recording_denial_is_preflighted() {
        let report = PermissionReport {
            accessibility: PermissionState::Granted,
            screen_recording: PermissionState::Denied {
                suggestion: "grant screen recording".into(),
            },
            automation: PermissionState::NotRequired,
        };
        let command = Commands::Screenshot(ScreenshotArgs {
            app: None,
            window_id: None,
            output_path: None,
        });

        let err = preflight(&command, &report).expect_err("denied screen capture fails");

        assert_eq!(err.code(), "PERM_DENIED");
    }

    #[test]
    fn accessibility_denial_does_not_preflight_ax_commands() {
        let report = PermissionReport {
            accessibility: PermissionState::Denied {
                suggestion: "grant accessibility".into(),
            },
            screen_recording: PermissionState::Granted,
            automation: PermissionState::NotRequired,
        };
        let command = Commands::Click(crate::cli_args::RefArgs {
            ref_id: "@e1".into(),
            snapshot_id: None,
        });

        preflight(&command, &report).expect("AX command should execute and report adapter result");
    }
}
