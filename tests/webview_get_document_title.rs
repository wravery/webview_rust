mod common;

use webview_official::bridge;

#[test]
fn webview_get_document_title() {
    common::initialize_com();

    {
        let frame = common::create_test_window("webview_get_document_title");
        let webview = common::navigate_to_test_html(&frame);
        let result = webview
            .get_document_title()
            .expect("call get_document_title");
        assert_eq!(r#"Sample HTML"#, bridge::from_utf16(&result));
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
