mod common;

#[test]
fn webview_open_dev_tools_window() {
    common::initialize_com();

    {
        let frame = common::create_test_window("webview_open_dev_tools_window");
        let webview = common::navigate_to_test_html(&frame);
        webview
            .open_dev_tools_window()
            .expect("call open_dev_tools_window");
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
