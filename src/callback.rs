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

#[completed_callback(
    interface = "WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler",
    arg_1 = "ErrorCodeArg",
    arg_2 = "InterfaceArg<WebView2::ICoreWebView2Environment>"
)]
pub struct CreateCoreWebView2EnvironmentCompletedHandler;

#[completed_callback(
    interface = "WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler",
    arg_1 = "ErrorCodeArg",
    arg_2 = "InterfaceArg<WebView2::ICoreWebView2Controller>"
)]
pub struct CreateCoreWebView2ControllerCompletedHandler;

type EventClosure<Arg1, Arg2> = Box<dyn FnMut(Arg1, Arg2) -> windows::ErrorCode>;

pub trait EventCallback<I: Interface, Arg1: ClosureArg, Arg2: ClosureArg>:
    CallbackInterface<I>
{
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

#[event_callback(
    interface = "WebView2::ICoreWebView2WebMessageReceivedEventHandler",
    arg_1 = "InterfaceArg<WebView2::ICoreWebView2>",
    arg_2 = "InterfaceArg<WebView2::ICoreWebView2WebMessageReceivedEventArgs>"
)]
pub struct WebMessageReceivedEventHandler;

#[event_callback(
    interface = "WebView2::ICoreWebView2NavigationCompletedEventHandler",
    arg_1 = "InterfaceArg<WebView2::ICoreWebView2>",
    arg_2 = "InterfaceArg<WebView2::ICoreWebView2NavigationCompletedEventArgs>"
)]
pub struct NavigationCompletedEventHandler;

pub struct StringArg();

impl ClosureArg for StringArg {
    type Input = PWSTR;
    type Output = String;

    fn convert(input: PWSTR) -> String {
        string_from_pwstr(input)
    }
}

#[completed_callback(
    interface = "WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler",
    arg_1 = "ErrorCodeArg",
    arg_2 = "StringArg"
)]
pub struct AddScriptToExecuteOnDocumentCreatedCompletedHandler;

#[completed_callback(
    interface = "WebView2::ICoreWebView2ExecuteScriptCompletedHandler",
    arg_1 = "ErrorCodeArg",
    arg_2 = "StringArg"
)]
pub struct ExecuteScriptCompletedHandler;
