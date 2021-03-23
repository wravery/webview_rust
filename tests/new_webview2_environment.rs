mod common;

#[test]
fn new_webview2_environment() {
    common::initialize_com();

    {
        let _environment = common::create_test_environment();
    }

    // Wait until the environment has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
