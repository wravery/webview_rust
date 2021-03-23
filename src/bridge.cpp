#include "webview_official/src/bridge.h"
#include "webview_official/src/bridge.rs.h"

#include <WebView2.h>
#include <WebView2EnvironmentOptions.h>

#include <windows.h>
#include <wrl.h>

#include <algorithm>
#include <iterator>
#include <stdexcept>
#include <string>
#include <string_view>

using SPWSTR = std::unique_ptr<wchar_t[], decltype(&::CoTaskMemFree)>;

std::wstring to_wstring(const rust::Vec<uint16_t> source)
{
    return {reinterpret_cast<const wchar_t *>(source.data()), source.size()};
}

std::wstring to_wstring(rust::Slice<const uint16_t> source)
{
    return {reinterpret_cast<const wchar_t *>(source.data()), source.size()};
}

rust::Vec<uint16_t> to_vec(std::wstring_view source)
{
    rust::Vec<uint16_t> result;

    result.reserve(source.size());
    std::transform(source.begin(), source.end(), std::back_inserter(result), [](wchar_t ch) {
        return static_cast<uint16_t>(ch);
    });

    return result;
}

class WebView2Environment::impl
{
public:
    void CreateWebView2Controller(ptrdiff_t parent_window, rust::Box<CreateWebView2ControllerCompletedHandler> handler) const;

    Microsoft::WRL::ComPtr<ICoreWebView2Environment> m_environment;

private:
    void CheckCreated() const;
};

class WebView2Controller::impl
{
public:
    bool get_IsVisible() const;
    void put_IsVisible(bool value) const;
    BoundsRectangle get_Bounds() const;
    void put_Bounds(BoundsRectangle value) const;
    void Close();
    const std::shared_ptr<WebView2> &get_WebView();

    Microsoft::WRL::ComPtr<ICoreWebView2Controller> m_controller;

private:
    void CheckCreated() const;

    std::shared_ptr<WebView2> m_webview;
};

class WebView2::impl
{
public:
    WebView2Settings get_Settings() const;
    void put_Settings(WebView2Settings value) const;
    void Navigate(rust::Slice<const uint16_t> url) const;
    void NavigateToString(rust::Slice<const uint16_t> html_content) const;
    void ExecuteScript(rust::Slice<const uint16_t> javascript, rust::Box<ExecuteScriptCompletedHandler> handler) const;
    void Reload() const;
    void PostWebMessage(rust::Slice<const uint16_t> json_message) const;
    void Stop() const;
    rust::Vec<uint16_t> GetDocumentTitle() const;
    void OpenDevToolsWindow() const;

    Microsoft::WRL::ComPtr<ICoreWebView2> m_webview;

private:
    void CheckCreated() const;
};

void WebView2Environment::impl::CreateWebView2Controller(ptrdiff_t parent_window, rust::Box<CreateWebView2ControllerCompletedHandler> handler) const
{
    CheckCreated();

    auto callback = Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>(
        [handler = std::move(handler)](HRESULT hr, ICoreWebView2Controller *controller) mutable noexcept {
            std::unique_ptr<WebView2Controller> result;

            if (SUCCEEDED(hr) && nullptr != controller)
            {
                result = std::make_unique<WebView2Controller>();
                result->m_pimpl->m_controller = controller;
            }

            invoke_controller_complete(std::move(handler), std::move(result));
            return S_OK;
        });

    if (FAILED(m_environment->CreateCoreWebView2Controller(reinterpret_cast<HWND>(parent_window), callback.Get())))
    {
        throw std::runtime_error("CreateCoreWebView2Controller failed");
    }
}

void WebView2Environment::impl::CheckCreated() const
{
    if (!m_environment)
    {
        throw std::runtime_error("ICoreWebView2Environment creation failed");
    }
}

bool WebView2Controller::impl::get_IsVisible() const
{
    CheckCreated();

    BOOL result = false;

    if (FAILED(m_controller->get_IsVisible(&result)))
    {
        throw std::runtime_error("get_IsVisible failed");
    }

    return static_cast<bool>(result);
}

void WebView2Controller::impl::put_IsVisible(bool value) const
{
    CheckCreated();

    if (FAILED(m_controller->put_IsVisible(static_cast<BOOL>(value))))
    {
        throw std::runtime_error("put_IsVisible failed");
    }
}

BoundsRectangle WebView2Controller::impl::get_Bounds() const
{
    CheckCreated();

    RECT value{};

    if (FAILED(m_controller->get_Bounds(&value)))
    {
        throw std::runtime_error("get_Bounds failed");
    }

    BoundsRectangle result{};

    result.left = value.left;
    result.top = value.top;
    result.right = value.right;
    result.bottom = value.bottom;

    return result;
}

void WebView2Controller::impl::put_Bounds(BoundsRectangle value) const
{
    CheckCreated();

    RECT result{};

    result.left = value.left;
    result.top = value.top;
    result.right = value.right;
    result.bottom = value.bottom;

    if (FAILED(m_controller->put_Bounds(result)))
    {
        throw std::runtime_error("put_Bounds failed");
    }
}

void WebView2Controller::impl::Close()
{
    CheckCreated();

    if (FAILED(m_controller->Close()))
    {
        throw std::runtime_error("Close failed");
    }

    m_controller = nullptr;
}

const std::shared_ptr<WebView2> &WebView2Controller::impl::get_WebView()
{
    CheckCreated();

    if (!m_webview)
    {
        auto webview = std::make_shared<WebView2>();

        if (FAILED(m_controller->get_CoreWebView2(&webview->m_pimpl->m_webview)))
        {
            throw std::runtime_error("get_CoreWebView2 failed");
        }

        m_webview = std::move(webview);
    }

    return m_webview;
}

void WebView2Controller::impl::CheckCreated() const
{
    if (!m_controller)
    {
        throw std::runtime_error("ICoreWebView2Controller creation failed");
    }
}

WebView2Settings WebView2::impl::get_Settings() const
{
    CheckCreated();

    Microsoft::WRL::ComPtr<ICoreWebView2Settings> settings;

    if (FAILED(m_webview->get_Settings(&settings)))
    {
        throw std::runtime_error("get_Settings failed");
    }

    BOOL isScriptEnabled = false;
    BOOL isWebMessageEnabled = false;
    BOOL areDefaultScriptDialogsEnabled = false;
    BOOL isStatusBarEnabled = false;
    BOOL areDevToolsEnabled = false;
    BOOL areDefaultContextMenusEnabled = false;
    BOOL isZoomControlEnabled = false;
    BOOL isBuiltInErrorPageEnabled = false;

    if (FAILED(settings->get_IsScriptEnabled(&isScriptEnabled)))
    {
        throw std::runtime_error("get_IsScriptEnabled failed");
    }

    if (FAILED(settings->get_IsWebMessageEnabled(&isWebMessageEnabled)))
    {
        throw std::runtime_error("get_IsWebMessageEnabled failed");
    }

    if (FAILED(settings->get_AreDefaultScriptDialogsEnabled(&areDefaultScriptDialogsEnabled)))
    {
        throw std::runtime_error("get_AreDefaultScriptDialogsEnabled failed");
    }

    if (FAILED(settings->get_IsStatusBarEnabled(&isStatusBarEnabled)))
    {
        throw std::runtime_error("get_IsStatusBarEnabled failed");
    }

    if (FAILED(settings->get_AreDevToolsEnabled(&areDevToolsEnabled)))
    {
        throw std::runtime_error("get_AreDevToolsEnabled failed");
    }

    if (FAILED(settings->get_AreDefaultContextMenusEnabled(&areDefaultContextMenusEnabled)))
    {
        throw std::runtime_error("get_AreDefaultContextMenusEnabled failed");
    }

    if (FAILED(settings->get_IsZoomControlEnabled(&isZoomControlEnabled)))
    {
        throw std::runtime_error("get_IsZoomControlEnabled failed");
    }

    if (FAILED(settings->get_IsBuiltInErrorPageEnabled(&isBuiltInErrorPageEnabled)))
    {
        throw std::runtime_error("get_IsBuiltInErrorPageEnabled failed");
    }

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

    if (FAILED(m_webview->get_Settings(&settings)))
    {
        throw std::runtime_error("get_Settings failed");
    }

    if (FAILED(settings->put_IsScriptEnabled(static_cast<BOOL>(value.is_script_enabled))))
    {
        throw std::runtime_error("put_IsScriptEnabled failed");
    }

    if (FAILED(settings->put_IsWebMessageEnabled(static_cast<BOOL>(value.is_web_message_enabled))))
    {
        throw std::runtime_error("put_IsWebMessageEnabled failed");
    }

    if (FAILED(settings->put_AreDefaultScriptDialogsEnabled(static_cast<BOOL>(value.are_default_script_dialogs_enabled))))
    {
        throw std::runtime_error("put_AreDefaultScriptDialogsEnabled failed");
    }

    if (FAILED(settings->put_IsStatusBarEnabled(static_cast<BOOL>(value.is_status_bar_enabled))))
    {
        throw std::runtime_error("put_IsStatusBarEnabled failed");
    }

    if (FAILED(settings->put_AreDevToolsEnabled(static_cast<BOOL>(value.are_dev_tools_enabled))))
    {
        throw std::runtime_error("put_AreDevToolsEnabled failed");
    }

    if (FAILED(settings->put_AreDefaultContextMenusEnabled(static_cast<BOOL>(value.are_default_context_menus_enabled))))
    {
        throw std::runtime_error("put_AreDefaultContextMenusEnabled failed");
    }

    if (FAILED(settings->put_IsZoomControlEnabled(static_cast<BOOL>(value.is_zoom_control_enabled))))
    {
        throw std::runtime_error("put_IsZoomControlEnabled failed");
    }

    if (FAILED(settings->put_IsBuiltInErrorPageEnabled(static_cast<BOOL>(value.is_built_in_error_page_enabled))))
    {
        throw std::runtime_error("put_IsBuiltInErrorPageEnabled failed");
    }
}

void WebView2::impl::Navigate(rust::Slice<const uint16_t> url) const
{
    CheckCreated();

    if (FAILED(m_webview->Navigate(to_wstring(url).c_str())))
    {
        throw std::runtime_error("Navigate failed");
    }
}

void WebView2::impl::NavigateToString(rust::Slice<const uint16_t> html_content) const
{
    CheckCreated();

    if (FAILED(m_webview->NavigateToString(to_wstring(html_content).c_str())))
    {
        throw std::runtime_error("NavigateToString failed");
    }
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

    if (FAILED(m_webview->ExecuteScript(to_wstring(javascript).c_str(), callback.Get())))
    {
        throw std::runtime_error("ExecuteScript failed");
    }
}

void WebView2::impl::Reload() const
{
    CheckCreated();

    if (FAILED(m_webview->Reload()))
    {
        throw std::runtime_error("Reload failed");
    }
}

void WebView2::impl::PostWebMessage(rust::Slice<const uint16_t> json_message) const
{
    CheckCreated();

    if (FAILED(m_webview->PostWebMessageAsJson(to_wstring(json_message).c_str())))
    {
        throw std::runtime_error("Stop failed");
    }
}

void WebView2::impl::Stop() const
{
    CheckCreated();

    if (FAILED(m_webview->Stop()))
    {
        throw std::runtime_error("Stop failed");
    }
}

rust::Vec<uint16_t> WebView2::impl::GetDocumentTitle() const
{
    CheckCreated();

    PWSTR documentTitle = nullptr;

    if (FAILED(m_webview->get_DocumentTitle(&documentTitle)))
    {
        throw std::runtime_error("get_DocumentTitle failed");
    }

    SPWSTR cleanup{documentTitle, ::CoTaskMemFree};

    return to_vec(documentTitle);
}

void WebView2::impl::OpenDevToolsWindow() const
{
    CheckCreated();

    if (FAILED(m_webview->OpenDevToolsWindow()))
    {
        throw std::runtime_error("OpenDevToolsWindow failed");
    }
}

void WebView2::impl::CheckCreated() const
{
    if (!m_webview)
    {
        throw std::runtime_error("ICoreWebView2 creation failed");
    }
}

void new_webview2_environment(rust::Box<CreateWebView2EnvironmentCompletedHandler> handler)
{
    auto callback = Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
        [handler = std::move(handler)](HRESULT hr, ICoreWebView2Environment *environment) mutable noexcept {
            std::unique_ptr<WebView2Environment> result;

            if (SUCCEEDED(hr) && nullptr != environment)
            {
                result = std::make_unique<WebView2Environment>();
                result->m_pimpl->m_environment = environment;
            }

            invoke_environment_complete(std::move(handler), std::move(result));
            return S_OK;
        });

    if (FAILED(CreateCoreWebView2Environment(callback.Get())))
    {
        throw std::runtime_error("CreateCoreWebView2Environment failed");
    }
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

        if (FAILED(spOptions->put_AdditionalBrowserArguments(additionalBrowserArguments.c_str())))
        {
            throw std::runtime_error("put_AdditionalBrowserArguments failed");
        }
    }

    if (!options.language.empty())
    {
        auto language = to_wstring(options.language);

        if (FAILED(spOptions->put_Language(language.c_str())))
        {
            throw std::runtime_error("put_Language failed");
        }
    }

    if (!options.target_compatible_browser_version.empty())
    {
        auto targetCompatibleBrowserVersion = to_wstring(options.target_compatible_browser_version);

        if (FAILED(spOptions->put_TargetCompatibleBrowserVersion(targetCompatibleBrowserVersion.c_str())))
        {
            throw std::runtime_error("put_TargetCompatibleBrowserVersion failed");
        }
    }

    if (FAILED(spOptions->put_AllowSingleSignOnUsingOSPrimaryAccount(options.allow_single_sign_on_using_os_primary_account)))
    {
        throw std::runtime_error("put_AllowSingleSignOnUsingOSPrimaryAccount failed");
    }

    auto callback = Microsoft::WRL::Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
        [handler = std::move(handler)](HRESULT hr, ICoreWebView2Environment *environment) mutable noexcept {
            std::unique_ptr<WebView2Environment> result;

            if (SUCCEEDED(hr) && nullptr != environment)
            {
                result = std::make_unique<WebView2Environment>();
                result->m_pimpl->m_environment = environment;
            }

            invoke_environment_complete(std::move(handler), std::move(result));
            return S_OK;
        });

    if (FAILED(CreateCoreWebView2EnvironmentWithOptions(to_wstring(browser_executable_folder).c_str(),
                                                        to_wstring(user_data_folder).c_str(),
                                                        spOptions.Get(),
                                                        callback.Get())))
    {
        throw std::runtime_error("CreateCoreWebView2Environment failed");
    }
}

rust::Vec<uint16_t> get_available_webview2_browser_version_string(rust::Slice<const uint16_t> browser_executable_folder)
{
    PWSTR version = nullptr;

    if (FAILED(GetAvailableCoreWebView2BrowserVersionString(to_wstring(browser_executable_folder).c_str(), &version)))
    {
        throw std::runtime_error("GetAvailableCoreWebView2BrowserVersionString failed");
    }

    SPWSTR cleanup{version, ::CoTaskMemFree};

    return to_vec(version);
}

int8_t compare_browser_versions(rust::Slice<const uint16_t> version1, rust::Slice<const uint16_t> version2)
{
    int result = 0;

    if (FAILED(CompareBrowserVersions(to_wstring(version1).c_str(), to_wstring(version2).c_str(), &result)))
    {
        throw std::runtime_error("CompareBrowserVersions failed");
    }

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

WebView2Environment::WebView2Environment()
    : m_pimpl{std::make_unique<impl>()}
{
}

WebView2Environment::~WebView2Environment()
{
}

const WebView2Environment &WebView2Environment::create_webview2_controller(ptrdiff_t parent_window, rust::Box<CreateWebView2ControllerCompletedHandler> handler) const
{
    m_pimpl->CreateWebView2Controller(parent_window, std::move(handler));
    return *this;
}

WebView2Controller::WebView2Controller()
    : m_pimpl{std::make_unique<impl>()}
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

const WebView2Controller &WebView2Controller::bounds(BoundsRectangle value) const
{
    m_pimpl->put_Bounds(value);
    return *this;
}

BoundsRectangle WebView2Controller::get_bounds() const
{
    return m_pimpl->get_Bounds();
}

void WebView2Controller::close() const
{
    m_pimpl->Close();
}

std::shared_ptr<WebView2> WebView2Controller::get_webview() const
{
    return {m_pimpl->get_WebView()};
}

WebView2::WebView2()
    : m_pimpl{std::make_unique<impl>()}
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

const WebView2 &WebView2::navigate(rust::Slice<const uint16_t> url) const
{
    m_pimpl->Navigate(url);
    return *this;
}

const WebView2 &WebView2::navigate_to_string(rust::Slice<const uint16_t> html_content) const
{
    m_pimpl->NavigateToString(html_content);
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
    return m_pimpl->GetDocumentTitle();
}

const WebView2 &WebView2::open_dev_tools_window() const
{
    m_pimpl->OpenDevToolsWindow();
    return *this;
}
