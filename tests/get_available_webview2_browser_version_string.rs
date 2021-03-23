mod common;

use webview_official::bridge::{self, core};

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
