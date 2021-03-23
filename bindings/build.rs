fn main() {
    windows::build!(windows::win32::com::{CoInitialize, CoUninitialize},
        windows::win32::debug::GetLastError,
        windows::win32::windows_and_messaging::*);
}
