use cxx;
use futures::{channel::oneshot, executor, task::LocalSpawnExt};
use std::fmt::Debug;
use webview_official::bindings::windows::win32::{
    com, debug,
    menus_and_resources::HMENU,
    system_services::{self, HINSTANCE, LRESULT, PWSTR},
    windows_and_messaging::{
        self, HWND, LPARAM, WINDOWS_EX_STYLE, WINDOWS_STYLE, WNDCLASSW, WPARAM,
    },
};
use webview_official::bridge::{self, core};

pub fn initialize_com() {
    unsafe {
        assert!(com::CoInitialize(0 as *mut _).is_ok());
    }
}

pub fn uninitialize_com() {
    unsafe {
        com::CoUninitialize();
    }
}

pub fn run_message_loop(pool: &mut executor::LocalPool) {
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

pub struct MessageLoopCompletedContext<T>(oneshot::Sender<T>);

impl<T> MessageLoopCompletedContext<T> {
    pub fn new(tx: oneshot::Sender<T>) -> Self {
        Self(tx)
    }

    pub fn send(self, value: T) {
        let result = self.0.send(value);
        assert!(result.is_ok(), "send the value");
    }
}

pub fn create_environment_with_callback<E: Debug>(
    create: Box<
        dyn FnOnce(Box<bridge::CreateWebView2EnvironmentCompletedHandler>) -> Result<(), E>,
    >,
) -> cxx::SharedPtr<core::WebView2Environment> {
    let (tx, rx) = oneshot::channel();
    let context = Box::new(MessageLoopCompletedContext::new(tx));
    let mut pool = executor::LocalPool::new();
    let spawner = pool.spawner();
    let output = spawner
        .spawn_local_with_handle(rx)
        .expect("spawn_local_with_handle");

    let environment = {
        let handler = Box::new(bridge::CreateWebView2EnvironmentCompletedHandler::new(
            Box::new(|environment| {
                context.send(environment);
            }),
        ));
        create(handler).expect("call create");

        run_message_loop(&mut pool);

        pool.run_until(output).expect("receive the environment")
    };

    assert!(!environment.is_null());
    environment
}

pub fn create_test_environment() -> cxx::SharedPtr<core::WebView2Environment> {
    let environment = create_environment_with_callback(Box::new(|handler| {
        core::new_webview2_environment(handler)
    }));
    assert!(!environment.is_null());
    environment
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
        let mut class_name = bridge::to_utf16("TestWindow");
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

pub struct TestWindow(HWND);

impl Drop for TestWindow {
    fn drop(&mut self) {
        match self.0 {
            HWND(0) => (),
            _ => unsafe {
                println!("DestroyWindow(0x{:08X})", self.0.0);
                windows_and_messaging::DestroyWindow(self.0);
                self.0 = HWND(0);
            },
        };
    }
}

#[allow(dead_code)]
pub fn create_test_window(name: &str) -> TestWindow {
    let mut class_name = register_window_class();

    let mut window_name = bridge::to_utf16(name);
    window_name.push(0);

    unsafe {
        TestWindow(windows_and_messaging::CreateWindowExW(
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
        ))
    }
}

pub fn create_test_controller(frame: &TestWindow) -> cxx::SharedPtr<core::WebView2Controller> {
    let environment = create_test_environment();
    let (tx, rx) = oneshot::channel();
    let mut pool = executor::LocalPool::new();
    let spawner = pool.spawner();
    let context = Box::new(MessageLoopCompletedContext::new(tx));
    let output = spawner
        .spawn_local_with_handle(rx)
        .expect("spawn_local_with_handle");

    environment
        .create_webview2_controller(
            frame.0 .0,
            Box::new(bridge::CreateWebView2ControllerCompletedHandler::new(
                Box::new(|controller| {
                    context.send(controller);
                }),
            )),
        )
        .expect("call create_webview2_controller");

    run_message_loop(&mut pool);

    let controller = pool.run_until(output).expect("receive the environment");
    assert!(!controller.is_null());

    controller
}

pub fn create_test_webview(frame: &TestWindow) -> cxx::SharedPtr<core::WebView2> {
    let webview = create_test_controller(frame)
        .get_webview()
        .expect("call get_webview");
    assert!(!webview.is_null());

    webview
}

#[allow(dead_code)]
pub fn navigate_to_test_html(frame: &TestWindow) -> cxx::SharedPtr<core::WebView2> {
    let webview = create_test_webview(frame);
    let (tx, rx) = oneshot::channel();
    let mut pool = executor::LocalPool::new();
    let spawner = pool.spawner();
    let context = Box::new(MessageLoopCompletedContext::new(tx));
    let output = spawner
        .spawn_local_with_handle(rx)
        .expect("spawn_local_with_handle");

    let html_content = bridge::to_utf16(
        r#"<html>
            <head>
                <title>Sample HTML</title>
            </head>
            <body>
                <h1>Nothing to see here...</h1>
            </body>
        </html>"#,
    );
    webview
        .navigate_to_string(
            &html_content,
            Box::new(bridge::NavigationCompletedHandler::new(Box::new(
                |_webview| {
                    context.send(());
                },
            ))),
        )
        .expect("call navigate_to_string");

    run_message_loop(&mut pool);

    pool.run_until(output).expect("completed the navigation");

    webview
}

#[allow(dead_code)]
pub fn execute_test_script(webview: &core::WebView2, javascript: &str) -> String {
    let (tx, rx) = oneshot::channel();
    let mut pool = executor::LocalPool::new();
    let spawner = pool.spawner();
    let context = Box::new(MessageLoopCompletedContext::new(tx));
    let output = spawner
        .spawn_local_with_handle(rx)
        .expect("spawn_local_with_handle");

    let javacript = bridge::to_utf16(javascript);
    webview
        .execute_script(
            &javacript,
            Box::new(bridge::ExecuteScriptCompletedHandler::new(Box::new(
                |result_as_json| {
                    context.send(result_as_json);
                },
            ))),
        )
        .expect("call execute_script");

    run_message_loop(&mut pool);

    pool.run_until(output).expect("received the result")
}
