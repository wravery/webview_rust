fn main() {
    windows::build!(windows::win32::com::{CoInitialize, CoUninitialize},
        windows::win32::debug::GetLastError,
        windows::win32::display_devices::RECT,
        windows::win32::gdi::UpdateWindow,
        windows::win32::hi_dpi::SetProcessDpiAwarenessContext,
        windows::win32::keyboard_and_mouse_input::SetFocus,
        windows::win32::menus_and_resources::HMENU,
        windows::win32::system_services::{DPI_AWARENESS_CONTEXT, GetCurrentThreadId, GetModuleHandleW, HINSTANCE, LRESULT, PWSTR},
        windows::win32::windows_and_messaging::*);
}
