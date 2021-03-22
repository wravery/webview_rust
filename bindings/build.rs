fn main() {
    windows::build!(windows::win32::system_services::*, windows::win32::windows_and_messaging::*);
}
