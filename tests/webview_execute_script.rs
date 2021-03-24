mod common;

#[test]
fn webview_execute_script() {
    common::initialize_com();

    {
        let frame = common::create_test_window("webview_execute_script");
        let webview = common::navigate_to_test_html(&frame);
        let javacript = r#""foo" + "bar" + "baz""#;
        let result = common::execute_test_script(&webview, &javacript);
        assert_eq!(r#""foobarbaz""#, result);
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
