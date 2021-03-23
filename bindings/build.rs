fn main() {
    windows::build!(windows::win32::com::{CoInitialize, CoUninitialize},
        windows::win32::debug::GetLastError,
        windows::win32::hi_dpi::SetProcessDpiAwarenessContext,
        windows::win32::menus_and_resources::HMENU,
        windows::win32::system_services::{DPI_AWARENESS_CONTEXT, GetModuleHandleW, HINSTANCE, LRESULT, PWSTR},
        windows::win32::windows_and_messaging::*);
}
