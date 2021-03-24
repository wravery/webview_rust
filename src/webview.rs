use std::sync::Arc;

use futures::{channel::oneshot, executor, task::LocalSpawnExt};

use super::bridge::{self, core};
use bindings::windows::win32::{
    com, debug,
    display_devices::RECT,
    menus_and_resources::HMENU,
    system_services::{self, HINSTANCE, LRESULT, PWSTR},
    windows_and_messaging::{
        self, HWND, LPARAM, WINDOWS_EX_STYLE, WINDOWS_STYLE, WNDCLASSW, WPARAM,
    },
};

pub struct Window(HWND);

#[repr(i32)]
#[derive(Debug)]
pub enum SizeHint {
    NONE = 0,
    MIN = 1,
    MAX = 2,
    FIXED = 3,
}

impl Default for SizeHint {
    fn default() -> Self {
        SizeHint::NONE
    }
}

struct WindowSize {
    width: i32,
    height: i32,
}

#[derive(Clone)]
pub struct Webview {
    controller: Arc<cxx::SharedPtr<core::WebView2Controller>>,
    parent: Arc<Window>,
    window: Arc<Option<Window>>,
    size: Arc<WindowSize>,
    max_size: Arc<Option<WindowSize>>,
    min_size: Arc<Option<WindowSize>>,
    url: String,
}

unsafe impl Send for Webview {}
unsafe impl Sync for Webview {}

impl Drop for Webview {
    fn drop(&mut self) {
        if Arc::strong_count(&self.controller) == 0 {
            self.controller.close();

            unsafe {
                match *self.window {
                    Some(Window(h_wnd)) => unsafe {
                        windows_and_messaging::DestroyWindow(h_wnd);
                    },
                    None => {
                        windows_and_messaging::PostQuitMessage(0);
                    }
                };
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

impl Webview {
    fn run_one(pool: &mut executor::LocalPool) {
        let mut msg = windows_and_messaging::MSG::default();
        let h_wnd = windows_and_messaging::HWND::default();

        loop {
            if pool.try_run_one() {
                break;
            }

            unsafe {
                match windows_and_messaging::GetMessageW(&mut msg, h_wnd, 0, 0).0 {
                    -1 => println!("GetMessageW failed: {}", debug::GetLastError()),
                    0 => break,
                    _ => {
                        windows_and_messaging::TranslateMessage(&msg);
                        windows_and_messaging::DispatchMessageW(&msg);
                    }
                }
            }
        }
    }

    pub fn create(debug: bool, window: Option<Window>) -> Webview {
        let h_wnd = match window {
            Some(Window(h_wnd)) => h_wnd,
            None => {
                extern "system" fn window_proc(
                    h_wnd: HWND,
                    msg: u32,
                    w_param: WPARAM,
                    l_param: LPARAM,
                ) -> LRESULT {
                    match msg {
                        windows_and_messaging::WM_SIZE => {
                            
                            LRESULT(0)
                        }
                        _ => unsafe { windows_and_messaging::DefWindowProcW(h_wnd, msg, w_param, l_param) }
                    }
                }

                let mut class_name = bridge::to_utf16("Webview");
                class_name.push(0);

                let mut window_class = WNDCLASSW::default();
                window_class.lpfn_wnd_proc = Some(window_proc);
                window_class.lpsz_class_name = PWSTR(class_name.as_mut_ptr());

                unsafe {
                    windows_and_messaging::RegisterClassW(&window_class);

                    windows_and_messaging::CreateWindowExW(
                        WINDOWS_EX_STYLE(0),
                        PWSTR(class_name.as_mut_ptr()),
                        PWSTR(class_name.as_mut_ptr()),
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
        };

        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        let environment = {
            core::new_webview2_environment(Box::new(
                bridge::CreateWebView2EnvironmentCompletedHandler::new(Box::new(|environment| {
                    context.send(environment);
                })),
            ))
            .expect("call new_webview2_environment");

            Webview::run_one(&mut pool);

            pool.run_until(output).expect("receive the environment")
        };

        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        let controller = {
            environment
                .create_webview2_controller(
                    h_wnd.0,
                    Box::new(bridge::CreateWebView2ControllerCompletedHandler::new(
                        Box::new(|controller| {
                            context.send(controller);
                        }),
                    )),
                )
                .expect("call create_webview2_controller");

            Webview::run_one(&mut pool);

            pool.run_until(output).expect("receive the controller")
        };

        let mut client_rect = RECT::default();
        unsafe { windows_and_messaging::GetClientRect(h_wnd, client_rect) };
        controller
            .bounds(core::WebView2ControllerBounds {
                left: 0,
                top: 0,
                right: client_rect.right - client_rect.left,
                bottom: client_rect.bottom - client_rect.top,
            })
            .expect("call bounds")
            .visible(true)
            .expect("call visible");

        let parent = Window(h_wnd);
        let webview = Webview {
            controller: Arc::new(controller),
            parent: Arc::new(parent),
            window: Arc::new(match window {
                Some(_) => None,
                None => Some(parent),
            }),
            size: Arc::new({
                WindowSize {
                    width: client_rect.right - client_rect.left,
                    height: client_rect.bottom - client_rect.top,
                }
            }),
            max_size: Arc::new(None),
            min_size: Arc::new(None),
            url: String::new(),
        };

        webview.init(r#""window.external={invoke:s=>window.chrome.webview.postMessage(s)}""#);

        webview
    }

    pub fn run(&self) {
        let webview = self.controller.get_webview().expect("call get_webview");
        let url = bridge::to_utf16(&self.url);

        webview.navigate(
            &url,
            Box::new(bridge::NavigationCompletedHandler::new(Box::new(
                |_webview| {},
            ))),
        );

        let mut msg = windows_and_messaging::MSG::default();
        let h_wnd = windows_and_messaging::HWND::default();

        loop {
            unsafe {
                match windows_and_messaging::GetMessageW(&mut msg, h_wnd, 0, 0).0 {
                    -1 => println!("GetMessageW failed: {}", debug::GetLastError()),
                    0 => break,
                    _ => {
                        windows_and_messaging::TranslateMessage(&msg);
                        windows_and_messaging::DispatchMessageW(&msg);
                    }
                }
            }
        }
    }

    pub fn terminate() {
        unsafe {
            windows_and_messaging::PostQuitMessage(0);
        }
    }

    // TODO Window instance
    pub fn set_title(&self, title: &str) {
        match *self.window {
            Some(Window(h_wnd)) => {
                let mut title = bridge::to_utf16(title);
                title.push(0);

                unsafe {
                    windows_and_messaging::SetWindowTextW(h_wnd, PWSTR(title.as_mut_ptr()));
                }
            }
            None => (),
        }
    }

    pub fn set_size(&mut self, width: i32, height: i32, hints: SizeHint) {
        match hints {
            MIN => {
                self.min_size = Arc::new(Some(WindowSize { width, height }));
            }
            MAX => {
                self.max_size = Arc::new(Some(WindowSize { width, height }));
            }
            _ => {
                self.size = Arc::new(WindowSize { width, height });
                self.controller.bounds(core::WebView2ControllerBounds {
                    left: 0,s
                    top: 0,
                    right: width,
                    bottom: height,
                });

                if let Some(Window(h_wnd)) = *self.window {
                    unsafe {
                        windows_and_messaging::SetWindowPos(
                            h_wnd,
                            HWND(0),
                            0,
                            0,
                            width,
                            height,
                            windows_and_messaging::SetWindowPos_uFlags::SWP_NOACTIVATE
                                | windows_and_messaging::SetWindowPos_uFlags::SWP_NOZORDER
                                | windows_and_messaging::SetWindowPos_uFlags::SWP_NOMOVE,
                        );
                    }
                }
            }
        }
    }

    pub fn get_window(&self) -> Arc<Window> {
        self.parent.clone()
    }

    pub fn navigate(&mut self, url: &str) {
        self.url = url.to_string();
    }

    pub fn init(&self, js: &str) {
        let js = bridge::to_utf16(js);
        let webview = self.controller.get_webview().expect("call get_webview");

        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        webview
            .add_script_to_execute_on_document_created(
                &js,
                Box::new(
                    bridge::AddScriptToExecuteOnDocumentCreatedCompletedHandler::new(Box::new(
                        |id| {
                            context.send(id);
                        },
                    )),
                ),
            )
            .expect("call add_script_to_execute_on_document_created");

        Webview::run_one(&mut pool);

        pool.run_until(output).expect("receive the id");
    }

    pub fn eval(&mut self, js: &str) {
        let js = bridge::to_utf16(js);
        let webview = self.controller.get_webview().expect("call get_webview");

        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        webview
            .execute_script(
                &js,
                Box::new(bridge::ExecuteScriptCompletedHandler::new(Box::new(
                    |result| {
                        context.send(result);
                    },
                ))),
            )
            .expect("call execute_script");

        Webview::run_one(&mut pool);

        pool.run_until(output).expect("receive the result");
    }

    pub fn dispatch<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Webview) + Send + 'static,
    {
        let closure = Box::into_raw(Box::new(f));
        extern "C" fn callback<F>(webview: sys::webview_t, arg: *mut c_void)
        where
            F: FnOnce(&mut Webview) + Send + 'static,
        {
            let mut webview = Webview {
                inner: Arc::new(webview),
                url: "".to_string(),
            };
            let closure: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
            (*closure)(&mut webview);
        }
        unsafe { sys::webview_dispatch(*self.inner, Some(callback::<F>), closure as *mut _) }
    }

    pub fn bind<F>(&mut self, name: &str, f: F)
    where
        F: FnMut(&str, &str),
    {
        let c_name = CString::new(name).expect("No null bytes in parameter name");
        let closure = Box::into_raw(Box::new(f));
        extern "C" fn callback<F>(seq: *const c_char, req: *const c_char, arg: *mut c_void)
        where
            F: FnMut(&str, &str),
        {
            let seq = unsafe {
                CStr::from_ptr(seq)
                    .to_str()
                    .expect("No null bytes in parameter seq")
            };
            let req = unsafe {
                CStr::from_ptr(req)
                    .to_str()
                    .expect("No null bytes in parameter req")
            };
            let mut f: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
            (*f)(seq, req);
            mem::forget(f);
        }
        unsafe {
            sys::webview_bind(
                *self.inner,
                c_name.as_ptr(),
                Some(callback::<F>),
                closure as *mut _,
            )
        }
    }

    pub fn r#return(&self, seq: &str, status: c_int, result: &str) {
        let c_seq = CString::new(seq).expect("No null bytes in parameter seq");
        let c_result = CString::new(result).expect("No null bytes in parameter result");
        unsafe { sys::webview_return(*self.inner, c_seq.as_ptr(), status, c_result.as_ptr()) }
    }
}
