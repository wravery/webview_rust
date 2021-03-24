mod common;

#[test]
fn controller_close() {
    common::initialize_com();

    {
        let frame = common::create_test_window("controller_close");
        let controller = common::create_test_controller(&frame);
        controller.close().expect("call close");
        controller.close().expect_err("second close");
    }

    // Wait until the controller has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
