mod common;

#[test]
fn webview_stop() {
    common::initialize_com();

    {
        let webview = common::navigate_to_test_html();
        webview.stop().expect("call stop");
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
