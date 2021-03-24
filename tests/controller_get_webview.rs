mod common;

#[test]
fn controller_get_webview() {
    common::initialize_com();

    {
        let frame = common::create_test_window("controller_get_webview");
        let _webview = common::create_test_webview(&frame);
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
