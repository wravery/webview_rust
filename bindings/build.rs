fn main() {
    windows::build!(microsoft::web::web_view2::core::*,
        windows::foundation::*,
        windows::win32::com::{CoInitialize, CoUninitialize},
        windows::win32::debug::GetLastError,
        windows::win32::display_devices::{POINT, RECT, SIZE},
        windows::win32::gdi::UpdateWindow,
        windows::win32::hi_dpi::{PROCESS_DPI_AWARENESS, SetProcessDpiAwareness},
        windows::win32::keyboard_and_mouse_input::SetFocus,
        windows::win32::menus_and_resources::HMENU,
        windows::win32::system_services::{GetCurrentThreadId, GetModuleHandleA, HINSTANCE, LRESULT, PWSTR},
        windows::win32::windows_and_messaging::*);
}
