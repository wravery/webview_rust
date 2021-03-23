mod common;

use webview_official::bridge::core;

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
