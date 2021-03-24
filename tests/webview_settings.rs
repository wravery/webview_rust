mod common;

use webview_official::bridge::core;

#[test]
fn webview_settings() {
    common::initialize_com();

    {
        let frame = common::create_test_window("webview_settings");
        let webview = common::create_test_webview(&frame);
        let settings = webview.get_settings().expect("call get_settings");
        // All of the settings start out enabled by default.
        assert!(settings.is_script_enabled);
        assert!(settings.is_web_message_enabled);
        assert!(settings.are_default_script_dialogs_enabled);
        assert!(settings.is_status_bar_enabled);
        assert!(settings.are_dev_tools_enabled);
        assert!(settings.are_default_context_menus_enabled);
        assert!(settings.is_zoom_control_enabled);
        assert!(settings.is_built_in_error_page_enabled);
        // Disable all of the settings and make sure it sticks.
        let settings = webview
            .settings(core::WebView2Settings {
                is_script_enabled: false,
                is_web_message_enabled: false,
                are_default_script_dialogs_enabled: false,
                is_status_bar_enabled: false,
                are_dev_tools_enabled: false,
                are_default_context_menus_enabled: false,
                is_zoom_control_enabled: false,
                is_built_in_error_page_enabled: false,
            })
            .expect("call settings")
            .get_settings()
            .expect("call get_settings");
        assert!(!settings.is_script_enabled);
        assert!(!settings.is_web_message_enabled);
        assert!(!settings.are_default_script_dialogs_enabled);
        assert!(!settings.is_status_bar_enabled);
        assert!(!settings.are_dev_tools_enabled);
        assert!(!settings.are_default_context_menus_enabled);
        assert!(!settings.is_zoom_control_enabled);
        assert!(!settings.is_built_in_error_page_enabled);
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
