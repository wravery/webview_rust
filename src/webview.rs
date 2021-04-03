use std::{
    collections::HashMap,
    ffi::CString,
    mem, ptr,
    sync::{mpsc, Arc, Mutex, Once},
};

use futures::{channel::oneshot, executor, task::LocalSpawnExt};

use serde::Deserialize;
use serde_json::Value;

use bindings::Windows::Win32::{
    Com, Debug,
    DisplayDevices::{POINT, RECT},
    Gdi,
    HiDpi::{self, PROCESS_DPI_AWARENESS},
    KeyboardAndMouseInput,
    MenusAndResources::HMENU,
    SystemServices::{self, HINSTANCE, LRESULT, PSTR, PWSTR},
    WebView2,
    WinRT::EventRegistrationToken,
    WindowsAndMessaging::{
        self, HWND, LPARAM, MINMAXINFO, SHOW_WINDOW_CMD, WINDOW_EX_STYLE, WINDOW_LONG_PTR_INDEX,
        WINDOW_STYLE, WNDCLASSA, WPARAM,
    },
};

use crate::callback;

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
    size: Arc<Mutex<WindowSize>>,
    max_size: Arc<Mutex<Option<WindowSize>>>,
    min_size: Arc<Mutex<Option<WindowSize>>>,
}

impl Drop for FrameWindow {
    fn drop(&mut self) {
        if Arc::strong_count(&self.window) == 0 {
            unsafe {
                WindowsAndMessaging::DestroyWindow(self.window.0);
                WindowsAndMessaging::PostQuitMessage(0);
            }
        }
    }
}

#[derive(Clone)]
pub struct Webview {
    controller: WebView2::ICoreWebView2Controller,
    webview: WebView2::ICoreWebView2,
    tx: mpsc::Sender<Box<dyn FnOnce(Webview) + Send>>,
    rx: Arc<mpsc::Receiver<Box<dyn FnOnce(Webview) + Send>>>,
    thread_id: u32,
    token: EventRegistrationToken,
    bindings: Arc<Mutex<HashMap<String, Box<dyn FnMut(&str, &str)>>>>,
    frame: Option<FrameWindow>,
    parent: Arc<Window>,
    url: Arc<Mutex<String>>,
}

unsafe impl Send for Webview {}
unsafe impl Sync for Webview {}

impl Drop for Webview {
    fn drop(&mut self) {
        match Arc::strong_count(&self.parent) {
            0 => {
                unsafe {
                    self.webview.remove_WebMessageReceived(self.token);
                    self.controller.Close();
                }

                if self.frame.is_none() {
                    unsafe {
                        WindowsAndMessaging::PostQuitMessage(0);
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

#[derive(Debug, Deserialize)]
struct InvokeMessage {
    id: u64,
    method: String,
    params: Vec<Value>,
}

impl Webview {
    fn run_one(pool: &mut executor::LocalPool) {
        let mut msg = WindowsAndMessaging::MSG::default();
        let h_wnd = WindowsAndMessaging::HWND::default();

        loop {
            if pool.try_run_one() {
                break;
            }

            unsafe {
                match WindowsAndMessaging::GetMessageA(&mut msg, h_wnd, 0, 0).0 {
                    -1 => println!("GetMessageW failed: {}", Debug::GetLastError()),
                    0 => break,
                    _ => {
                        WindowsAndMessaging::TranslateMessage(&msg);
                        WindowsAndMessaging::DispatchMessageA(&msg);
                    }
                }
            }
        }
    }

    #[cfg(target_pointer_width = "32")]
    fn get_window_extra_size() -> i32 {
        // It'll fit in a single entry for GWLP_USERDATA
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
            WindowsAndMessaging::SetWindowLongA(
                h_wnd,
                WINDOW_LONG_PTR_INDEX::GWLP_USERDATA,
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
            WindowsAndMessaging::SetWindowLongA(
                h_wnd,
                WINDOW_LONG_PTR_INDEX::GWLP_USERDATA,
                low as _,
            );
            WindowsAndMessaging::SetWindowLongA(h_wnd, WINDOW_LONG_PTR_INDEX(0), high as _);
        }
    }

    #[cfg(target_pointer_width = "32")]
    fn get_window_webview(h_wnd: HWND) -> Option<Box<Webview>> {
        unsafe {
            let data =
                WindowsAndMessaging::GetWindowLongA(h_wnd, WINDOW_LONG_PTR_INDEX::GWLP_USERDATA);

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
                WindowsAndMessaging::GetWindowLongA(h_wnd, WINDOW_LONG_PTR_INDEX::GWLP_USERDATA)
                    as u32;
            let high = WindowsAndMessaging::GetWindowLongA(h_wnd, WINDOW_LONG_PTR_INDEX(0)) as u32;

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
        unsafe { WindowsAndMessaging::GetClientRect(h_wnd, &mut client_rect) };
        WindowSize {
            width: client_rect.right - client_rect.left,
            height: client_rect.bottom - client_rect.top,
        }
    }

    fn create_frame() -> FrameWindow {
        unsafe {
            let _code =
                HiDpi::SetProcessDpiAwareness(PROCESS_DPI_AWARENESS::PROCESS_PER_MONITOR_DPI_AWARE);
        }

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
                        WindowsAndMessaging::DefWindowProcA(h_wnd, msg, w_param, l_param)
                    }
                }
            };

            let frame = webview
                .frame
                .as_ref()
                .expect("should only be called for owned windows");

            match msg {
                WindowsAndMessaging::WM_SIZE => {
                    let size = Webview::get_window_size(h_wnd);
                    unsafe {
                        webview.controller.put_Bounds(RECT {
                            left: 0,
                            top: 0,
                            right: size.width,
                            bottom: size.height,
                        });
                    }
                    *frame.size.lock().expect("lock size") = size;
                    LRESULT(0)
                }

                WindowsAndMessaging::WM_CLOSE => {
                    unsafe {
                        WindowsAndMessaging::DestroyWindow(h_wnd);
                    }
                    LRESULT(0)
                }

                WindowsAndMessaging::WM_DESTROY => {
                    webview.terminate();
                    LRESULT(0)
                }

                WindowsAndMessaging::WM_GETMINMAXINFO => {
                    if l_param.0 != 0 {
                        let min_max_info: *mut MINMAXINFO = unsafe { mem::transmute(l_param.0) };

                        if let Some(max) = frame.max_size.lock().expect("lock max_size").as_ref() {
                            let max_size = POINT {
                                x: max.width,
                                y: max.height,
                            };

                            unsafe {
                                (*min_max_info).ptMaxSize = max_size;
                                (*min_max_info).ptMaxTrackSize = max_size;
                            }
                        }

                        if let Some(min) = frame.min_size.lock().expect("lock max_size").as_ref() {
                            unsafe {
                                (*min_max_info).ptMinTrackSize = POINT {
                                    x: min.width,
                                    y: min.height,
                                };
                            }
                        }
                    }

                    LRESULT(0)
                }

                _ => unsafe { WindowsAndMessaging::DefWindowProcA(h_wnd, msg, w_param, l_param) },
            }
        }

        let h_wnd = {
            let class_name = "Webview";
            let c_class_name = CString::new(class_name).expect("convert");

            let mut window_class = WNDCLASSA::default();
            window_class.lpfnWndProc = Some(window_proc);
            window_class.lpszClassName = PSTR(c_class_name.as_ptr() as *mut _);
            window_class.cbWndExtra = Webview::get_window_extra_size();

            unsafe {
                WindowsAndMessaging::RegisterClassA(&window_class);

                WindowsAndMessaging::CreateWindowExA(
                    WINDOW_EX_STYLE(0),
                    class_name,
                    class_name,
                    WINDOW_STYLE::WS_OVERLAPPEDWINDOW,
                    WindowsAndMessaging::CW_USEDEFAULT,
                    WindowsAndMessaging::CW_USEDEFAULT,
                    640,
                    480,
                    HWND(0),
                    HMENU(0),
                    HINSTANCE(SystemServices::GetModuleHandleA(PSTR(ptr::null_mut()))),
                    ptr::null_mut(),
                )
            }
        };

        FrameWindow {
            window: Arc::new(Window(h_wnd)),
            size: Arc::new(Mutex::new(WindowSize {
                width: 0,
                height: 0,
            })),
            min_size: Arc::new(Mutex::new(None)),
            max_size: Arc::new(Mutex::new(None)),
        }
    }

    pub fn create(debug: bool, window: Option<Window>) -> Webview {
        static COM_INIT: Once = Once::new();

        COM_INIT.call_once(|| {
            windows::initialize_sta().expect("initialize COM");
        });

        let frame = match window {
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
            unsafe {
                let handler = callback::create::<
                    callback::CreateCoreWebView2EnvironmentCompletedHandler,
                >(Box::new(|error_code, environment| {
                    if error_code.is_ok() {
                        if let Some(environment) = environment {
                            context.send(environment);
                        }
                    }
                    windows::ErrorCode::S_OK
                }))
                .unwrap();

                WebView2::CreateCoreWebView2Environment(handler);
            };

            Webview::run_one(&mut pool);

            pool.run_until(output).expect("receive the environment")
        };

        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        let controller = {
            unsafe {
                let handler = callback::create::<
                    callback::CreateCoreWebView2ControllerCompletedHandler,
                >(Box::new(|error_code, controller| {
                    if error_code.is_ok() {
                        if let Some(controller) = controller {
                            context.send(controller);
                        }
                    }
                    windows::ErrorCode::S_OK
                }))
                .unwrap();
                environment.CreateCoreWebView2Controller(h_wnd, handler);
            }

            Webview::run_one(&mut pool);

            pool.run_until(output).expect("receive the controller")
        };

        let size = Webview::get_window_size(h_wnd);
        unsafe {
            controller.put_Bounds(RECT {
                left: 0,
                top: 0,
                right: size.width,
                bottom: size.height,
            });
            controller.put_IsVisible(true);
        }

        let mut webview = None;
        unsafe {
            controller.get_CoreWebView2(&mut webview);
        }
        let webview = webview.expect("get_CoreWebView2");
        let bindings: Arc<Mutex<HashMap<String, Box<dyn FnMut(&str, &str)>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let bindings_ref = bindings.clone();
        let mut token = EventRegistrationToken::default();
        unsafe {
            let handler = callback::create::<callback::WebMessageReceivedEventHandler>(Box::new(
                move |_sender, args| {
                    let mut message = PWSTR::default();
                    if let Some(args) = args {
                        let code = windows::ErrorCode(
                            args.get_WebMessageAsJson(&mut message).0 as u32,
                        );
                        if code.is_ok() {
                            let message = take_pwstr(message);
                            if let Ok(value) = serde_json::from_str::<InvokeMessage>(&message) {
                                let mut bindings = bindings_ref.lock().expect("lock bindings");
                                if let Some(f) = bindings.get_mut(&value.method) {
                                    let id = serde_json::to_string(&value.id).unwrap();
                                    let params = serde_json::to_string(&value.params).unwrap();
                                    (*f)(&id, &params);
                                }
                            }
                        }
                    }

                    windows::ErrorCode::S_OK
                },
            ))
            .unwrap();
            webview.add_WebMessageReceived(handler, &mut token);
        }

        if !debug {
            let mut settings = None;
            unsafe {
                let code = windows::ErrorCode(webview.get_Settings(&mut settings).0 as u32);
                if code.is_ok() {
                    if let Some(settings) = settings {
                        settings.put_AreDevToolsEnabled(false);
                        settings.put_AreDefaultContextMenusEnabled(false);
                    }
                }
            }
        }

        if let Some(frame) = frame.as_ref() {
            *frame.size.lock().expect("lock size") = size;
        }

        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(rx);
        let thread_id = unsafe { SystemServices::GetCurrentThreadId() };
        let parent = Window(h_wnd);

        let webview = Webview {
            controller,
            webview,
            tx,
            rx,
            thread_id,
            token,
            bindings,
            frame,
            parent: Arc::new(parent),
            url: Arc::new(Mutex::new(String::new())),
        };

        // Inject the invoke handler.
        webview.init(r#"window.external={invoke:s=>window.chrome.webview.postMessage(s)}"#);

        if webview.frame.is_some() {
            Webview::set_window_webview(h_wnd, Box::new(webview.clone()));
        }

        webview
    }

    pub fn run(&self) {
        let url = self.url.lock().expect("lock url").clone();
        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");

        if !url.is_empty() {
            let mut closure = Some(move || {
                context.send(());
            });
            let mut token = EventRegistrationToken::default();
            unsafe {
                let handler = callback::create::<callback::NavigationCompletedEventHandler>(
                    Box::new(move |_sender, _args| {
                        if let Some(closure) = closure.take() {
                            closure();
                        }
                        windows::ErrorCode::S_OK
                    }),
                )
                .unwrap();
                self.webview.add_NavigationCompleted(handler, &mut token);
                self.webview.Navigate(url);
            }

            Webview::run_one(&mut pool);

            pool.run_until(output).expect("completed the navigation");

            unsafe {
                self.webview.remove_NavigationCompleted(token);
            }
        }

        if let Some(frame) = self.frame.as_ref() {
            let h_wnd = frame.window.0;
            unsafe {
                WindowsAndMessaging::ShowWindow(h_wnd, SHOW_WINDOW_CMD::SW_SHOW);
                Gdi::UpdateWindow(h_wnd);
                KeyboardAndMouseInput::SetFocus(h_wnd);
            }
        }

        let mut msg = WindowsAndMessaging::MSG::default();
        let h_wnd = WindowsAndMessaging::HWND::default();

        loop {
            while let Ok(f) = self.rx.try_recv() {
                (f)(self.clone());
            }

            unsafe {
                let result = WindowsAndMessaging::GetMessageA(&mut msg, h_wnd, 0, 0).0;

                match result {
                    -1 => println!("GetMessageW failed: {}", Debug::GetLastError()),
                    0 => break,
                    _ => match msg.message {
                        WindowsAndMessaging::WM_APP => (),
                        _ => {
                            WindowsAndMessaging::TranslateMessage(&msg);
                            WindowsAndMessaging::DispatchMessageA(&msg);
                        }
                    },
                }
            }
        }
    }

    pub fn terminate(&self) {
        self.dispatch(|_webview| unsafe {
            WindowsAndMessaging::PostQuitMessage(0);
        });
    }

    // TODO Window instance
    pub fn set_title(&self, title: &str) {
        match self.frame.as_ref() {
            Some(frame) => unsafe {
                WindowsAndMessaging::SetWindowTextA(frame.window.0, title);
            },
            None => (),
        }
    }

    pub fn set_size(&self, width: i32, height: i32, hints: SizeHint) {
        match self.frame.as_ref() {
            Some(frame) => match hints {
                SizeHint::MIN => {
                    *frame.min_size.lock().expect("lock min_size") =
                        Some(WindowSize { width, height });
                }
                SizeHint::MAX => {
                    *frame.max_size.lock().expect("lock max_size") =
                        Some(WindowSize { width, height });
                }
                _ => {
                    *frame.size.lock().expect("lock size") = WindowSize { width, height };

                    unsafe {
                        self.controller.put_Bounds(RECT {
                            left: 0,
                            top: 0,
                            right: width,
                            bottom: height,
                        });

                        WindowsAndMessaging::SetWindowPos(
                            frame.window.0,
                            HWND(0),
                            0,
                            0,
                            width,
                            height,
                            WindowsAndMessaging::SetWindowPos_uFlags::SWP_NOACTIVATE
                                | WindowsAndMessaging::SetWindowPos_uFlags::SWP_NOZORDER
                                | WindowsAndMessaging::SetWindowPos_uFlags::SWP_NOMOVE,
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

    pub fn navigate(&self, url: &str) {
        *self.url.lock().expect("lock url") = url.to_string();
    }

    pub fn init(&self, js: &str) {
        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");
        unsafe {
            let handler = callback::create::<
                callback::AddScriptToExecuteOnDocumentCreatedCompletedHandler,
            >(Box::new(|error_code, id| {
                if error_code.is_ok() {
                    context.send(id);
                }
                windows::ErrorCode::S_OK
            }))
            .unwrap();

            self.webview
                .AddScriptToExecuteOnDocumentCreated(js, handler);
        };

        Webview::run_one(&mut pool);

        pool.run_until(output).expect("receive the id");
    }

    pub fn eval(&self, js: &str) {
        let (tx, rx) = oneshot::channel();
        let context = Box::new(MessageLoopCompletedContext::new(tx));
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");
        unsafe {
            let handler = callback::create::<callback::ExecuteScriptCompletedHandler>(Box::new(
                |error_code, result| {
                    if error_code.is_ok() {
                        context.send(result);
                    }
                    windows::ErrorCode::S_OK
                },
            ))
            .unwrap();

            self.webview.ExecuteScript(js, handler);
        };

        Webview::run_one(&mut pool);

        pool.run_until(output).expect("receive the result");
    }

    pub fn dispatch<F>(&self, f: F)
    where
        F: FnOnce(Webview) + Send + 'static,
    {
        self.tx.send(Box::new(f)).expect("send the fn");

        unsafe {
            WindowsAndMessaging::PostThreadMessageA(
                self.thread_id,
                WindowsAndMessaging::WM_APP,
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
                    window.external.invoke({
                        id: seq,
                        method: name,
                        params: Array.prototype.slice.call(arguments),
                    });
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

fn take_pwstr(source: PWSTR) -> String {
    let result = callback::string_from_pwstr(source);

    if !source.0.is_null() {
        unsafe {
            Com::CoTaskMemFree(mem::transmute(source.0));
        }
    }

    result
}
