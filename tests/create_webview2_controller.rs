mod common;

#[test]
fn create_webview2_controller() {
    common::initialize_com();

    {
        let frame = common::create_test_window("create_webview2_controller");
        let _controller = common::create_test_controller(&frame);
    }

    // Wait until the controller has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
