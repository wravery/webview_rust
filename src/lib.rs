mod builder;
mod webview;

pub use builder::WebviewBuilder;
pub use webview::{SizeHint, Webview, Window};

use bindings::windows::win32::{system_services::*, windows_and_messaging::*};

#[cxx::bridge]
pub mod core {
    #[derive(Debug)]
    struct WebView2EnvironmentOptions {
        aditional_browser_arguments: Vec<u16>,
        language: Vec<u16>,
        target_compatible_browser_version: Vec<u16>,
        allow_single_sign_on_using_os_primary_account: bool,
    }

    extern "Rust" {
        type CreateWebView2EnvironmentCompletedHandler;

        fn invoke_environment_complete(
            handler: &CreateWebView2EnvironmentCompletedHandler,
            environment: UniquePtr<WebView2Environment>,
        );

        type CreateWebView2ControllerCompletedHandler;

        fn invoke_controller_complete(
            handler: &CreateWebView2ControllerCompletedHandler,
            environment: UniquePtr<WebView2Controller>,
        );
    }

    unsafe extern "C++" {
        include!("webview_official/include/webview2-rs.h");

        fn new_webview2_environment(
            handler: Box<CreateWebView2EnvironmentCompletedHandler>,
        ) -> Result<()>;

        fn new_webview2_environment_with_options(
            browser_executable_folder: &[u16],
            user_data_folder: &[u16],
            options: &WebView2EnvironmentOptions,
            handler: Box<CreateWebView2EnvironmentCompletedHandler>,
        ) -> Result<()>;

        fn get_available_webview2_browser_version_string(
            browser_executable_folder: &[u16],
        ) -> Result<Vec<u16>>;

        fn compare_browser_versions(version1: &[u16], version2: &[u16]) -> Result<i8>;

        type WebView2Environment;

        fn create_webview2_controller(
            self: &WebView2Environment,
            parent_window: isize,
            handler: Box<CreateWebView2ControllerCompletedHandler>,
        ) -> Result<()>;

        type WebView2Controller;

        // fn new_blob_store_client() -> UniquePtr<BlobStoreClient>;
        // fn put(&self, parts: &mut MultiBuf) -> usize;
        // fn tag(&self, blob_id: usize, tag: &str);
        // fn metadata(&self, blob_id: usize) -> BlobMetadata;

        type WebView2;

        // fn new_blob_store_client() -> UniquePtr<BlobStoreClient>;
        // fn put(&self, parts: &mut MultiBuf) -> usize;
        // fn tag(&self, blob_id: usize, tag: &str);
        // fn metadata(&self, blob_id: usize) -> BlobMetadata;
    }
}

impl core::WebView2EnvironmentOptions {
    pub fn new(
        aditional_browser_arguments: &str,
        language: &str,
        target_compatible_browser_version: &str,
        allow_single_sign_on_using_os_primary_account: bool,
    ) -> core::WebView2EnvironmentOptions {
        core::WebView2EnvironmentOptions {
            aditional_browser_arguments: aditional_browser_arguments.encode_utf16().collect(),
            language: language.encode_utf16().collect(),
            target_compatible_browser_version: target_compatible_browser_version
                .encode_utf16()
                .collect(),
            allow_single_sign_on_using_os_primary_account,
        }
    }
}

pub struct CreateWebView2EnvironmentCompletedHandler {
    pub callback: Box<dyn Fn(cxx::UniquePtr<core::WebView2Environment>)>,
}

pub fn invoke_environment_complete(
    handler: &CreateWebView2EnvironmentCompletedHandler,
    environment: cxx::UniquePtr<core::WebView2Environment>,
) {
    (handler.callback)(environment);
}

pub struct CreateWebView2ControllerCompletedHandler {
    pub callback: Box<dyn Fn(cxx::UniquePtr<core::WebView2Controller>)>,
}

pub fn invoke_controller_complete(
    handler: &CreateWebView2ControllerCompletedHandler,
    controller: cxx::UniquePtr<core::WebView2Controller>,
) {
    (handler.callback)(controller);
}
