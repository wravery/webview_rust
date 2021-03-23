mod common;

#[test]
fn webview_navigate_to_string() {
    common::initialize_com();

    common::navigate_to_test_html();

    common::uninitialize_com();
}
