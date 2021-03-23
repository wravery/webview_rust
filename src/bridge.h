#pragma once

#include "rust/cxx.h"

#include <memory>

struct WebView2EnvironmentOptions;
struct BoundsRectangle;
struct WebView2Settings;

struct CreateWebView2EnvironmentCompletedHandler;
struct CreateWebView2ControllerCompletedHandler;
struct NavigationCompletedHandler;
struct ExecuteScriptCompletedHandler;

void new_webview2_environment(rust::Box<CreateWebView2EnvironmentCompletedHandler> handler);

void new_webview2_environment_with_options(rust::Slice<const uint16_t> browser_executable_folder,
                                           rust::Slice<const uint16_t> user_data_folder,
                                           const WebView2EnvironmentOptions &options,
                                           rust::Box<CreateWebView2EnvironmentCompletedHandler> handler);

rust::Vec<uint16_t> get_available_webview2_browser_version_string(rust::Slice<const uint16_t> browser_executable_folder);

int8_t compare_browser_versions(rust::Slice<const uint16_t> version1, rust::Slice<const uint16_t> version2);

class WebView2Environment
    : public std::enable_shared_from_this<WebView2Environment>
{
public:
    class impl;

    WebView2Environment(std::unique_ptr<impl> &&pimpl);
    ~WebView2Environment();

    const WebView2Environment &create_webview2_controller(ptrdiff_t parent_window, rust::Box<CreateWebView2ControllerCompletedHandler> handler) const;

private:
    std::unique_ptr<impl> m_pimpl;
};

class WebView2;

class WebView2Controller
    : public std::enable_shared_from_this<WebView2Controller>
{
public:
    class impl;

    WebView2Controller(std::unique_ptr<impl> &&pimpl);
    ~WebView2Controller();

    const WebView2Controller &visible(bool value) const;
    bool get_visible() const;
    const WebView2Controller &bounds(BoundsRectangle value) const;
    BoundsRectangle get_bounds() const;
    void close() const;
    std::shared_ptr<WebView2> get_webview() const;

private:
    std::unique_ptr<impl> m_pimpl;
};

class WebView2
    : public std::enable_shared_from_this<WebView2>
{
public:
    class impl;

    WebView2(std::unique_ptr<impl> &&pimpl);
    ~WebView2();

    const WebView2 &settings(WebView2Settings value) const;
    WebView2Settings get_settings() const;
    const WebView2 &navigate(rust::Slice<const uint16_t> url, rust::Box<NavigationCompletedHandler> handler) const;
    const WebView2 &navigate_to_string(rust::Slice<const uint16_t> html_content, rust::Box<NavigationCompletedHandler> handler) const;
    const WebView2 &execute_script(rust::Slice<const uint16_t> javascript, rust::Box<ExecuteScriptCompletedHandler> handler) const;
    const WebView2 &reload() const;
    const WebView2 &post_web_message(rust::Slice<const uint16_t> json_message) const;
    const WebView2 &stop() const;
    rust::Vec<uint16_t> get_document_title() const;
    const WebView2 &open_dev_tools_window() const;

private:
    std::unique_ptr<impl> m_pimpl;
};
