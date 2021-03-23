mod common;

#[test]
fn controller_visible() {
    common::initialize_com();

    {
        let controller = common::create_test_controller();
        assert!(!controller.get_visible().expect("call get_visible"));
        assert!(controller
            .visible(true)
            .expect("call visible")
            .get_visible()
            .expect("call get_visible"));
    }

    // Wait until the controller has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
