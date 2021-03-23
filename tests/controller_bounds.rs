mod common;

use webview_official::bridge::core;

#[test]
fn controller_bounds() {
    common::initialize_com();

    {
        let controller = common::create_test_controller();
        let bounds = controller.get_bounds().expect("call get_bounds");
        // The bounds default to the null rect.
        assert_eq!(bounds.left, 0);
        assert_eq!(bounds.top, 0);
        assert_eq!(bounds.right, 0);
        assert_eq!(bounds.bottom, 0);
        // Set distinct values for each value in the bounds rectangle and make sure it sticks.
        let bounds = controller
            .bounds(core::BoundsRectangle {
                left: 10,
                top: 20,
                right: 30,
                bottom: 40,
            })
            .expect("call visible")
            .get_bounds()
            .expect("call get_bounds");
        assert_eq!(bounds.left, 10);
        assert_eq!(bounds.top, 20);
        assert_eq!(bounds.right, 30);
        assert_eq!(bounds.bottom, 40);
    }

    // Wait until the controller has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
