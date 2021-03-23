// use crate::builder::WebviewBuilder;
// use crate::webview::{SizeHint, Webview, Window};

#[cxx::bridge]
pub mod core {
    #[derive(Debug)]
    struct WebView2EnvironmentOptions {
        aditional_browser_arguments: Vec<u16>,
        language: Vec<u16>,
        target_compatible_browser_version: Vec<u16>,
        allow_single_sign_on_using_os_primary_account: bool,
    }

    #[derive(Debug)]
    struct BoundsRectangle {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[derive(Debug)]
    struct WebView2Settings {
        is_script_enabled: bool,
        is_web_message_enabled: bool,
        are_default_script_dialogs_enabled: bool,
        is_status_bar_enabled: bool,
        are_dev_tools_enabled: bool,
        are_default_context_menus_enabled: bool,
        is_zoom_control_enabled: bool,
        is_built_in_error_page_enabled: bool,
    }

    extern "Rust" {
        fn to_utf16(value: &str) -> Vec<u16>;
        fn from_utf16(value: &[u16]) -> String;

        type CreateWebView2EnvironmentCompletedHandler;

        fn invoke_environment_complete(
            handler: Box<CreateWebView2EnvironmentCompletedHandler>,
            environment: UniquePtr<WebView2Environment>,
        );

        type CreateWebView2ControllerCompletedHandler;

        fn invoke_controller_complete(
            handler: Box<CreateWebView2ControllerCompletedHandler>,
            controller: UniquePtr<WebView2Controller>,
        );

        type ExecuteScriptCompletedHandler;

        fn invoke_script_complete(handler: Box<ExecuteScriptCompletedHandler>, result: Vec<u16>);
    }

    unsafe extern "C++" {
        include!("webview_official/src/bridge.h");

        fn new_webview2_environment(
            handler: Box<CreateWebView2EnvironmentCompletedHandler>,
        ) -> Result<()>;

        fn new_webview2_environment_with_options(
            browser_executable_folder: &[u16],
            user_data_folder: &[u16],
            options: &WebView2EnvironmentOptions,
            handler: Box<CreateWebView2EnvironmentCompletedHandler>,
        ) -> Result<()>;

        fn get_available_webview2_browser_version_string(
            browser_executable_folder: &[u16],
        ) -> Result<Vec<u16>>;

        fn compare_browser_versions(version1: &[u16], version2: &[u16]) -> Result<i8>;

        type WebView2Environment;

        fn create_webview2_controller(
            self: &WebView2Environment,
            parent_window: isize,
            handler: Box<CreateWebView2ControllerCompletedHandler>,
        ) -> Result<&WebView2Environment>;

        type WebView2Controller;

        fn visible(self: &WebView2Controller, value: bool) -> Result<&WebView2Controller>;
        fn get_visible(self: &WebView2Controller) -> Result<bool>;
        fn bounds(self: &WebView2Controller, value: BoundsRectangle)
            -> Result<&WebView2Controller>;
        fn get_bounds(self: &WebView2Controller) -> Result<BoundsRectangle>;
        fn close(self: &WebView2Controller);
        fn get_webview(self: &WebView2Controller) -> Result<SharedPtr<WebView2>>;

        type WebView2;

        fn settings(self: &WebView2, value: WebView2Settings) -> Result<&WebView2>;
        fn get_settings(self: &WebView2) -> Result<WebView2Settings>;
        fn navigate(self: &WebView2, url: &[u16]) -> Result<&WebView2>;
        fn navigate_to_string(self: &WebView2, html_content: &[u16]) -> Result<&WebView2>;
        fn execute_script(
            self: &WebView2,
            javascript: &[u16],
            handler: Box<ExecuteScriptCompletedHandler>,
        ) -> Result<&WebView2>;
        fn reload(self: &WebView2) -> Result<&WebView2>;
        fn post_web_message(self: &WebView2, json_message: &[u16]) -> Result<&WebView2>;
        fn stop(self: &WebView2) -> Result<&WebView2>;
        fn get_document_title(self: &WebView2) -> Result<Vec<u16>>;
        fn open_dev_tools_window(self: &WebView2) -> Result<&WebView2>;
    }
}

pub fn to_utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

pub fn from_utf16(value: &[u16]) -> String {
    match String::from_utf16(value) {
        Ok(result) => result,
        Err(_) => String::new(),
    }
}

impl core::WebView2EnvironmentOptions {
    pub fn new(
        aditional_browser_arguments: &str,
        language: &str,
        target_compatible_browser_version: &str,
        allow_single_sign_on_using_os_primary_account: bool,
    ) -> core::WebView2EnvironmentOptions {
        core::WebView2EnvironmentOptions {
            aditional_browser_arguments: to_utf16(aditional_browser_arguments),
            language: to_utf16(language),
            target_compatible_browser_version: to_utf16(target_compatible_browser_version),
            allow_single_sign_on_using_os_primary_account,
        }
    }
}

type EnvironmentCompletedCallback = Box<dyn FnOnce(cxx::UniquePtr<core::WebView2Environment>)>;

pub struct CreateWebView2EnvironmentCompletedHandler {
    pub callback: EnvironmentCompletedCallback,
}

pub fn invoke_environment_complete(
    handler: Box<CreateWebView2EnvironmentCompletedHandler>,
    environment: cxx::UniquePtr<core::WebView2Environment>,
) {
    (handler.callback)(environment);
}

type ControllerCompletedCallback = Box<dyn FnOnce(cxx::UniquePtr<core::WebView2Controller>)>;

pub struct CreateWebView2ControllerCompletedHandler {
    pub callback: ControllerCompletedCallback,
}

pub fn invoke_controller_complete(
    handler: Box<CreateWebView2ControllerCompletedHandler>,
    controller: cxx::UniquePtr<core::WebView2Controller>,
) {
    (handler.callback)(controller);
}

type ExecuteScriptCompletedCallback = Box<dyn FnOnce(String)>;

pub struct ExecuteScriptCompletedHandler {
    pub callback: ExecuteScriptCompletedCallback,
}

pub fn invoke_script_complete(handler: Box<ExecuteScriptCompletedHandler>, result: Vec<u16>) {
    (handler.callback)(from_utf16(&result));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bindings::windows::win32::{
        com, debug,
        menus_and_resources::HMENU,
        system_services::{self, HINSTANCE, LRESULT, PWSTR},
        windows_and_messaging::{
            self, HWND, LPARAM, WINDOWS_EX_STYLE, WINDOWS_STYLE, WNDCLASSW, WPARAM,
        },
    };
    use futures::{channel::oneshot, executor, task::LocalSpawnExt};

    fn run_message_loop(pool: &mut executor::LocalPool) {
        let mut msg = windows_and_messaging::MSG::default();
        let h_wnd = windows_and_messaging::HWND::default();

        loop {
            if pool.try_run_one() {
                println!("pool.try_run_one() returned true");
                break;
            }

            unsafe {
                match windows_and_messaging::GetMessageW(&mut msg, h_wnd, 0, 0).0 {
                    -1 => panic!("GetMessageW failed: {}", debug::GetLastError()),
                    0 => println!("GetMessageW returned 0"),
                    _ => {
                        windows_and_messaging::TranslateMessage(&msg);
                        windows_and_messaging::DispatchMessageW(&msg);
                    }
                }
            }
        }
    }

    struct MessageLoopCompletedContext<T>(oneshot::Sender<T>);

    #[test]
    fn new_webview2_environment() {
        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        {
            let environment = unsafe {
                assert!(com::CoInitialize(0 as *mut _).is_ok());

                core::new_webview2_environment(Box::new(
                    CreateWebView2EnvironmentCompletedHandler {
                        callback: Box::new(|environment| {
                            let result = context.0.send(environment);
                            assert!(matches!(result, Ok(())), "send the environment");
                        }),
                    },
                ))
                .expect("call new_webview2_environment");

                run_message_loop(&mut pool);

                pool.run_until(output).expect("receive the environment")
            };

            assert!(!environment.is_null());
        }

        unsafe {
            // Wait until the environment has gone out of scope before calling CoUninitialize.
            com::CoUninitialize();
        }
    }

    #[test]
    fn new_webview2_environment_with_options() {
        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");
        let options = core::WebView2EnvironmentOptions::new("", "en-US", "", true);

        {
            let environment = unsafe {
                assert!(com::CoInitialize(0 as *mut _).is_ok());

                core::new_webview2_environment_with_options(
                    &[],
                    &[],
                    &options,
                    Box::new(CreateWebView2EnvironmentCompletedHandler {
                        callback: Box::new(|environment| {
                            let result = context.0.send(environment);
                            assert!(matches!(result, Ok(())), "send the environment");
                        }),
                    }),
                )
                .expect("call new_webview2_environment");

                run_message_loop(&mut pool);

                pool.run_until(output).expect("receive the environment")
            };

            assert!(!environment.is_null());
        }

        unsafe {
            // Wait until the environment has gone out of scope before calling CoUninitialize.
            com::CoUninitialize();
        }
    }

    #[test]
    fn get_available_webview2_browser_version_string() {
        let available_version = unsafe {
            assert!(com::CoInitialize(0 as *mut _).is_ok());
            let available_version = core::get_available_webview2_browser_version_string(&[])
                .expect("call new_webview2_environment");
            com::CoUninitialize();

            from_utf16(&available_version)
        };

        println!(
            "get_available_webview2_browser_version_string: {}",
            available_version
        );
        assert_ne!(available_version, String::new());
    }

    #[test]
    fn compare_browser_versions_less() {
        let version1 = to_utf16("89.0.774.57");
        let version2 = to_utf16("89.0.800.50");
        let comparison = unsafe {
            assert!(com::CoInitialize(0 as *mut _).is_ok());
            let comparison = core::compare_browser_versions(&version1, &version2)
                .expect("call new_webview2_environment");
            com::CoUninitialize();

            comparison
        };

        assert_eq!(-1, comparison);
    }

    #[test]
    fn compare_browser_versions_equal() {
        let version1 = to_utf16("89.0.774.57");
        let version2 = to_utf16("89.0.774.57");
        let comparison = unsafe {
            assert!(com::CoInitialize(0 as *mut _).is_ok());
            let comparison = core::compare_browser_versions(&version1, &version2)
                .expect("call new_webview2_environment");
            com::CoUninitialize();

            comparison
        };

        assert_eq!(0, comparison);
    }

    #[test]
    fn compare_browser_versions_greater() {
        let version1 = to_utf16("89.0.800.50");
        let version2 = to_utf16("89.0.774.57");
        let comparison = unsafe {
            assert!(com::CoInitialize(0 as *mut _).is_ok());
            let comparison = core::compare_browser_versions(&version1, &version2)
                .expect("call new_webview2_environment");
            com::CoUninitialize();

            comparison
        };

        assert_eq!(1, comparison);
    }

    extern "system" fn test_window_proc(
        h_wnd: HWND,
        msg: u32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        unsafe { windows_and_messaging::DefWindowProcW(h_wnd, msg, w_param, l_param) }
    }

    fn register_window_class() -> Vec<u16> {
        static mut IS_REGISTERED: bool = false;

        unsafe {
            let mut class_name = to_utf16("TestWindow");
            class_name.push(0);

            if !IS_REGISTERED {
                let mut window_class = WNDCLASSW::default();
                window_class.lpfn_wnd_proc = Some(test_window_proc);
                window_class.lpsz_class_name = PWSTR(class_name.as_mut_ptr());
                windows_and_messaging::RegisterClassW(&window_class);

                IS_REGISTERED = true;
            }

            class_name
        }
    }

    fn create_test_window(name: &str) -> HWND {
        let mut class_name = register_window_class();

        let mut window_name = to_utf16(name);
        window_name.push(0);

        unsafe {
            windows_and_messaging::CreateWindowExW(
                WINDOWS_EX_STYLE(0),
                PWSTR(class_name.as_mut_ptr()),
                PWSTR(window_name.as_mut_ptr()),
                WINDOWS_STYLE::WS_OVERLAPPED,
                windows_and_messaging::CW_USEDEFAULT,
                windows_and_messaging::CW_USEDEFAULT,
                windows_and_messaging::CW_USEDEFAULT,
                windows_and_messaging::CW_USEDEFAULT,
                HWND(0),
                HMENU(0),
                HINSTANCE(system_services::GetModuleHandleW(PWSTR(0 as *mut _))),
                0 as *mut _,
            )
        }
    }

    #[test]
    fn create_webview2_controller() {
        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        {
            let environment = unsafe {
                assert!(com::CoInitialize(0 as *mut _).is_ok());

                core::new_webview2_environment(Box::new(
                    CreateWebView2EnvironmentCompletedHandler {
                        callback: Box::new(|environment| {
                            let result = context.0.send(environment);
                            assert!(matches!(result, Ok(())), "send the environment");
                        }),
                    },
                ))
                .expect("call new_webview2_environment");

                run_message_loop(&mut pool);

                pool.run_until(output).expect("receive the environment")
            };
            assert!(!environment.is_null());

            let frame = create_test_window("create_webview2_controller");
            let (tx, rx) = oneshot::channel();
            let context = Box::new(MessageLoopCompletedContext(tx));
            let output = spawner
                .spawn_local_with_handle(rx)
                .expect("spawn_local_with_handle");

            environment
                .create_webview2_controller(
                    frame.0,
                    Box::new(CreateWebView2ControllerCompletedHandler {
                        callback: Box::new(|controller| {
                            let result = context.0.send(controller);
                            assert!(matches!(result, Ok(())), "send the controller");
                        }),
                    }),
                )
                .expect("call create_webview2_controller");

            run_message_loop(&mut pool);

            let controller = pool.run_until(output).expect("receive the environment");
            assert!(!controller.is_null());
        }

        unsafe {
            // Wait until the environment has gone out of scope before calling CoUninitialize.
            com::CoUninitialize();
        }
    }
}
