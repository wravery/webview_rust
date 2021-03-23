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

    Microsoft::WRL::ComPtr<ICoreWebView2Controller> m_controller;

private:
    void CheckCreated() const;
};

class WebView2::impl
{
public:
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

void WebView2Controller::impl::CheckCreated() const
{
    if (!m_controller)
    {
        throw std::runtime_error("ICoreWebView2Controller creation failed");
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
    auto browserExecutableFolder = to_wstring(browser_executable_folder);
    auto userDataFolder = to_wstring(user_data_folder);
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

    if (FAILED(CreateCoreWebView2EnvironmentWithOptions(browserExecutableFolder.c_str(),
                                                        userDataFolder.c_str(),
                                                        spOptions.Get(),
                                                        callback.Get())))
    {
        throw std::runtime_error("CreateCoreWebView2Environment failed");
    }
}

rust::Vec<uint16_t> get_available_webview2_browser_version_string(rust::Slice<const uint16_t> browser_executable_folder)
{
    std::wstring browserExecutableFolder{
        reinterpret_cast<const wchar_t *>(browser_executable_folder.data()),
        browser_executable_folder.size()};
    PWSTR version = nullptr;

    if (FAILED(GetAvailableCoreWebView2BrowserVersionString(browserExecutableFolder.c_str(), &version)))
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

void WebView2Environment::create_webview2_controller(ptrdiff_t parent_window, rust::Box<CreateWebView2ControllerCompletedHandler> handler) const
{
    m_pimpl->CreateWebView2Controller(parent_window, std::move(handler));
}

WebView2Controller::WebView2Controller()
    : m_pimpl{std::make_unique<impl>()}
{
}

WebView2Controller::~WebView2Controller()
{
}

const WebView2Controller& WebView2Controller::visible(bool value) const
{
    m_pimpl->put_IsVisible(value);
    return *this;
}

bool WebView2Controller::is_visible() const
{
    return m_pimpl->get_IsVisible();
}

WebView2::WebView2()
    : m_pimpl{std::make_unique<impl>()}
{
}

WebView2::~WebView2()
{
}
