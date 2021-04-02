use std::{
    collections::HashMap,
    ffi::CString,
    marker::PhantomData,
    mem, ptr,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc, Arc, Mutex, Once,
    },
};

use futures::{channel::oneshot, executor, task::LocalSpawnExt};

use serde::Deserialize;
use serde_json::Value;

use windows::{Abi, Interface};

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
                    self.webview.remove_WebMessageReceived(self.token).unwrap();
                    self.controller.Close().unwrap();
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
                        webview
                            .controller
                            .put_Bounds(RECT {
                                left: 0,
                                top: 0,
                                right: size.width,
                                bottom: size.height,
                            })
                            .unwrap();
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
            let handler = Box::new(CreateCoreWebView2EnvironmentCompletedHandler::new(
                Box::new(|error_code, environment| {
                    if error_code.is_ok() {
                        if let Some(environment) = environment {
                            context.send(environment);
                        }
                    }
                    windows::ErrorCode::S_OK
                }),
            ));

            unsafe {
                let handler: WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler =
                    from_abi(Box::into_raw(handler) as windows::RawPtr).unwrap();
                WebView2::CreateCoreWebView2Environment(handler).unwrap();
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
            let handler = Box::new(CreateCoreWebView2ControllerCompletedHandler::new(Box::new(
                |error_code, controller| {
                    if error_code.is_ok() {
                        if let Some(controller) = controller {
                            context.send(controller);
                        }
                    }
                    windows::ErrorCode::S_OK
                },
            )));

            unsafe {
                let handler: WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler =
                    from_abi(Box::into_raw(handler) as windows::RawPtr).unwrap();
                environment
                    .CreateCoreWebView2Controller(h_wnd, handler)
                    .unwrap();
            }

            Webview::run_one(&mut pool);

            pool.run_until(output).expect("receive the controller")
        };

        let size = Webview::get_window_size(h_wnd);
        unsafe {
            controller
                .put_Bounds(RECT {
                    left: 0,
                    top: 0,
                    right: size.width,
                    bottom: size.height,
                })
                .unwrap();
            controller.put_IsVisible(true).unwrap();
        }

        let mut webview = None;
        unsafe {
            controller.get_CoreWebView2(&mut webview).unwrap();
        }
        let webview = webview.expect("get_CoreWebView2");
        let bindings: Arc<Mutex<HashMap<String, Box<dyn FnMut(&str, &str)>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let bindings_ref = bindings.clone();
        let handler = Box::new(WebMessageReceivedEventHandler::new(Box::new(
            move |_sender, args| {
                let mut message = PWSTR::default();
                if let Some(args) = args {
                    if unsafe { args.get_WebMessageAsJson(&mut message) }.is_ok() {
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
        )));
        let mut token = EventRegistrationToken::default();
        unsafe {
            let handler: WebView2::ICoreWebView2WebMessageReceivedEventHandler =
                from_abi(Box::into_raw(handler) as windows::RawPtr).unwrap();
            webview.add_WebMessageReceived(handler, &mut token).unwrap();
        }

        if !debug {
            let mut settings = None;
            unsafe {
                if webview.get_Settings(&mut settings).is_ok() {
                    if let Some(settings) = settings {
                        settings.put_AreDevToolsEnabled(false).unwrap();
                        settings.put_AreDefaultContextMenusEnabled(false).unwrap();
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
            let handler = Box::new(NavigationCompletedEventHandler::new(Box::new(
                move |_sender, _args| {
                    if let Some(closure) = closure.take() {
                        closure();
                    }
                    windows::ErrorCode::S_OK
                },
            )));
            let mut token = EventRegistrationToken::default();
            unsafe {
                let handler: WebView2::ICoreWebView2NavigationCompletedEventHandler =
                    from_abi(Box::into_raw(handler) as windows::RawPtr).unwrap();
                self.webview
                    .add_NavigationCompleted(handler, &mut token)
                    .unwrap();
                self.webview.Navigate(url).unwrap();
            }

            Webview::run_one(&mut pool);

            pool.run_until(output).expect("completed the navigation");

            unsafe {
                self.webview.remove_NavigationCompleted(token).unwrap();
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
                        self.controller
                            .put_Bounds(RECT {
                                left: 0,
                                top: 0,
                                right: width,
                                bottom: height,
                            })
                            .unwrap();

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
        let handler = Box::new(AddScriptToExecuteOnDocumentCreatedCompletedHandler::new(
            Box::new(|error_code, id| {
                if error_code.is_ok() {
                    context.send(id);
                }
                windows::ErrorCode::S_OK
            }),
        ));

        unsafe {
            let handler: WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler =
                from_abi(Box::into_raw(handler) as windows::RawPtr).unwrap();
            self.webview
                .AddScriptToExecuteOnDocumentCreated(js, handler)
                .unwrap();
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
        let handler = Box::new(ExecuteScriptCompletedHandler::new(Box::new(
            |error_code, result| {
                if error_code.is_ok() {
                    context.send(result);
                }
                windows::ErrorCode::S_OK
            },
        )));

        unsafe {
            let handler: WebView2::ICoreWebView2ExecuteScriptCompletedHandler =
                from_abi(Box::into_raw(handler) as windows::RawPtr).unwrap();
            self.webview.ExecuteScript(js, handler).unwrap();
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

unsafe fn from_abi<I: Interface>(this: windows::RawPtr) -> windows::Result<I> {
    let unknown = windows::IUnknown::from_abi(this)?;
    unknown.vtable().1(unknown.abi()); // add_ref to balance the release called in IUnknown::drop
    Ok(unknown.cast()?)
}

fn string_from_pwstr(source: PWSTR) -> String {
    let mut buffer = Vec::new();
    let mut pwz = source.0;

    unsafe {
        while *pwz != 0 {
            buffer.push(*pwz);
            pwz = pwz.add(1);
        }
    }

    String::from_utf16(&buffer).expect("string_from_pwstr")
}

fn take_pwstr(source: PWSTR) -> String {
    let result = string_from_pwstr(source);

    if !source.0.is_null() {
        unsafe {
            Com::CoTaskMemFree(mem::transmute(source.0));
        }
    }

    result
}

trait CallbackInterface<I: Interface>: Sized {
    fn refcount(&self) -> &AtomicU32;

    unsafe extern "system" fn query_interface(
        this: windows::RawPtr,
        iid: &windows::Guid,
        interface: *mut windows::RawPtr,
    ) -> windows::ErrorCode {
        if interface.is_null() {
            windows::ErrorCode::E_POINTER
        } else if *iid == windows::IUnknown::IID || *iid == <I as Interface>::IID {
            Self::add_ref(this);
            *interface = this;
            windows::ErrorCode::S_OK
        } else {
            windows::ErrorCode::E_NOINTERFACE
        }
    }

    unsafe extern "system" fn add_ref(this: windows::RawPtr) -> u32 {
        let interface: *mut Self = mem::transmute(this);
        let count = (*interface).refcount().fetch_add(1, Ordering::Release) + 1;
        count
    }

    unsafe extern "system" fn release(this: windows::RawPtr) -> u32 {
        let interface: *mut Self = mem::transmute(this);
        let count = (*interface).refcount().fetch_sub(1, Ordering::Release) - 1;
        if count == 0 {
            // Destroy the underlying data
            Box::from_raw(interface);
        }
        count
    }
}

type CompletedClosure<Arg1, Arg2> = Box<dyn FnOnce(Arg1, Arg2) -> windows::ErrorCode>;

trait ClosureArg {
    type Input;
    type Output;

    fn convert(input: Self::Input) -> Self::Output;
}

trait CompletedCallback<I: Interface, Arg1: ClosureArg, Arg2: ClosureArg>:
    CallbackInterface<I>
{
    fn completed(&mut self) -> Option<CompletedClosure<Arg1::Output, Arg2::Output>>;

    unsafe extern "system" fn invoke(
        this: windows::RawPtr,
        arg_1: Arg1::Input,
        arg_2: Arg2::Input,
    ) -> windows::ErrorCode {
        let interface: *mut Self = mem::transmute(this);
        match (*interface).completed() {
            Some(completed) => completed(Arg1::convert(arg_1), Arg2::convert(arg_2)),
            None => windows::ErrorCode::S_OK,
        }
    }
}

type CreateCoreWebView2EnvironmentCompletedCallback =
    CompletedClosure<windows::ErrorCode, Option<WebView2::ICoreWebView2Environment>>;

#[repr(C)]
struct CreateCoreWebView2EnvironmentCompletedHandler {
    vtable: *const WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_abi,
    refcount: AtomicU32,
    completed: Option<CreateCoreWebView2EnvironmentCompletedCallback>,
}

impl CreateCoreWebView2EnvironmentCompletedHandler {
    pub fn new(completed: CreateCoreWebView2EnvironmentCompletedCallback) -> Self {
        static VTABLE: WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_abi =
            WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_abi(
                CreateCoreWebView2EnvironmentCompletedHandler::query_interface,
                CreateCoreWebView2EnvironmentCompletedHandler::add_ref,
                CreateCoreWebView2EnvironmentCompletedHandler::release,
                CreateCoreWebView2EnvironmentCompletedHandler::invoke,
            );

        Self {
            vtable: &VTABLE,
            refcount: AtomicU32::new(1),
            completed: Some(completed),
        }
    }
}

struct ErrorCodeArg();

impl ClosureArg for ErrorCodeArg {
    type Input = windows::ErrorCode;
    type Output = windows::ErrorCode;

    fn convert(input: windows::ErrorCode) -> windows::ErrorCode {
        input
    }
}

struct InterfaceArg<I: Interface>(PhantomData<I>);

impl<I: Interface> ClosureArg for InterfaceArg<I> {
    type Input = windows::RawPtr;
    type Output = Option<I>;

    fn convert(input: windows::RawPtr) -> Option<I> {
        if input.is_null() {
            None
        } else {
            match unsafe { from_abi(input) } {
                Ok(interface) => Some(interface),
                Err(_) => None,
            }
        }
    }
}

impl CallbackInterface<WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>
    for CreateCoreWebView2EnvironmentCompletedHandler
{
    fn refcount(&self) -> &AtomicU32 {
        &self.refcount
    }
}

impl
    CompletedCallback<
        WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler,
        ErrorCodeArg,
        InterfaceArg<WebView2::ICoreWebView2Environment>,
    > for CreateCoreWebView2EnvironmentCompletedHandler
{
    fn completed(&mut self) -> Option<CreateCoreWebView2EnvironmentCompletedCallback> {
        self.completed.take()
    }
}

type CreateCoreWebView2ControllerCompletedCallback =
    CompletedClosure<windows::ErrorCode, Option<WebView2::ICoreWebView2Controller>>;

#[repr(C)]
struct CreateCoreWebView2ControllerCompletedHandler {
    vtable: *const WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler_abi,
    refcount: AtomicU32,
    completed: Option<CreateCoreWebView2ControllerCompletedCallback>,
}

impl CreateCoreWebView2ControllerCompletedHandler {
    pub fn new(completed: CreateCoreWebView2ControllerCompletedCallback) -> Self {
        static VTABLE: WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler_abi =
            WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler_abi(
                CreateCoreWebView2ControllerCompletedHandler::query_interface,
                CreateCoreWebView2ControllerCompletedHandler::add_ref,
                CreateCoreWebView2ControllerCompletedHandler::release,
                CreateCoreWebView2ControllerCompletedHandler::invoke,
            );

        Self {
            vtable: &VTABLE,
            refcount: AtomicU32::new(1),
            completed: Some(completed),
        }
    }
}

impl CallbackInterface<WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>
    for CreateCoreWebView2ControllerCompletedHandler
{
    fn refcount(&self) -> &AtomicU32 {
        &self.refcount
    }
}

impl
    CompletedCallback<
        WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler,
        ErrorCodeArg,
        InterfaceArg<WebView2::ICoreWebView2Controller>,
    > for CreateCoreWebView2ControllerCompletedHandler
{
    fn completed(&mut self) -> Option<CreateCoreWebView2ControllerCompletedCallback> {
        self.completed.take()
    }
}

type EventClosure<Arg1, Arg2> = Box<dyn FnMut(Arg1, Arg2) -> windows::ErrorCode>;

trait EventCallback<I: Interface, Arg1: ClosureArg, Arg2: ClosureArg>: CallbackInterface<I> {
    fn event(&mut self) -> &mut EventClosure<Arg1::Output, Arg2::Output>;

    unsafe extern "system" fn invoke(
        this: windows::RawPtr,
        arg_1: Arg1::Input,
        arg_2: Arg2::Input,
    ) -> windows::ErrorCode {
        let interface: *mut Self = mem::transmute(this);
        ((*interface).event())(Arg1::convert(arg_1), Arg2::convert(arg_2))
    }
}

type WebMessageReceivedEventCallback = EventClosure<
    Option<WebView2::ICoreWebView2>,
    Option<WebView2::ICoreWebView2WebMessageReceivedEventArgs>,
>;

#[repr(C)]
struct WebMessageReceivedEventHandler {
    vtable: *const WebView2::ICoreWebView2WebMessageReceivedEventHandler_abi,
    refcount: AtomicU32,
    event: WebMessageReceivedEventCallback,
}

impl WebMessageReceivedEventHandler {
    pub fn new(event: WebMessageReceivedEventCallback) -> Self {
        static VTABLE: WebView2::ICoreWebView2WebMessageReceivedEventHandler_abi =
            WebView2::ICoreWebView2WebMessageReceivedEventHandler_abi(
                WebMessageReceivedEventHandler::query_interface,
                WebMessageReceivedEventHandler::add_ref,
                WebMessageReceivedEventHandler::release,
                WebMessageReceivedEventHandler::invoke,
            );

        Self {
            vtable: &VTABLE,
            refcount: AtomicU32::new(1),
            event,
        }
    }
}

impl CallbackInterface<WebView2::ICoreWebView2WebMessageReceivedEventHandler>
    for WebMessageReceivedEventHandler
{
    fn refcount(&self) -> &AtomicU32 {
        &self.refcount
    }
}

impl
    EventCallback<
        WebView2::ICoreWebView2WebMessageReceivedEventHandler,
        InterfaceArg<WebView2::ICoreWebView2>,
        InterfaceArg<WebView2::ICoreWebView2WebMessageReceivedEventArgs>,
    > for WebMessageReceivedEventHandler
{
    fn event(&mut self) -> &mut WebMessageReceivedEventCallback {
        &mut self.event
    }
}

type NavigationCompletedEventCallback = EventClosure<
    Option<WebView2::ICoreWebView2>,
    Option<WebView2::ICoreWebView2NavigationCompletedEventArgs>,
>;

#[repr(C)]
struct NavigationCompletedEventHandler {
    vtable: *const WebView2::ICoreWebView2NavigationCompletedEventHandler_abi,
    refcount: AtomicU32,
    event: NavigationCompletedEventCallback,
}

impl NavigationCompletedEventHandler {
    pub fn new(event: NavigationCompletedEventCallback) -> Self {
        static VTABLE: WebView2::ICoreWebView2NavigationCompletedEventHandler_abi =
            WebView2::ICoreWebView2NavigationCompletedEventHandler_abi(
                NavigationCompletedEventHandler::query_interface,
                NavigationCompletedEventHandler::add_ref,
                NavigationCompletedEventHandler::release,
                NavigationCompletedEventHandler::invoke,
            );

        Self {
            vtable: &VTABLE,
            refcount: AtomicU32::new(1),
            event,
        }
    }
}

impl CallbackInterface<WebView2::ICoreWebView2NavigationCompletedEventHandler>
    for NavigationCompletedEventHandler
{
    fn refcount(&self) -> &AtomicU32 {
        &self.refcount
    }
}

impl
    EventCallback<
        WebView2::ICoreWebView2NavigationCompletedEventHandler,
        InterfaceArg<WebView2::ICoreWebView2>,
        InterfaceArg<WebView2::ICoreWebView2NavigationCompletedEventArgs>,
    > for NavigationCompletedEventHandler
{
    fn event(&mut self) -> &mut NavigationCompletedEventCallback {
        &mut self.event
    }
}

struct StringArg();

impl ClosureArg for StringArg {
    type Input = PWSTR;
    type Output = String;

    fn convert(input: PWSTR) -> String {
        string_from_pwstr(input)
    }
}

type AddScriptToExecuteOnDocumentCreatedCompletedCallback =
    CompletedClosure<windows::ErrorCode, String>;

#[repr(C)]
struct AddScriptToExecuteOnDocumentCreatedCompletedHandler {
    vtable: *const WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler_abi,
    refcount: AtomicU32,
    completed: Option<AddScriptToExecuteOnDocumentCreatedCompletedCallback>,
}

impl AddScriptToExecuteOnDocumentCreatedCompletedHandler {
    pub fn new(completed: AddScriptToExecuteOnDocumentCreatedCompletedCallback) -> Self {
        static VTABLE:
            WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler_abi =
            WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler_abi(
                AddScriptToExecuteOnDocumentCreatedCompletedHandler::query_interface,
                AddScriptToExecuteOnDocumentCreatedCompletedHandler::add_ref,
                AddScriptToExecuteOnDocumentCreatedCompletedHandler::release,
                AddScriptToExecuteOnDocumentCreatedCompletedHandler::invoke,
            );

        Self {
            vtable: &VTABLE,
            refcount: AtomicU32::new(1),
            completed: Some(completed),
        }
    }
}

impl CallbackInterface<WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler>
    for AddScriptToExecuteOnDocumentCreatedCompletedHandler
{
    fn refcount(&self) -> &AtomicU32 {
        &self.refcount
    }
}

impl
    CompletedCallback<
        WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler,
        ErrorCodeArg,
        StringArg,
    > for AddScriptToExecuteOnDocumentCreatedCompletedHandler
{
    fn completed(&mut self) -> Option<AddScriptToExecuteOnDocumentCreatedCompletedCallback> {
        self.completed.take()
    }
}

type ExecuteScriptCompletedCallback = CompletedClosure<windows::ErrorCode, String>;

#[repr(C)]
struct ExecuteScriptCompletedHandler {
    vtable: *const WebView2::ICoreWebView2ExecuteScriptCompletedHandler_abi,
    refcount: AtomicU32,
    completed: Option<ExecuteScriptCompletedCallback>,
}

impl ExecuteScriptCompletedHandler {
    pub fn new(completed: ExecuteScriptCompletedCallback) -> Self {
        static VTABLE: WebView2::ICoreWebView2ExecuteScriptCompletedHandler_abi =
            WebView2::ICoreWebView2ExecuteScriptCompletedHandler_abi(
                ExecuteScriptCompletedHandler::query_interface,
                ExecuteScriptCompletedHandler::add_ref,
                ExecuteScriptCompletedHandler::release,
                ExecuteScriptCompletedHandler::invoke,
            );

        Self {
            vtable: &VTABLE,
            refcount: AtomicU32::new(1),
            completed: Some(completed),
        }
    }
}

impl CallbackInterface<WebView2::ICoreWebView2ExecuteScriptCompletedHandler>
    for ExecuteScriptCompletedHandler
{
    fn refcount(&self) -> &AtomicU32 {
        &self.refcount
    }
}

impl
    CompletedCallback<WebView2::ICoreWebView2ExecuteScriptCompletedHandler, ErrorCodeArg, StringArg>
    for ExecuteScriptCompletedHandler
{
    fn completed(&mut self) -> Option<ExecuteScriptCompletedCallback> {
        self.completed.take()
    }
}
