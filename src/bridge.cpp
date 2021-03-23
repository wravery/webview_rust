#include "webview_official/src/bridge.h"
#include "webview_official/src/bridge.rs.h"

#include <WebView2.h>
#include <WebView2EnvironmentOptions.h>

#include <windows.h>
#include <wrl.h>

#include <algorithm>
#include <iomanip>
#include <iterator>
#include <optional>
#include <stdexcept>
#include <sstream>
#include <string>
#include <string_view>

using namespace std::literals;

using SPWSTR = std::unique_ptr<wchar_t[], decltype(&::CoTaskMemFree)>;

std::wstring to_wstring(const rust::Vec<uint16_t> source) noexcept
{
    return {reinterpret_cast<const wchar_t *>(source.data()), source.size()};
}

std::wstring to_wstring(rust::Slice<const uint16_t> source) noexcept
{
    return {reinterpret_cast<const wchar_t *>(source.data()), source.size()};
}

rust::Vec<uint16_t> to_vec(std::wstring_view source) noexcept
{
    rust::Vec<uint16_t> result;

    result.reserve(source.size());
    std::transform(source.begin(), source.end(), std::back_inserter(result), [](wchar_t ch) {
        return static_cast<uint16_t>(ch);
    });

    return result;
}

void throw_failed(std::string_view expression, HRESULT hr)
{
    std::ostringstream oss;

    oss << expression << " failed: 0x"
        << std::hex << std::setw(8) << std::setfill('0') << static_cast<uint32_t>(hr);

    throw std::runtime_error(oss.str());
}

#define CHECK_HR(expression)                   \
    {                                          \
        const HRESULT hr = (expression);       \
        if (FAILED(hr))                        \
        {                                      \
            throw_failed(#expression##sv, hr); \
        }                                      \
    }

class WebView2Environment::impl
{
public:
    impl(Microsoft::WRL::ComPtr<ICoreWebView2Environment> environment);

    void CreateWebView2Controller(ptrdiff_t parent_window,
                                  rust::Box<CreateWebView2ControllerCompletedHandler> handler,
                                  std::shared_ptr<const WebView2Environment> instance) const;

private:
    void CheckCreated() const;

    Microsoft::WRL::ComPtr<ICoreWebView2Environment> m_environment;
};

class WebView2Controller::impl
{
public:
    impl(Microsoft::WRL::ComPtr<ICoreWebView2Controller> controller, std::shared_ptr<const WebView2Environment> environment);

    bool get_IsVisible() const;
    void put_IsVisible(bool value) const;
    WebView2ControllerBounds get_Bounds() const;
    void put_Bounds(WebView2ControllerBounds value) const;
    void Close();
    std::shared_ptr<WebView2> get_WebView(std::shared_ptr<const WebView2Controller> instance);

private:
    void CheckCreated() const;

    Microsoft::WRL::ComPtr<ICoreWebView2Controller> m_controller;
    std::shared_ptr<const WebView2Environment> m_environment;
    std::weak_ptr<WebView2> m_webview;
};

class WebView2::impl
{
public:
    impl(Microsoft::WRL::ComPtr<ICoreWebView2> webview, std::shared_ptr<const WebView2Controller> controller);
    ~impl();

    WebView2Settings get_Settings() const;
    void put_Settings(WebView2Settings value) const;
    void Navigate(rust::Slice<const uint16_t> url, rust::Box<NavigationCompletedHandler> handler, std::shared_ptr<const WebView2> instance);
    void NavigateToString(rust::Slice<const uint16_t> html_content, rust::Box<NavigationCompletedHandler> handler, std::shared_ptr<const WebView2> instance);
    void ExecuteScript(rust::Slice<const uint16_t> javascript, rust::Box<ExecuteScriptCompletedHandler> handler) const;
    void Reload() const;
    void PostWebMessage(rust::Slice<const uint16_t> json_message) const;
    void Stop() const;
    rust::Vec<uint16_t> get_DocumentTitle() const;
    void OpenDevToolsWindow() const;

private:
    void CheckCreated() const;
    void AddEventHandlers();
    void RemoveEventHandlers();

    Microsoft::WRL::ComPtr<ICoreWebView2> m_webview;
    std::shared_ptr<const WebView2Controller> m_controller;
    std::optional<rust::Box<NavigationCompletedHandler>> m_navigationCompletedHandler;
    std::weak_ptr<const WebView2> m_navigationCompletedInstance;
    EventRegistrationToken m_navigationCompletedToken;
};

WebView2Environment::impl::impl(Microsoft::WRL::ComPtr<ICoreWebView2Environment> environment)
    : m_environment{std::move(environment)}
{
    CheckCreated();
}

void WebView2Environment::impl::CreateWebView2Controller(ptrdiff_t parent_window,
                                                         rust::Box<CreateWebView2ControllerCompletedHandler> handler,
                                                         std::shared_ptr<const WebView2Environment> instance) const
{
    CheckCreated();

    auto callback = Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>(
        [handler = std::move(handler), instance = std::move(instance)](HRESULT hr, ICoreWebView2Controller *controller) mutable noexcept {
            std::shared_ptr<WebView2Controller> result;

            if (SUCCEEDED(hr) && nullptr != controller)
            {
                result = std::make_shared<WebView2Controller>(std::make_unique<WebView2Controller::impl>(std::move(controller), std::move(instance)));
            }

            invoke_controller_complete(std::move(handler), std::move(result));
            return S_OK;
        });

    CHECK_HR(m_environment->CreateCoreWebView2Controller(reinterpret_cast<HWND>(parent_window), callback.Get()));
}

void WebView2Environment::impl::CheckCreated() const
{
    if (!m_environment)
    {
        throw std::runtime_error("ICoreWebView2Environment creation failed");
    }
}

WebView2Controller::impl::impl(Microsoft::WRL::ComPtr<ICoreWebView2Controller> controller, std::shared_ptr<const WebView2Environment> environment)
    : m_controller{std::move(controller)}, m_environment{std::move(environment)}
{
    CheckCreated();
}

bool WebView2Controller::impl::get_IsVisible() const
{
    CheckCreated();

    BOOL result = false;

    CHECK_HR(m_controller->get_IsVisible(&result));

    return static_cast<bool>(result);
}

void WebView2Controller::impl::put_IsVisible(bool value) const
{
    CheckCreated();

    CHECK_HR(m_controller->put_IsVisible(static_cast<BOOL>(value)));
}

WebView2ControllerBounds WebView2Controller::impl::get_Bounds() const
{
    CheckCreated();

    RECT value{};

    CHECK_HR(m_controller->get_Bounds(&value));

    WebView2ControllerBounds result{};

    result.left = value.left;
    result.top = value.top;
    result.right = value.right;
    result.bottom = value.bottom;

    return result;
}

void WebView2Controller::impl::put_Bounds(WebView2ControllerBounds value) const
{
    CheckCreated();

    RECT result{};

    result.left = value.left;
    result.top = value.top;
    result.right = value.right;
    result.bottom = value.bottom;

    CHECK_HR(m_controller->put_Bounds(result));
}

void WebView2Controller::impl::Close()
{
    CheckCreated();

    CHECK_HR(m_controller->Close());

    m_controller = nullptr;
}

std::shared_ptr<WebView2> WebView2Controller::impl::get_WebView(std::shared_ptr<const WebView2Controller> instance)
{
    CheckCreated();

    auto result = m_webview.lock();

    if (!result)
    {
        Microsoft::WRL::ComPtr<ICoreWebView2> webview;

        CHECK_HR(m_controller->get_CoreWebView2(&webview));

        result = std::make_shared<WebView2>(std::make_unique<WebView2::impl>(std::move(webview), std::move(instance)));
        m_webview = result;
    }

    return result;
}

void WebView2Controller::impl::CheckCreated() const
{
    if (!m_controller)
    {
        throw std::runtime_error("ICoreWebView2Controller creation failed");
    }
}

WebView2::impl::impl(Microsoft::WRL::ComPtr<ICoreWebView2> webview, std::shared_ptr<const WebView2Controller> controller)
    : m_webview{std::move(webview)}
    , m_controller{std::move(controller)}
{
    AddEventHandlers();
}

WebView2::impl::~impl()
{
    RemoveEventHandlers();
}

WebView2Settings WebView2::impl::get_Settings() const
{
    CheckCreated();

    Microsoft::WRL::ComPtr<ICoreWebView2Settings> settings;

    CHECK_HR(m_webview->get_Settings(&settings));

    BOOL isScriptEnabled = false;
    BOOL isWebMessageEnabled = false;
    BOOL areDefaultScriptDialogsEnabled = false;
    BOOL isStatusBarEnabled = false;
    BOOL areDevToolsEnabled = false;
    BOOL areDefaultContextMenusEnabled = false;
    BOOL isZoomControlEnabled = false;
    BOOL isBuiltInErrorPageEnabled = false;

    CHECK_HR(settings->get_IsScriptEnabled(&isScriptEnabled));
    CHECK_HR(settings->get_IsWebMessageEnabled(&isWebMessageEnabled));
    CHECK_HR(settings->get_AreDefaultScriptDialogsEnabled(&areDefaultScriptDialogsEnabled));
    CHECK_HR(settings->get_IsStatusBarEnabled(&isStatusBarEnabled));
    CHECK_HR(settings->get_AreDevToolsEnabled(&areDevToolsEnabled));
    CHECK_HR(settings->get_AreDefaultContextMenusEnabled(&areDefaultContextMenusEnabled));
    CHECK_HR(settings->get_IsZoomControlEnabled(&isZoomControlEnabled));
    CHECK_HR(settings->get_IsBuiltInErrorPageEnabled(&isBuiltInErrorPageEnabled));

    WebView2Settings results{};

    results.is_script_enabled = static_cast<bool>(isScriptEnabled);
    results.is_web_message_enabled = static_cast<bool>(isWebMessageEnabled);
    results.are_default_script_dialogs_enabled = static_cast<bool>(areDefaultScriptDialogsEnabled);
    results.is_status_bar_enabled = static_cast<bool>(isStatusBarEnabled);
    results.are_dev_tools_enabled = static_cast<bool>(areDevToolsEnabled);
    results.are_default_context_menus_enabled = static_cast<bool>(areDefaultContextMenusEnabled);
    results.is_zoom_control_enabled = static_cast<bool>(isZoomControlEnabled);
    results.is_built_in_error_page_enabled = static_cast<bool>(isBuiltInErrorPageEnabled);

    return results;
}

void WebView2::impl::put_Settings(WebView2Settings value) const
{
    CheckCreated();

    Microsoft::WRL::ComPtr<ICoreWebView2Settings> settings;

    CHECK_HR(m_webview->get_Settings(&settings));
    CHECK_HR(settings->put_IsScriptEnabled(static_cast<BOOL>(value.is_script_enabled)));
    CHECK_HR(settings->put_IsWebMessageEnabled(static_cast<BOOL>(value.is_web_message_enabled)));
    CHECK_HR(settings->put_AreDefaultScriptDialogsEnabled(static_cast<BOOL>(value.are_default_script_dialogs_enabled)));
    CHECK_HR(settings->put_IsStatusBarEnabled(static_cast<BOOL>(value.is_status_bar_enabled)));
    CHECK_HR(settings->put_AreDevToolsEnabled(static_cast<BOOL>(value.are_dev_tools_enabled)));
    CHECK_HR(settings->put_AreDefaultContextMenusEnabled(static_cast<BOOL>(value.are_default_context_menus_enabled)));
    CHECK_HR(settings->put_IsZoomControlEnabled(static_cast<BOOL>(value.is_zoom_control_enabled)));
    CHECK_HR(settings->put_IsBuiltInErrorPageEnabled(static_cast<BOOL>(value.is_built_in_error_page_enabled)));
}

void WebView2::impl::Navigate(rust::Slice<const uint16_t> url, rust::Box<NavigationCompletedHandler> handler, std::shared_ptr<const WebView2> instance)
{
    CheckCreated();

    m_navigationCompletedHandler = std::make_optional<rust::Box<NavigationCompletedHandler>>(std::move(handler));
    m_navigationCompletedInstance = instance;

    CHECK_HR(m_webview->Navigate(to_wstring(url).c_str()));
}

void WebView2::impl::NavigateToString(rust::Slice<const uint16_t> html_content, rust::Box<NavigationCompletedHandler> handler, std::shared_ptr<const WebView2> instance)
{
    CheckCreated();

    m_navigationCompletedHandler = std::make_optional<rust::Box<NavigationCompletedHandler>>(std::move(handler));
    m_navigationCompletedInstance = instance;

    CHECK_HR(m_webview->NavigateToString(to_wstring(html_content).c_str()));
}

void WebView2::impl::ExecuteScript(rust::Slice<const uint16_t> javascript, rust::Box<ExecuteScriptCompletedHandler> handler) const
{
    CheckCreated();

    auto callback = Microsoft::WRL::Callback<ICoreWebView2ExecuteScriptCompletedHandler>(
        [handler = std::move(handler)](HRESULT hr, PCWSTR resultObjectAsJson) mutable noexcept {
            rust::Vec<uint16_t> result;

            if (SUCCEEDED(hr) && nullptr != resultObjectAsJson)
            {
                result = to_vec(resultObjectAsJson);
            }

            invoke_script_complete(std::move(handler), std::move(result));
            return S_OK;
        });

    CHECK_HR(m_webview->ExecuteScript(to_wstring(javascript).c_str(), callback.Get()));
}

void WebView2::impl::Reload() const
{
    CheckCreated();

    CHECK_HR(m_webview->Reload());
}

void WebView2::impl::PostWebMessage(rust::Slice<const uint16_t> json_message) const
{
    CheckCreated();

    CHECK_HR(m_webview->PostWebMessageAsJson(to_wstring(json_message).c_str()));
}

void WebView2::impl::Stop() const
{
    CheckCreated();

    CHECK_HR(m_webview->Stop());
}

rust::Vec<uint16_t> WebView2::impl::get_DocumentTitle() const
{
    CheckCreated();

    PWSTR documentTitle = nullptr;

    CHECK_HR(m_webview->get_DocumentTitle(&documentTitle));

    SPWSTR cleanup{documentTitle, ::CoTaskMemFree};

    return to_vec(documentTitle);
}

void WebView2::impl::OpenDevToolsWindow() const
{
    CheckCreated();

    CHECK_HR(m_webview->OpenDevToolsWindow());
}

void WebView2::impl::CheckCreated() const
{
    if (!m_webview)
    {
        throw std::runtime_error("ICoreWebView2 creation failed");
    }
}

void WebView2::impl::AddEventHandlers()
{
    CheckCreated();

    auto callback = Microsoft::WRL::Callback<ICoreWebView2NavigationCompletedEventHandler>(
        [this](ICoreWebView2 *, ICoreWebView2NavigationCompletedEventArgs *) mutable noexcept {
            if (m_navigationCompletedHandler)
            {
                if (auto instance = m_navigationCompletedInstance.lock())
                {
                    invoke_navigation_complete(std::move(*m_navigationCompletedHandler), *instance);
                }
            }

            m_navigationCompletedHandler = std::nullopt;
            m_navigationCompletedInstance.reset();
            return S_OK;
        });

    CHECK_HR(m_webview->add_NavigationCompleted(callback.Get(), &m_navigationCompletedToken));
}

void WebView2::impl::RemoveEventHandlers()
{
    if (!m_webview)
    {
        return;
    }
}

void new_webview2_environment(rust::Box<CreateWebView2EnvironmentCompletedHandler> handler)
{
    auto callback = Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
        [handler = std::move(handler)](HRESULT hr, ICoreWebView2Environment *environment) mutable noexcept {
            std::shared_ptr<WebView2Environment> result;

            if (SUCCEEDED(hr) && nullptr != environment)
            {
                result = std::make_shared<WebView2Environment>(std::make_unique<WebView2Environment::impl>(std::move(environment)));
            }

            invoke_environment_complete(std::move(handler), std::move(result));
            return S_OK;
        });

    CHECK_HR(CreateCoreWebView2Environment(callback.Get()));
}

void new_webview2_environment_with_options(rust::Slice<const uint16_t> browser_executable_folder,
                                           rust::Slice<const uint16_t> user_data_folder,
                                           const WebView2EnvironmentOptions &options,
                                           rust::Box<CreateWebView2EnvironmentCompletedHandler> handler)
{
    auto spOptions = Microsoft::WRL::Make<CoreWebView2EnvironmentOptions>();

    if (!options.aditional_browser_arguments.empty())
    {
        auto additionalBrowserArguments = to_wstring(options.aditional_browser_arguments);

        CHECK_HR(spOptions->put_AdditionalBrowserArguments(additionalBrowserArguments.c_str()));
    }

    if (!options.language.empty())
    {
        auto language = to_wstring(options.language);

        CHECK_HR(spOptions->put_Language(language.c_str()));
    }

    if (!options.target_compatible_browser_version.empty())
    {
        auto targetCompatibleBrowserVersion = to_wstring(options.target_compatible_browser_version);

        CHECK_HR(spOptions->put_TargetCompatibleBrowserVersion(targetCompatibleBrowserVersion.c_str()));
    }

    CHECK_HR(spOptions->put_AllowSingleSignOnUsingOSPrimaryAccount(options.allow_single_sign_on_using_os_primary_account));

    auto callback = Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
        [handler = std::move(handler)](HRESULT hr, ICoreWebView2Environment *environment) mutable noexcept {
            std::shared_ptr<WebView2Environment> result;

            if (SUCCEEDED(hr) && nullptr != environment)
            {
                result = std::make_shared<WebView2Environment>(std::make_unique<WebView2Environment::impl>(std::move(environment)));
            }

            invoke_environment_complete(std::move(handler), std::move(result));
            return S_OK;
        });

    CHECK_HR(CreateCoreWebView2EnvironmentWithOptions(to_wstring(browser_executable_folder).c_str(),
                                                      to_wstring(user_data_folder).c_str(),
                                                      spOptions.Get(),
                                                      callback.Get()));
}

rust::Vec<uint16_t> get_available_webview2_browser_version_string(rust::Slice<const uint16_t> browser_executable_folder)
{
    PWSTR version = nullptr;

    CHECK_HR(GetAvailableCoreWebView2BrowserVersionString(to_wstring(browser_executable_folder).c_str(), &version));

    SPWSTR cleanup{version, ::CoTaskMemFree};

    return to_vec(version);
}

int8_t compare_browser_versions(rust::Slice<const uint16_t> version1, rust::Slice<const uint16_t> version2)
{
    int result = 0;

    CHECK_HR(CompareBrowserVersions(to_wstring(version1).c_str(), to_wstring(version2).c_str(), &result));

    if (result > 0)
    {
        return 1;
    }
    else if (result < 0)
    {
        return -1;
    }

    return 0;
}

WebView2Environment::WebView2Environment(std::unique_ptr<impl> &&pimpl)
    : m_pimpl{std::move(pimpl)}
{
}

WebView2Environment::~WebView2Environment()
{
}

const WebView2Environment &WebView2Environment::create_webview2_controller(ptrdiff_t parent_window, rust::Box<CreateWebView2ControllerCompletedHandler> handler) const
{
    m_pimpl->CreateWebView2Controller(parent_window, std::move(handler), shared_from_this());
    return *this;
}

WebView2Controller::WebView2Controller(std::unique_ptr<impl> &&pimpl)
    : m_pimpl{std::move(pimpl)}
{
}

WebView2Controller::~WebView2Controller()
{
}

const WebView2Controller &WebView2Controller::visible(bool value) const
{
    m_pimpl->put_IsVisible(value);
    return *this;
}

bool WebView2Controller::get_visible() const
{
    return m_pimpl->get_IsVisible();
}

const WebView2Controller &WebView2Controller::bounds(WebView2ControllerBounds value) const
{
    m_pimpl->put_Bounds(value);
    return *this;
}

WebView2ControllerBounds WebView2Controller::get_bounds() const
{
    return m_pimpl->get_Bounds();
}

void WebView2Controller::close() const
{
    m_pimpl->Close();
}

std::shared_ptr<WebView2> WebView2Controller::get_webview() const
{
    return m_pimpl->get_WebView(shared_from_this());
}

WebView2::WebView2(std::unique_ptr<impl> &&pimpl)
    : m_pimpl{std::move(pimpl)}
{
}

WebView2::~WebView2()
{
}

const WebView2 &WebView2::settings(WebView2Settings value) const
{
    m_pimpl->put_Settings(value);
    return *this;
}

WebView2Settings WebView2::get_settings() const
{
    return m_pimpl->get_Settings();
}

const WebView2 &WebView2::navigate(rust::Slice<const uint16_t> url, rust::Box<NavigationCompletedHandler> handler) const
{
    m_pimpl->Navigate(url, std::move(handler), shared_from_this());
    return *this;
}

const WebView2 &WebView2::navigate_to_string(rust::Slice<const uint16_t> html_content, rust::Box<NavigationCompletedHandler> handler) const
{
    m_pimpl->NavigateToString(html_content, std::move(handler), shared_from_this());
    return *this;
}

const WebView2 &WebView2::execute_script(rust::Slice<const uint16_t> javascript, rust::Box<ExecuteScriptCompletedHandler> handler) const
{
    m_pimpl->ExecuteScript(javascript, std::move(handler));
    return *this;
}

const WebView2 &WebView2::reload() const
{
    m_pimpl->Reload();
    return *this;
}

const WebView2 &WebView2::post_web_message(rust::Slice<const uint16_t> json_message) const
{
    m_pimpl->PostWebMessage(json_message);
    return *this;
}

const WebView2 &WebView2::stop() const
{
    m_pimpl->Stop();
    return *this;
}

rust::Vec<uint16_t> WebView2::get_document_title() const
{
    return m_pimpl->get_DocumentTitle();
}

const WebView2 &WebView2::open_dev_tools_window() const
{
    m_pimpl->OpenDevToolsWindow();
    return *this;
}
