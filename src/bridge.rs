#[cxx::bridge]
pub mod core {
    #[derive(Debug, Default)]
    struct WebView2EnvironmentOptions {
        aditional_browser_arguments: Vec<u16>,
        language: Vec<u16>,
        target_compatible_browser_version: Vec<u16>,
        allow_single_sign_on_using_os_primary_account: bool,
    }

    #[derive(Debug, Default)]
    struct WebView2ControllerBounds {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[derive(Debug)]
    struct WebView2Settings {
        is_script_enabled: bool,
        is_web_message_enabled: bool,
        are_default_script_dialogs_enabled: bool,
        is_status_bar_enabled: bool,
        are_dev_tools_enabled: bool,
        are_default_context_menus_enabled: bool,
        is_zoom_control_enabled: bool,
        is_built_in_error_page_enabled: bool,
    }

    extern "Rust" {
        fn to_utf16(value: &str) -> Vec<u16>;
        fn from_utf16(value: &[u16]) -> String;

        type CreateWebView2EnvironmentCompletedHandler;

        fn invoke_environment_complete(
            handler: Box<CreateWebView2EnvironmentCompletedHandler>,
            environment: SharedPtr<WebView2Environment>,
        );

        type CreateWebView2ControllerCompletedHandler;

        fn invoke_controller_complete(
            handler: Box<CreateWebView2ControllerCompletedHandler>,
            controller: SharedPtr<WebView2Controller>,
        );

        type NavigationCompletedHandler;

        fn invoke_navigation_complete(handler: Box<NavigationCompletedHandler>, webview: &WebView2);

        type AddScriptToExecuteOnDocumentCreatedCompletedHandler;

        fn invoke_add_script_on_document_created_complete(
            handler: Box<AddScriptToExecuteOnDocumentCreatedCompletedHandler>,
            id: Vec<u16>,
        );

        type ExecuteScriptCompletedHandler;

        fn invoke_execute_script_complete(
            handler: Box<ExecuteScriptCompletedHandler>,
            result: Vec<u16>,
        );

        type WebMessageReceivedHandler;

        fn invoke_web_message_received(
            handler: &WebMessageReceivedHandler,
            source: Vec<u16>,
            message: Vec<u16>,
        );
    }

    unsafe extern "C++" {
        include!("webview_official/src/bridge.h");

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
        ) -> Result<&WebView2Environment>;

        type WebView2Controller;

        fn visible(self: &WebView2Controller, value: bool) -> Result<&WebView2Controller>;
        fn get_visible(self: &WebView2Controller) -> Result<bool>;
        fn bounds(
            self: &WebView2Controller,
            value: WebView2ControllerBounds,
        ) -> Result<&WebView2Controller>;
        fn get_bounds(self: &WebView2Controller) -> Result<WebView2ControllerBounds>;
        fn close(self: &WebView2Controller) -> Result<()>;
        fn get_webview(self: &WebView2Controller) -> Result<SharedPtr<WebView2>>;

        type WebView2;

        fn settings(self: &WebView2, value: WebView2Settings) -> Result<&WebView2>;
        fn get_settings(self: &WebView2) -> Result<WebView2Settings>;
        fn navigate(
            self: &WebView2,
            url: &[u16],
            handler: Box<NavigationCompletedHandler>,
        ) -> Result<&WebView2>;
        fn navigate_to_string(
            self: &WebView2,
            html_content: &[u16],
            handler: Box<NavigationCompletedHandler>,
        ) -> Result<&WebView2>;
        fn add_script_to_execute_on_document_created(
            self: &WebView2,
            javascript: &[u16],
            handler: Box<AddScriptToExecuteOnDocumentCreatedCompletedHandler>,
        ) -> Result<&WebView2>;
        fn remove_script_to_execute_on_document_created(
            self: &WebView2,
            id: &[u16],
        ) -> Result<&WebView2>;
        fn execute_script(
            self: &WebView2,
            javascript: &[u16],
            handler: Box<ExecuteScriptCompletedHandler>,
        ) -> Result<&WebView2>;
        fn reload(self: &WebView2) -> Result<&WebView2>;
        fn post_web_message(self: &WebView2, json_message: &[u16]) -> Result<&WebView2>;
        fn add_web_message_received(
            self: &WebView2,
            handler: Box<WebMessageReceivedHandler>,
        ) -> Result<i64>;
        fn remove_web_message_received(self: &WebView2, token: i64) -> Result<&WebView2>;
        fn stop(self: &WebView2) -> Result<&WebView2>;
        fn get_document_title(self: &WebView2) -> Result<Vec<u16>>;
        fn open_dev_tools_window(self: &WebView2) -> Result<&WebView2>;
    }
}

pub fn to_utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

pub fn from_utf16(value: &[u16]) -> String {
    match String::from_utf16(value) {
        Ok(result) => result,
        Err(_) => String::new(),
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
            aditional_browser_arguments: to_utf16(aditional_browser_arguments),
            language: to_utf16(language),
            target_compatible_browser_version: to_utf16(target_compatible_browser_version),
            allow_single_sign_on_using_os_primary_account,
        }
    }
}

type EnvironmentCompletedCallback = Box<dyn FnOnce(cxx::SharedPtr<core::WebView2Environment>)>;

pub struct CreateWebView2EnvironmentCompletedHandler {
    callback: EnvironmentCompletedCallback,
}

impl CreateWebView2EnvironmentCompletedHandler {
    pub fn new(callback: EnvironmentCompletedCallback) -> Self {
        Self { callback }
    }

    pub fn handle(self, environment: cxx::SharedPtr<core::WebView2Environment>) {
        (self.callback)(environment);
    }
}

pub fn invoke_environment_complete(
    handler: Box<CreateWebView2EnvironmentCompletedHandler>,
    environment: cxx::SharedPtr<core::WebView2Environment>,
) {
    handler.handle(environment);
}

type ControllerCompletedCallback = Box<dyn FnOnce(cxx::SharedPtr<core::WebView2Controller>)>;

pub struct CreateWebView2ControllerCompletedHandler {
    callback: ControllerCompletedCallback,
}

impl CreateWebView2ControllerCompletedHandler {
    pub fn new(callback: ControllerCompletedCallback) -> Self {
        Self { callback }
    }

    pub fn handle(self, controller: cxx::SharedPtr<core::WebView2Controller>) {
        (self.callback)(controller);
    }
}

pub fn invoke_controller_complete(
    handler: Box<CreateWebView2ControllerCompletedHandler>,
    controller: cxx::SharedPtr<core::WebView2Controller>,
) {
    handler.handle(controller);
}

type NavigationCompletedCallback = Box<dyn FnOnce(&core::WebView2)>;

pub struct NavigationCompletedHandler {
    callback: NavigationCompletedCallback,
}

impl NavigationCompletedHandler {
    pub fn new(callback: NavigationCompletedCallback) -> Self {
        Self { callback }
    }

    pub fn handle(self, webview: &core::WebView2) {
        (self.callback)(webview);
    }
}

pub fn invoke_navigation_complete(
    handler: Box<NavigationCompletedHandler>,
    webview: &core::WebView2,
) {
    handler.handle(webview)
}

type AddScriptToExecuteOnDocumentCreatedCompletedCallback = Box<dyn FnOnce(String)>;

pub struct AddScriptToExecuteOnDocumentCreatedCompletedHandler {
    callback: ExecuteScriptCompletedCallback,
}

impl AddScriptToExecuteOnDocumentCreatedCompletedHandler {
    pub fn new(callback: AddScriptToExecuteOnDocumentCreatedCompletedCallback) -> Self {
        Self { callback }
    }

    pub fn handle(self, result: String) {
        (self.callback)(result);
    }
}

pub fn invoke_add_script_on_document_created_complete(
    handler: Box<AddScriptToExecuteOnDocumentCreatedCompletedHandler>,
    id: Vec<u16>,
) {
    handler.handle(from_utf16(&id));
}

type ExecuteScriptCompletedCallback = Box<dyn FnOnce(String)>;

pub struct ExecuteScriptCompletedHandler {
    callback: ExecuteScriptCompletedCallback,
}

impl ExecuteScriptCompletedHandler {
    pub fn new(callback: ExecuteScriptCompletedCallback) -> Self {
        Self { callback }
    }

    pub fn handle(self, result: String) {
        (self.callback)(result);
    }
}

pub fn invoke_execute_script_complete(
    handler: Box<ExecuteScriptCompletedHandler>,
    result: Vec<u16>,
) {
    handler.handle(from_utf16(&result));
}

type WebMessageReceivedCallback = Box<dyn Fn(String, String)>;

pub struct WebMessageReceivedHandler {
    callback: WebMessageReceivedCallback,
}

impl WebMessageReceivedHandler {
    pub fn new(callback: WebMessageReceivedCallback) -> Self {
        Self { callback }
    }

    pub fn handle(&self, source: String, message: String) {
        (self.callback)(source, message);
    }
}

pub fn invoke_web_message_received(
    handler: &WebMessageReceivedHandler,
    source: Vec<u16>,
    message: Vec<u16>,
) {
    handler.handle(from_utf16(&source), from_utf16(&message));
}
