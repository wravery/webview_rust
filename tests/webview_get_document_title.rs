mod common;

use webview_official::bridge;

#[test]
fn webview_get_document_title() {
    common::initialize_com();

    {
        let webview = common::navigate_to_test_html();
        let result = webview
            .get_document_title()
            .expect("call get_document_title");
        assert_eq!(r#"Sample HTML"#, bridge::from_utf16(&result));
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
