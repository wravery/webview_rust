mod common;

use webview_official::bridge::{self, core};

#[test]
fn new_webview2_environment() {
    common::initialize_com();

    {
        let _environment = common::create_test_environment();
    }

    // Wait until the environment has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}

#[test]
fn new_webview2_environment_with_options() {
    common::initialize_com();

    {
        let _environment = common::create_environment_with_callback(Box::new(|handler| {
            let options = core::WebView2EnvironmentOptions::new("", "en-US", "", true);
            core::new_webview2_environment_with_options(&[], &[], &options, handler)
        }));
    }

    // Wait until the environment has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}

#[test]
fn get_available_webview2_browser_version_string() {
    common::initialize_com();

    let available_version = core::get_available_webview2_browser_version_string(&[])
        .expect("call new_webview2_environment");
    let available_version = bridge::from_utf16(&available_version);

    common::uninitialize_com();

    println!(
        "get_available_webview2_browser_version_string: {}",
        available_version
    );
    assert_ne!(available_version, String::new());
}

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
