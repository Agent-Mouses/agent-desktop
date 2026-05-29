use super::*;

#[test]
fn app_name_from_command_extracts_app_bundle_name() {
    assert_eq!(
        app_name_from_command("/Applications/Finder.app/Contents/MacOS/Finder").as_deref(),
        Some("Finder")
    );
}

#[test]
fn app_name_from_command_rejects_framework_helpers() {
    assert_eq!(
        app_name_from_command(
            "/Applications/Foo.app/Contents/Frameworks/Foo Helper.app/Contents/MacOS/Foo Helper",
        ),
        None
    );
}

#[test]
fn app_name_from_command_rejects_xpc_services() {
    assert_eq!(
        app_name_from_command("/Applications/Foo.app/Contents/XPCServices/Worker.xpc/Worker"),
        None
    );
}
