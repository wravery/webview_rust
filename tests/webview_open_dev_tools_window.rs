mod common;

#[test]
fn webview_open_dev_tools_window() {
    common::initialize_com();

    {
        let webview = common::navigate_to_test_html();
        webview
            .open_dev_tools_window()
            .expect("call open_dev_tools_window");
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
