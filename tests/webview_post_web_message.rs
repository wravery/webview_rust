mod common;

use webview_official::bridge;

#[test]
fn webview_post_web_message() {
    common::initialize_com();

    {
        let webview = common::navigate_to_test_html();
        common::execute_test_script(
            &webview,
            r#"window.testMessage = null;

            window.chrome.webview.addEventListener("message", function (payload) {
                window.testMessage = payload.data;
            });"#,
        );
        let message = bridge::to_utf16(r#""message received!""#);
        webview
            .post_web_message(&message)
            .expect("call post_web_message");
        let result = common::execute_test_script(&webview, r#"window.testMessage"#);
        assert_eq!(r#""message received!""#, result);
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
