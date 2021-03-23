mod common;

#[test]
fn controller_close() {
    common::initialize_com();

    {
        let controller = common::create_test_controller();
        controller.close().expect("call close");
        controller.close().expect_err("second close");
    }

    // Wait until the controller has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
