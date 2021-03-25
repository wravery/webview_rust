use std::{
    collections::HashMap,
    mem,
    sync::{mpsc, Arc, Mutex},
};

use futures::{channel::oneshot, executor, task::LocalSpawnExt};

use super::bridge::{self, core};
use bindings::windows::win32::{
    debug,
    display_devices::{POINT, RECT},
    gdi, hi_dpi, keyboard_and_mouse_input,
    menus_and_resources::HMENU,
    system_services::{self, DPI_AWARENESS_CONTEXT, HINSTANCE, LRESULT, PWSTR},
    windows_and_messaging::{
        self, SetWindowLong_nIndex, HWND, LPARAM, MINMAXINFO, SHOW_WINDOW_CMD, WINDOWS_EX_STYLE,
        WINDOWS_STYLE, WNDCLASSW, WPARAM,
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
pub struct FrameWindow {
    window: Arc<Window>,
    size: Arc<WindowSize>,
    max_size: Arc<Option<WindowSize>>,
    min_size: Arc<Option<WindowSize>>,
}

impl Drop for FrameWindow {
    fn drop(&mut self) {
        if Arc::strong_count(&self.window) == 0 {
            unsafe {
                windows_and_messaging::DestroyWindow(self.window.0);
                windows_and_messaging::PostQuitMessage(0);
            }
        }
    }
}
#[derive(Clone)]
pub struct Webview {
    controller: Arc<cxx::SharedPtr<core::WebView2Controller>>,
    tx: mpsc::Sender<Box<dyn FnOnce(Webview) + Send>>,
    rx: Arc<mpsc::Receiver<Box<dyn FnOnce(Webview) + Send>>>,
    thread_id: u32,
    bindings: Arc<Mutex<HashMap<String, Box<dyn FnMut(&str, &str)>>>>,
    frame: Option<FrameWindow>,
    parent: Arc<Window>,
    url: String,
}

unsafe impl Send for Webview {}
unsafe impl Sync for Webview {}

impl Drop for Webview {
    fn drop(&mut self) {
        match Arc::strong_count(&self.controller) {
            0 => {
                self.controller.close().expect("call close");

                if self.frame.is_none() {
                    unsafe {
                        windows_and_messaging::PostQuitMessage(0);
                    }
                }
            }
            _ => (),
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

    #[cfg(target_pointer_width = "32")]
    fn get_window_extra_size() -> i32 {
        // It'll fit in a single entry for GWL_USERDATA
        0
    }

    #[cfg(target_pointer_width = "64")]
    fn get_window_extra_size() -> i32 {
        // We need an extra 4 bytes since the win32 bindings don't expose Get/SetWindowLongPtr
        4
    }

    #[cfg(target_pointer_width = "32")]
    fn set_window_webview(h_wnd: HWND, webview: Box<Webview>) {
        unsafe {
            windows_and_messaging::SetWindowLongW(
                h_wnd,
                SetWindowLong_nIndex::GWL_USERDATA,
                Box::into_raw(webview) as _,
            );
        }
    }

    #[cfg(target_pointer_width = "64")]
    fn set_window_webview(h_wnd: HWND, webview: Box<Webview>) {
        let address = Box::into_raw(webview) as usize;
        let low = address as u32;
        let high = (address >> 32) as u32;

        unsafe {
            windows_and_messaging::SetWindowLongW(
                h_wnd,
                SetWindowLong_nIndex::GWL_USERDATA,
                low as _,
            );
            windows_and_messaging::SetWindowLongW(h_wnd, SetWindowLong_nIndex(0), high as _);
        }
    }

    #[cfg(target_pointer_width = "32")]
    fn get_window_webview(h_wnd: HWND) -> Option<Box<Webview>> {
        unsafe {
            let data =
                windows_and_messaging::GetWindowLongW(h_wnd, SetWindowLong_nIndex::GWL_USERDATA);

            match data {
                0 => None,
                _ => {
                    let webview_ptr = data as *mut Webview;
                    let raw = Box::from_raw(webview_ptr);
                    let webview = Box::new(raw.clone());
                    mem::forget(raw);

                    Some(webview)
                }
            }
        }
    }

    #[cfg(target_pointer_width = "64")]
    fn get_window_webview(h_wnd: HWND) -> Option<Box<Webview>> {
        unsafe {
            let low =
                windows_and_messaging::GetWindowLongW(h_wnd, SetWindowLong_nIndex::GWL_USERDATA)
                    as u32;
            let high = windows_and_messaging::GetWindowLongW(h_wnd, SetWindowLong_nIndex(0)) as u32;

            match (low, high) {
                (0, 0) => None,
                _ => {
                    let address = (low as u64) | ((high as u64) << 32);
                    let webview_ptr = address as *mut Webview;
                    let raw = Box::from_raw(webview_ptr);
                    let webview = raw.clone();
                    mem::forget(raw);

                    Some(webview)
                }
            }
        }
    }

    fn get_window_size(h_wnd: HWND) -> WindowSize {
        let mut client_rect = RECT::default();
        unsafe { windows_and_messaging::GetClientRect(h_wnd, &mut client_rect) };
        WindowSize {
            width: client_rect.right - client_rect.left,
            height: client_rect.bottom - client_rect.top,
        }
    }

    fn create_frame() -> FrameWindow {
        extern "system" fn window_proc(
            h_wnd: HWND,
            msg: u32,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> LRESULT {
            let webview = match Webview::get_window_webview(h_wnd) {
                Some(webview) => webview,
                None => {
                    return unsafe {
                        windows_and_messaging::DefWindowProcW(h_wnd, msg, w_param, l_param)
                    }
                }
            };

            let mut frame = webview
                .frame
                .clone()
                .expect("should only be called for owned windows");

            match msg {
                windows_and_messaging::WM_SIZE => {
                    let size = Webview::get_window_size(h_wnd);
                    webview
                        .controller
                        .bounds(core::WebView2ControllerBounds {
                            left: 0,
                            top: 0,
                            right: size.width,
                            bottom: size.height,
                        })
                        .expect("call bounds");
                    frame.size = Arc::new(size);
                    LRESULT(0)
                }

                windows_and_messaging::WM_CLOSE => {
                    unsafe {
                        windows_and_messaging::DestroyWindow(h_wnd);
                    }
                    LRESULT(0)
                }

                windows_and_messaging::WM_DESTROY => {
                    webview.terminate();
                    LRESULT(0)
                }

                windows_and_messaging::WM_GETMINMAXINFO => {
                    let mut min_max_info = MINMAXINFO::default();

                    if let Some(max) = frame.max_size.as_ref() {
                        let max_size = POINT {
                            x: max.width,
                            y: max.height,
                        };

                        min_max_info.pt_max_track_size = max_size;
                        min_max_info.pt_max_size = max_size;
                    }

                    if let Some(min) = frame.min_size.as_ref() {
                        min_max_info.pt_min_track_size = POINT {
                            x: min.width,
                            y: min.height,
                        };
                    }

                    LRESULT(0)
                }

                _ => unsafe { windows_and_messaging::DefWindowProcW(h_wnd, msg, w_param, l_param) },
            }
        }

        let h_wnd = {
            let mut class_name = bridge::to_utf16("Webview");
            class_name.push(0);

            let mut window_class = WNDCLASSW::default();
            window_class.lpfn_wnd_proc = Some(window_proc);
            window_class.lpsz_class_name = PWSTR(class_name.as_mut_ptr());
            window_class.cb_wnd_extra = Webview::get_window_extra_size();

            unsafe {
                windows_and_messaging::RegisterClassW(&window_class);

                windows_and_messaging::CreateWindowExW(
                    WINDOWS_EX_STYLE(0),
                    PWSTR(class_name.as_mut_ptr()),
                    PWSTR(class_name.as_mut_ptr()),
                    WINDOWS_STYLE::WS_OVERLAPPED | WINDOWS_STYLE::WS_VISIBLE,
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
        };

        FrameWindow {
            window: Arc::new(Window(h_wnd)),
            size: Arc::new(WindowSize {
                width: 0,
                height: 0,
            }),
            min_size: Arc::new(None),
            max_size: Arc::new(None),
        }
    }

    pub fn create(debug: bool, window: Option<Window>) -> Webview {
        let mut frame = match window {
            Some(Window(_)) => None,
            None => Some(Webview::create_frame()),
        };

        let h_wnd = match window {
            Some(Window(h_wnd)) => h_wnd,
            None => frame.as_ref().unwrap().window.0,
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

        let size = Webview::get_window_size(h_wnd);
        let mut client_rect = RECT::default();
        unsafe { windows_and_messaging::GetClientRect(h_wnd, &mut client_rect) };
        controller
            .bounds(core::WebView2ControllerBounds {
                left: 0,
                top: 0,
                right: size.width,
                bottom: size.height,
            })
            .expect("call bounds")
            .visible(true)
            .expect("call visible");

        if debug {
            let webview = controller.get_webview().expect("call get_webview");
            let settings = core::WebView2Settings {
                are_dev_tools_enabled: true,
                are_default_context_menus_enabled: true,
                ..core::WebView2Settings::default()
            };
            webview.settings(settings).expect("call settings");
        }

        if let Some(frame) = frame.as_mut() {
            frame.size = Arc::new(size);
        }

        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(rx);
        let thread_id = unsafe { system_services::GetCurrentThreadId() };
        let parent = Window(h_wnd);

        let webview = Webview {
            controller: Arc::new(controller),
            tx,
            rx,
            thread_id,
            bindings: Arc::new(Mutex::new(HashMap::new())),
            frame,
            parent: Arc::new(parent),
            url: String::new(),
        };

        // Inject the invoke handler.
        webview.init(r#"window.external={invoke:s=>window.chrome.webview.postMessage(s)}"#);

        if webview.frame.is_some() {
            let webview = Box::new(webview.clone());

            unsafe {
                Webview::set_window_webview(h_wnd, webview);

                hi_dpi::SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT(-3));
                windows_and_messaging::ShowWindow(h_wnd, SHOW_WINDOW_CMD::SW_SHOW);
                gdi::UpdateWindow(h_wnd);
                keyboard_and_mouse_input::SetFocus(h_wnd);
            }
        }

        webview
    }

    pub fn run(&self) {
        let webview = self.controller.get_webview().expect("call get_webview");
        let url = bridge::to_utf16(&self.url);
        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        webview
            .navigate(
                &url,
                Box::new(bridge::NavigationCompletedHandler::new(Box::new(
                    |_webview| {
                        context.send(());
                    },
                ))),
            )
            .expect("call navigate");

        Webview::run_one(&mut pool);

        pool.run_until(output).expect("completed the navigation");
        println!("Navigation complete");

        let mut msg = windows_and_messaging::MSG::default();
        let h_wnd = windows_and_messaging::HWND::default();

        loop {
            while let Ok(f) = self.rx.try_recv() {
                (f)(self.clone());
            }

            unsafe {
                let result = windows_and_messaging::GetMessageW(&mut msg, h_wnd, 0, 0).0;

                match result {
                    -1 => println!("GetMessageW failed: {}", debug::GetLastError()),
                    0 => break,
                    _ => match msg.message {
                        windows_and_messaging::WM_APP => (),
                        _ => {
                            windows_and_messaging::TranslateMessage(&msg);
                            windows_and_messaging::DispatchMessageW(&msg);
                        }
                    },
                }
            }
        }
    }

    pub fn terminate(&self) {
        self.dispatch(|_webview| unsafe {
            windows_and_messaging::PostQuitMessage(0);
        });
    }

    // TODO Window instance
    pub fn set_title(&self, title: &str) {
        match self.frame.as_ref() {
            Some(frame) => {
                let mut title = bridge::to_utf16(title);
                title.push(0);

                unsafe {
                    windows_and_messaging::SetWindowTextW(
                        frame.window.0,
                        PWSTR(title.as_mut_ptr()),
                    );
                }
            }
            None => (),
        }
    }

    pub fn set_size(&mut self, width: i32, height: i32, hints: SizeHint) {
        match self.frame.as_mut() {
            Some(frame) => match hints {
                SizeHint::MIN => {
                    frame.min_size = Arc::new(Some(WindowSize { width, height }));
                }
                SizeHint::MAX => {
                    frame.max_size = Arc::new(Some(WindowSize { width, height }));
                }
                _ => {
                    frame.size = Arc::new(WindowSize { width, height });
                    self.controller
                        .bounds(core::WebView2ControllerBounds {
                            left: 0,
                            top: 0,
                            right: width,
                            bottom: height,
                        })
                        .expect("call bounds");

                    unsafe {
                        windows_and_messaging::SetWindowPos(
                            frame.window.0,
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
            },
            None => (),
        }
    }

    pub fn get_window(&self) -> Arc<Window> {
        self.parent.clone()
    }

    pub fn navigate(&mut self, url: &str) {
        self.url = url.to_string();
    }

    pub fn init(&self, js: &str) {
        println!("Installing init script: {}", js);

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

    pub fn eval(&self, js: &str) {
        println!("Evaluating script: {}", js);

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

    pub fn dispatch<F>(&self, f: F)
    where
        F: FnOnce(Webview) + Send + 'static,
    {
        self.tx.send(Box::new(f)).expect("send the fn");

        unsafe {
            windows_and_messaging::PostThreadMessageW(
                self.thread_id,
                windows_and_messaging::WM_APP,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }

    pub fn bind<F>(&mut self, name: &str, f: F)
    where
        F: FnMut(&str, &str) + 'static,
    {
        self.bindings
            .lock()
            .unwrap()
            .insert(String::from(name), Box::new(f));

        let js = String::from(
            r#"
            (function() {
                var name = '"#,
        ) + name
            + r#"';
                var RPC = window._rpc = (window._rpc || {nextSeq: 1});
                window[name] = function() {
                    var seq = RPC.nextSeq++;
                    var promise = new Promise(function(resolve, reject) {
                        RPC[seq] = {
                            resolve: resolve,
                            reject: reject,
                        };
                    });
                    window.external.invoke(JSON.stringify({
                        id: seq,
                        method: name,
                        params: Array.prototype.slice.call(arguments),
                    }));
                    return promise;
                }
            })()"#;

        self.init(&js);
    }

    pub fn r#return(&self, seq: &str, status: i32, result: &str) {
        let seq = String::from(seq);
        let result = String::from(result);

        self.dispatch(move |webview| {
            let method = match status {
                0 => "resolve",
                _ => "reject",
            };
            let js = format!(
                r#"
                window._rpc["{}"].{}("{}");
                window._rpc["{}"] = undefined;"#,
                seq, method, result, seq
            );

            webview.eval(&js);
        });
    }
}
