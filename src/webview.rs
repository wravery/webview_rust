use std::{
    collections::HashMap,
    ffi::CString,
    mem,
    sync::{mpsc, Arc, Mutex, Once},
};

use serde::Deserialize;
use serde_json::Value;

use bindings::{
    microsoft::web::web_view2::core::*,
    windows::{
        foundation::*,
        win32::{
            self,
            display_devices::{POINT, RECT, SIZE},
            gdi,
            hi_dpi::{self, PROCESS_DPI_AWARENESS},
            keyboard_and_mouse_input,
            menus_and_resources::HMENU,
            system_services::{self, HINSTANCE, LRESULT, PSTR},
            windows_and_messaging::{
                self, SetWindowLong_nIndex, SetWindowPos_uFlags, HWND, LPARAM, MINMAXINFO, MSG,
                SHOW_WINDOW_CMD, WINDOWS_EX_STYLE, WINDOWS_STYLE, WNDCLASSA, WPARAM,
            },
        },
    },
};

use windows::*;

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

#[derive(Clone)]
pub struct FrameWindow {
    window: Arc<Window>,
    size: Arc<Mutex<SIZE>>,
    max_size: Arc<Mutex<Option<SIZE>>>,
    min_size: Arc<Mutex<Option<SIZE>>>,
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
    controller: Arc<CoreWebView2Controller>,
    webview: Arc<CoreWebView2>,
    tx: mpsc::Sender<Box<dyn FnOnce(Webview) + Send>>,
    rx: Arc<mpsc::Receiver<Box<dyn FnOnce(Webview) + Send>>>,
    thread_id: u32,
    token: EventRegistrationToken,
    bindings: Arc<Mutex<HashMap<String, Box<dyn FnMut(&str, &str)>>>>,
    frame: Option<FrameWindow>,
    parent: Arc<Window>,
    url: Arc<Mutex<HString>>,
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
#[derive(Debug, Deserialize)]
struct InvokeMessage {
    id: u64,
    method: String,
    params: Vec<Value>,
}

impl Webview {
    fn run_one<T>(rx: mpsc::Receiver<T>) -> Result<T> {
        let mut msg = MSG::default();
        let h_wnd = HWND::default();

        loop {
            if let Ok(result) = rx.try_recv() {
                return Ok(result);
            }

            unsafe {
                match windows_and_messaging::GetMessageA(&mut msg, h_wnd, 0, 0).0 {
                    -1 => {
                        let error = format!("GetMessageW failed: {}", win32::debug::GetLastError());
                        return Err(Error::new(ErrorCode::E_NOINTERFACE, &error));
                    }
                    0 => return Err(Error::new(ErrorCode::E_NOINTERFACE, "task canceled")),
                    _ => {
                        windows_and_messaging::TranslateMessage(&msg);
                        windows_and_messaging::DispatchMessageA(&msg);
                    }
                }
            }
        }
    }

    #[cfg(target_pointer_width = "32")]
    const fn get_window_extra_size() -> i32 {
        // It'll fit in a single entry for GWL_USERDATA
        0
    }

    #[cfg(target_pointer_width = "64")]
    const fn get_window_extra_size() -> i32 {
        // We need an extra 4 bytes since the win32 bindings don't expose Get/SetWindowLongPtr
        (mem::size_of::<isize>() - mem::size_of::<i32>()) as i32
    }

    const CB_EXTRA: i32 = Webview::get_window_extra_size();

    #[cfg(target_pointer_width = "32")]
    fn set_window_webview(h_wnd: HWND, webview: Box<Webview>) {
        unsafe {
            windows_and_messaging::SetWindowLongA(
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
            windows_and_messaging::SetWindowLongA(
                h_wnd,
                SetWindowLong_nIndex::GWL_USERDATA,
                low as _,
            );
            windows_and_messaging::SetWindowLongA(h_wnd, SetWindowLong_nIndex(0), high as _);
        }
    }

    #[cfg(target_pointer_width = "32")]
    fn get_window_webview(h_wnd: HWND) -> Option<Box<Webview>> {
        unsafe {
            let data =
                windows_and_messaging::GetWindowLongA(h_wnd, SetWindowLong_nIndex::GWL_USERDATA);

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
                windows_and_messaging::GetWindowLongA(h_wnd, SetWindowLong_nIndex::GWL_USERDATA)
                    as u32;
            let high = windows_and_messaging::GetWindowLongA(h_wnd, SetWindowLong_nIndex(0)) as u32;

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

    fn get_window_size(h_wnd: HWND) -> SIZE {
        let mut client_rect = RECT::default();
        unsafe { windows_and_messaging::GetClientRect(h_wnd, &mut client_rect) };
        SIZE {
            cx: client_rect.right - client_rect.left,
            cy: client_rect.bottom - client_rect.top,
        }
    }

    fn create_frame() -> FrameWindow {
        unsafe {
            let _code = hi_dpi::SetProcessDpiAwareness(
                PROCESS_DPI_AWARENESS::PROCESS_PER_MONITOR_DPI_AWARE,
            );
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
                        windows_and_messaging::DefWindowProcA(h_wnd, msg, w_param, l_param)
                    }
                }
            };

            let frame = webview
                .frame
                .as_ref()
                .expect("should only be called for owned windows");

            match msg {
                windows_and_messaging::WM_SIZE => {
                    let size = Webview::get_window_size(h_wnd);
                    webview
                        .controller
                        .set_bounds(Rect {
                            x: 0f32,
                            y: 0f32,
                            width: size.cx as f32,
                            height: size.cy as f32,
                        })
                        .expect("call set_bounds");
                    *frame.size.lock().expect("lock size") = size;
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
                    if l_param.0 != 0 {
                        let min_max_info: *mut MINMAXINFO = l_param.0 as *mut _;

                        if let Some(max) = frame.max_size.lock().expect("lock max_size").as_ref() {
                            let max = POINT {
                                x: max.cx,
                                y: max.cy,
                            };
                            unsafe {
                                (*min_max_info).pt_max_size = max;
                                (*min_max_info).pt_max_track_size = max;
                            }
                        }

                        if let Some(min) = frame.min_size.lock().expect("lock max_size").as_ref() {
                            let min = POINT {
                                x: min.cx,
                                y: min.cy,
                            };
                            unsafe {
                                (*min_max_info).pt_min_track_size = min;
                            }
                        }
                    }

                    LRESULT(0)
                }

                _ => unsafe { windows_and_messaging::DefWindowProcA(h_wnd, msg, w_param, l_param) },
            }
        }

        let h_wnd = {
            let class_name = CString::new("Webview").expect("lpsz_class_name");
            let mut window_class = WNDCLASSA::default();
            window_class.lpfn_wnd_proc = Some(window_proc);
            window_class.lpsz_class_name = PSTR(class_name.as_ptr() as *mut _);
            window_class.cb_wnd_extra = Webview::CB_EXTRA;

            unsafe {
                windows_and_messaging::RegisterClassA(&window_class);

                windows_and_messaging::CreateWindowExA(
                    WINDOWS_EX_STYLE(0),
                    PSTR(class_name.as_ptr() as *mut _),
                    PSTR(class_name.as_ptr() as *mut _),
                    WINDOWS_STYLE::WS_OVERLAPPEDWINDOW,
                    windows_and_messaging::CW_USEDEFAULT,
                    windows_and_messaging::CW_USEDEFAULT,
                    640,
                    480,
                    HWND(0),
                    HMENU(0),
                    HINSTANCE(system_services::GetModuleHandleA(PSTR(0 as *mut _))),
                    0 as *mut _,
                )
            }
        };

        FrameWindow {
            window: Arc::new(Window(h_wnd)),
            size: Arc::new(Mutex::new(SIZE { cx: 0, cy: 0 })),
            min_size: Arc::new(Mutex::new(None)),
            max_size: Arc::new(Mutex::new(None)),
        }
    }

    pub fn create(debug: bool, window: Option<Window>) -> Result<Webview> {
        static COM_INIT: Once = Once::new();

        COM_INIT.call_once(|| unsafe {
            assert!(win32::com::CoInitialize(0 as *mut _).is_ok());
        });

        let frame = match window {
            Some(Window(_)) => None,
            None => Some(Webview::create_frame()),
        };

        let h_wnd = match window {
            Some(Window(h_wnd)) => h_wnd,
            None => frame.as_ref().unwrap().window.0,
        };

        let (tx, rx) = mpsc::channel();
        let environment = {
            CoreWebView2Environment::create_async()?.set_completed(
                AsyncOperationCompletedHandler::new(move |op, _status| {
                    if let Some(op) = op {
                        tx.send(op.get_results()?).expect("send over mpsc");
                    }
                    Ok(())
                }),
            )?;

            Webview::run_one(rx)
        }?;

        let (tx, rx) = mpsc::channel();
        let controller = {
            environment
                .create_core_web_view2_controller_async(
                    CoreWebView2ControllerWindowReference::create_from_window_handle(
                        h_wnd.0 as u64,
                    )?,
                )?
                .set_completed(AsyncOperationCompletedHandler::new(move |op, _status| {
                    if let Some(op) = op.as_ref() {
                        tx.send(op.get_results()?).expect("send over mpsc");
                    }
                    Ok(())
                }))?;

            Webview::run_one(rx)
        }?;

        let size = Webview::get_window_size(h_wnd);
        let mut client_rect = RECT::default();
        unsafe { windows_and_messaging::GetClientRect(h_wnd, &mut client_rect) };
        controller.set_bounds(Rect {
            x: 0f32,
            y: 0f32,
            width: size.cx as f32,
            height: size.cy as f32,
        })?;
        controller.set_is_visible(true)?;

        let webview = controller.core_web_view2()?;

        if !debug {
            let settings = webview.settings()?;
            settings.set_are_dev_tools_enabled(false)?;
            settings.set_are_default_context_menus_enabled(false)?;
        }

        let bindings: Arc<Mutex<HashMap<String, Box<dyn FnMut(&str, &str)>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let bindings_ref = bindings.clone();
        let token = webview.web_message_received(TypedEventHandler::<
            CoreWebView2,
            CoreWebView2WebMessageReceivedEventArgs,
        >::new(move |_sender, args| {
            println!("WebMessageReceived...");
            if let Some(args) = args {
                if let Ok(message) = String::from_utf16(args.web_message_as_json()?.as_wide()) {
                    println!("{}", message);
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
            Ok(())
        }))?;
        println!("Registered web_message_received: {:#?}", token);

        if let Some(frame) = frame.as_ref() {
            *frame.size.lock().expect("lock size") = size;
        }

        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(rx);
        let thread_id = unsafe { system_services::GetCurrentThreadId() };
        let parent = Window(h_wnd);

        let webview = Webview {
            controller: Arc::new(controller),
            webview: Arc::new(webview),
            tx,
            rx,
            thread_id,
            token,
            bindings,
            frame,
            parent: Arc::new(parent),
            url: Arc::new(Mutex::new(HString::new())),
        };

        // Inject the invoke handler.
        webview.init(r#"window.external={invoke:s=>window.chrome.webview.postMessage(s)}"#)?;

        if webview.frame.is_some() {
            Webview::set_window_webview(h_wnd, Box::new(webview.clone()));
        }

        Ok(webview)
    }

    pub fn run(&self) -> Result<()> {
        let webview = self.webview.as_ref();
        let url = self.url.lock().expect("lock url").clone();
        let (tx, rx) = mpsc::channel();

        if url.len() > 0 {
            let token =
                webview.navigation_completed(TypedEventHandler::<
                    CoreWebView2,
                    CoreWebView2NavigationCompletedEventArgs,
                >::new(move |_sender, _args| {
                    tx.send(()).expect("send over mpsc");
                    Ok(())
                }))?;

            webview.navigate(&url)?;

            let result = Webview::run_one(rx);
            webview.remove_navigation_completed(token)?;
            result?;
        }

        if let Some(frame) = self.frame.as_ref() {
            let h_wnd = frame.window.0;
            unsafe {
                windows_and_messaging::ShowWindow(h_wnd, SHOW_WINDOW_CMD::SW_SHOW);
                gdi::UpdateWindow(h_wnd);
                keyboard_and_mouse_input::SetFocus(h_wnd);
            }
        }

        let mut msg = MSG::default();
        let h_wnd = HWND::default();

        loop {
            while let Ok(f) = self.rx.try_recv() {
                (f)(self.clone());
            }

            unsafe {
                let result = windows_and_messaging::GetMessageA(&mut msg, h_wnd, 0, 0).0;

                match result {
                    -1 => {
                        break {
                            let error =
                                format!("GetMessageW failed: {}", win32::debug::GetLastError());
                            Err(Error::new(ErrorCode::E_NOINTERFACE, &error))
                        }
                    }
                    0 => break Ok(()),
                    _ => match msg.message {
                        windows_and_messaging::WM_APP => (),
                        _ => {
                            windows_and_messaging::TranslateMessage(&msg);
                            windows_and_messaging::DispatchMessageA(&msg);
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

    pub fn set_title(&self, title: &str) {
        if let Some(frame) = self.frame.as_ref() {
            if let Ok(title) = CString::new(title) {
                unsafe {
                    windows_and_messaging::SetWindowTextA(
                        frame.window.0,
                        PSTR(title.as_ptr() as *mut _),
                    );
                }
            }
        }
    }

    pub fn set_size(&self, width: i32, height: i32, hints: SizeHint) -> Result<()> {
        if let Some(frame) = self.frame.as_ref() {
            match hints {
                SizeHint::MIN => {
                    *frame.min_size.lock().expect("lock min_size") = Some(SIZE {
                        cx: width,
                        cy: height,
                    });
                }
                SizeHint::MAX => {
                    *frame.max_size.lock().expect("lock max_size") = Some(SIZE {
                        cx: width,
                        cy: height,
                    });
                }
                _ => {
                    *frame.size.lock().expect("lock size") = SIZE {
                        cx: width,
                        cy: height,
                    };
                    self.controller.set_bounds(Rect {
                        x: 0f32,
                        y: 0f32,
                        width: width as f32,
                        height: height as f32,
                    })?;

                    unsafe {
                        windows_and_messaging::SetWindowPos(
                            frame.window.0,
                            HWND(0),
                            0,
                            0,
                            width,
                            height,
                            SetWindowPos_uFlags::SWP_NOACTIVATE
                                | SetWindowPos_uFlags::SWP_NOZORDER
                                | SetWindowPos_uFlags::SWP_NOMOVE,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_window(&self) -> Arc<Window> {
        self.parent.clone()
    }

    pub fn navigate(&self, url: &str) {
        let url: Vec<u16> = url.encode_utf16().collect();
        let url = HString::from_wide(&url);
        *self.url.lock().expect("lock url") = url;
    }

    pub fn init(&self, js: &str) -> Result<()> {
        let js: Vec<u16> = js.encode_utf16().collect();
        let js = HString::from_wide(&js);

        let (tx, rx) = mpsc::channel();
        let webview = self.webview.as_ref();
        webview
            .add_script_to_execute_on_document_created_async(js)?
            .set_completed(AsyncOperationCompletedHandler::new(move |op, _status| {
                if let Some(op) = op {
                    tx.send(op.get_results()?).expect("send over mpsc");
                }
                Ok(())
            }))?;

        Webview::run_one(rx)?;
        Ok(())
    }

    pub fn eval(&self, js: &str) -> Result<()> {
        let js: Vec<u16> = js.encode_utf16().collect();
        let js = HString::from_wide(&js);

        let webview = self.webview.as_ref();
        let (tx, rx) = mpsc::channel();
        webview
            .execute_script_async(js)?
            .set_completed(AsyncOperationCompletedHandler::new(move |op, _status| {
                if let Some(op) = op {
                    tx.send(op.get_results()?).expect("send over mpsc");
                }
                Ok(())
            }))?;

        Webview::run_one(rx)?;
        Ok(())
    }

    pub fn dispatch<F>(&self, f: F)
    where
        F: FnOnce(Webview) + Send + 'static,
    {
        self.tx.send(Box::new(f)).expect("send the fn");

        unsafe {
            windows_and_messaging::PostThreadMessageA(
                self.thread_id,
                windows_and_messaging::WM_APP,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }

    pub fn bind<F>(&mut self, name: &str, f: F) -> Result<()>
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

        self.init(&js)
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

            webview.eval(&js).expect("eval return script");
        });
    }
}
