mod common;

#[test]
fn webview_navigate_to_string() {
    common::initialize_com();

    let frame = common::create_test_window("webview_navigate_to_string");
    common::navigate_to_test_html(&frame);

    common::uninitialize_com();
}
