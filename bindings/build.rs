fn main() {
    windows::build!(Windows::Win32::Com::{CoTaskMemFree},
        Windows::Win32::Debug::GetLastError,
        Windows::Win32::DisplayDevices::RECT,
        Windows::Win32::Gdi::UpdateWindow,
        Windows::Win32::HiDpi::{PROCESS_DPI_AWARENESS, SetProcessDpiAwareness},
        Windows::Win32::KeyboardAndMouseInput::SetFocus,
        Windows::Win32::MenusAndResources::HMENU,
        Windows::Win32::SystemServices::{GetCurrentThreadId, GetModuleHandleA, HINSTANCE, LRESULT, PWSTR},
        Windows::Win32::WebView2::*,
        Windows::Win32::WinRT::EventRegistrationToken,
        Windows::Win32::WindowsAndMessaging::*);
}
