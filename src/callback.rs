use std::{
    marker::PhantomData,
    mem,
    sync::atomic::{AtomicU32, Ordering},
};

use windows::{Abi, Interface};

use bindings::Windows::Win32::{Com::HRESULT, SystemServices::PWSTR, WebView2};

unsafe fn from_abi<I: Interface>(this: windows::RawPtr) -> windows::Result<I> {
    let unknown = windows::IUnknown::from_abi(this)?;
    unknown.vtable().1(unknown.abi()); // add_ref to balance the release called in IUnknown::drop
    Ok(unknown.cast()?)
}

pub unsafe fn create<T: Callback>(
    closure: <T as Callback>::Closure,
) -> windows::Result<<T as Callback>::Interface> {
    let handler = Box::new(T::new(closure));
    let handler = from_abi(Box::into_raw(handler) as windows::RawPtr)?;
    Ok(handler)
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

pub trait Callback {
    type Interface: Interface;
    type Closure;

    fn new(closure: Self::Closure) -> Self;
}

pub trait CallbackInterface<T: Callback>: Sized {
    fn refcount(&self) -> &AtomicU32;

    unsafe extern "system" fn query_interface(
        this: windows::RawPtr,
        iid: &windows::Guid,
        interface: *mut windows::RawPtr,
    ) -> windows::ErrorCode {
        if interface.is_null() {
            windows::ErrorCode::E_POINTER
        } else if *iid == windows::IUnknown::IID
            || *iid == <<T as Callback>::Interface as Interface>::IID
        {
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

pub trait ClosureArg {
    type Input;
    type Output;

    fn convert(input: Self::Input) -> Self::Output;
}

pub struct ErrorCodeArg;

impl ClosureArg for ErrorCodeArg {
    type Input = HRESULT;
    type Output = windows::ErrorCode;

    fn convert(input: HRESULT) -> windows::ErrorCode {
        windows::ErrorCode(input.0 as u32)
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

pub struct StringArg;

impl ClosureArg for StringArg {
    type Input = PWSTR;
    type Output = String;

    fn convert(input: PWSTR) -> String {
        string_from_pwstr(input)
    }
}

type CompletedClosure<Arg1, Arg2> = Box<dyn FnOnce(Arg1, Arg2) -> windows::ErrorCode>;

pub trait CompletedCallback<T: Callback, Arg1: ClosureArg, Arg2: ClosureArg>:
    CallbackInterface<T>
{
    fn completed(&mut self) -> Option<CompletedClosure<Arg1::Output, Arg2::Output>>;

    unsafe extern "system" fn invoke(
        this: windows::RawPtr,
        arg_1: Arg1::Input,
        arg_2: Arg2::Input,
    ) -> HRESULT {
        let interface: *mut Self = mem::transmute(this);
        match (*interface).completed() {
            Some(completed) => {
                HRESULT(completed(Arg1::convert(arg_1), Arg2::convert(arg_2)).0 as i32)
            }
            None => HRESULT(windows::ErrorCode::S_OK.0 as i32),
        }
    }
}

type EventClosure<Arg1, Arg2> = Box<dyn FnMut(Arg1, Arg2) -> windows::ErrorCode>;

pub trait EventCallback<T: Callback, Arg1: ClosureArg, Arg2: ClosureArg>:
    CallbackInterface<T>
{
    fn event(&mut self) -> &mut EventClosure<Arg1::Output, Arg2::Output>;

    unsafe extern "system" fn invoke(
        this: windows::RawPtr,
        arg_1: Arg1::Input,
        arg_2: Arg2::Input,
    ) -> HRESULT {
        let interface: *mut Self = mem::transmute(this);
        HRESULT(((*interface).event())(Arg1::convert(arg_1), Arg2::convert(arg_2)).0 as i32)
    }
}

#[completed_callback]
pub struct CreateCoreWebView2EnvironmentCompletedHandler(
    WebView2::ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler,
    ErrorCodeArg,
    InterfaceArg<WebView2::ICoreWebView2Environment>,
);

#[completed_callback]
pub struct CreateCoreWebView2ControllerCompletedHandler(
    WebView2::ICoreWebView2CreateCoreWebView2ControllerCompletedHandler,
    ErrorCodeArg,
    InterfaceArg<WebView2::ICoreWebView2Controller>,
);

#[event_callback]
pub struct WebMessageReceivedEventHandler(
    WebView2::ICoreWebView2WebMessageReceivedEventHandler,
    InterfaceArg<WebView2::ICoreWebView2>,
    InterfaceArg<WebView2::ICoreWebView2WebMessageReceivedEventArgs>,
);

#[event_callback]
pub struct NavigationCompletedEventHandler(
    WebView2::ICoreWebView2NavigationCompletedEventHandler,
    InterfaceArg<WebView2::ICoreWebView2>,
    InterfaceArg<WebView2::ICoreWebView2NavigationCompletedEventArgs>,
);

#[completed_callback]
pub struct AddScriptToExecuteOnDocumentCreatedCompletedHandler(
    WebView2::ICoreWebView2AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    ErrorCodeArg,
    StringArg,
);

#[completed_callback]
pub struct ExecuteScriptCompletedHandler(
    WebView2::ICoreWebView2ExecuteScriptCompletedHandler,
    ErrorCodeArg,
    StringArg,
);
