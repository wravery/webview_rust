mod common;

#[test]
fn webview_reload() {
    common::initialize_com();

    {
        let webview = common::navigate_to_test_html();
        webview.reload().expect("call reload");
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
