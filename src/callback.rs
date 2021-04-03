#[macro_use]
extern crate callback_derive;

use std::{
    marker::PhantomData,
    mem,
    sync::atomic::{AtomicU32, Ordering},
};

use windows::{Abi, Interface};

use bindings::Windows::Win32::{SystemServices::PWSTR, WebView2};

pub unsafe fn from_abi<I: Interface>(this: windows::RawPtr) -> windows::Result<I> {
    let unknown = windows::IUnknown::from_abi(this)?;
    unknown.vtable().1(unknown.abi()); // add_ref to balance the release called in IUnknown::drop
    Ok(unknown.cast()?)
}

pub fn string_from_pwstr(source: PWSTR) -> String {
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

pub trait CallbackInterface<I: Interface>: Sized {
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

pub trait ClosureArg {
    type Input;
    type Output;

    fn convert(input: Self::Input) -> Self::Output;
}

pub trait CompletedCallback<I: Interface, Arg1: ClosureArg, Arg2: ClosureArg>:
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

// type CreateCoreWebView2EnvironmentCompletedCallback =
//     CompletedClosure<windows::ErrorCode, Option<WebView2::ICoreWebView2Environment>>;

#[repr(C)]
#[derive(CompletedCallback)]
#[interface = "WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_abi"]
#[arg_1 = "ErrorCodeArg"]
#[arg_2 = "InterfaceArg<WebView2::ICoreWebView2Environment>"]
pub struct CreateCoreWebView2EnvironmentCompletedHandler {
    vtable: *const WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_abi,
    refcount: AtomicU32,
    completed: Option<CreateCoreWebView2EnvironmentCompletedHandlerClosure>,
}

// impl CreateCoreWebView2EnvironmentCompletedHandler {
//     pub fn new(completed: CreateCoreWebView2EnvironmentCompletedCallback) -> Self {
//         static VTABLE: WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_abi =
//             WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler_abi(
//                 CreateCoreWebView2EnvironmentCompletedHandler::query_interface,
//                 CreateCoreWebView2EnvironmentCompletedHandler::add_ref,
//                 CreateCoreWebView2EnvironmentCompletedHandler::release,
//                 CreateCoreWebView2EnvironmentCompletedHandler::invoke,
//             );

//         Self {
//             vtable: &VTABLE,
//             refcount: AtomicU32::new(1),
//             completed: Some(completed),
//         }
//     }
// }

pub struct ErrorCodeArg();

impl ClosureArg for ErrorCodeArg {
    type Input = windows::ErrorCode;
    type Output = windows::ErrorCode;

    fn convert(input: windows::ErrorCode) -> windows::ErrorCode {
        input
    }
}

pub struct InterfaceArg<I: Interface>(PhantomData<I>);

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

// impl CallbackInterface<WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>
//     for CreateCoreWebView2EnvironmentCompletedHandler
// {
//     fn refcount(&self) -> &AtomicU32 {
//         &self.refcount
//     }
// }

// impl
//     CompletedCallback<
//         WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler,
//         ErrorCodeArg,
//         InterfaceArg<WebView2::ICoreWebView2Environment>,
//     > for CreateCoreWebView2EnvironmentCompletedHandler
// {
//     fn completed(&mut self) -> Option<CreateCoreWebView2EnvironmentCompletedCallback> {
//         self.completed.take()
//     }
// }

type CreateCoreWebView2ControllerCompletedCallback =
    CompletedClosure<windows::ErrorCode, Option<WebView2::ICoreWebView2Controller>>;

#[repr(C)]
pub struct CreateCoreWebView2ControllerCompletedHandler {
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

pub trait EventCallback<I: Interface, Arg1: ClosureArg, Arg2: ClosureArg>: CallbackInterface<I> {
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
pub struct WebMessageReceivedEventHandler {
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
pub struct NavigationCompletedEventHandler {
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

pub struct StringArg();

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
pub struct AddScriptToExecuteOnDocumentCreatedCompletedHandler {
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
pub struct ExecuteScriptCompletedHandler {
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
