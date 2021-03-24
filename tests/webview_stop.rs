mod common;

#[test]
fn webview_stop() {
    common::initialize_com();

    {
        let frame = common::create_test_window("webview_stop");
        let webview = common::navigate_to_test_html(&frame);
        webview.stop().expect("call stop");
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
