mod common;

use webview_official::bridge::{self, core};

#[test]
fn compare_browser_versions_less() {
    let version1 = bridge::to_utf16("89.0.774.57");
    let version2 = bridge::to_utf16("89.0.800.50");

    common::initialize_com();

    let comparison = core::compare_browser_versions(&version1, &version2)
        .expect("call new_webview2_environment");

    common::uninitialize_com();

    assert_eq!(-1, comparison);
}

#[test]
fn compare_browser_versions_equal() {
    let version1 = bridge::to_utf16("89.0.774.57");
    let version2 = bridge::to_utf16("89.0.774.57");

    common::initialize_com();

    let comparison = core::compare_browser_versions(&version1, &version2)
        .expect("call new_webview2_environment");

    common::uninitialize_com();

    assert_eq!(0, comparison);
}

#[test]
fn compare_browser_versions_greater() {
    let version1 = bridge::to_utf16("89.0.800.50");
    let version2 = bridge::to_utf16("89.0.774.57");

    common::initialize_com();

    let comparison = core::compare_browser_versions(&version1, &version2)
        .expect("call new_webview2_environment");

    common::uninitialize_com();

    assert_eq!(1, comparison);
}
