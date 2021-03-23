mod common;

#[test]
fn controller_get_webview() {
    common::initialize_com();

    {
        let _webview = common::create_test_webview();
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
