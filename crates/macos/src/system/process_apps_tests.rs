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
fn app_name_from_command_rejects_plugin_helpers() {
    assert_eq!(
        app_name_from_command(
            "/Applications/Foo.app/Contents/PlugIns/Foo Plugin.app/Contents/MacOS/Foo Plugin",
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

#[test]
fn app_name_from_command_rejects_app_extensions() {
    assert_eq!(
        app_name_from_command("/Applications/Foo.app/Contents/PlugIns/Share.appex/Share"),
        None
    );
}

#[test]
fn app_name_from_command_rejects_empty_app_name() {
    assert_eq!(
        app_name_from_command("/Applications/.app/Contents/MacOS/Foo"),
        None
    );
}
