#pragma once

#include "rust/cxx.h"

#include <cstdint>
#include <memory>

// Shared structs defined in bridge.rs.h
struct WebView2EnvironmentOptions;
struct WebView2ControllerBounds;
struct WebView2Settings;

// Opaque extern "Rust" structs
struct CreateWebView2EnvironmentCompletedHandler;
struct CreateWebView2ControllerCompletedHandler;
struct NavigationCompletedHandler;
struct AddScriptToExecuteOnDocumentCreatedCompletedHandler;
struct ExecuteScriptCompletedHandler;
struct WebMessageReceivedHandler;

void new_webview2_environment(rust::Box<CreateWebView2EnvironmentCompletedHandler> handler);

void new_webview2_environment_with_options(rust::Slice<const std::uint16_t> browser_executable_folder,
                                           rust::Slice<const std::uint16_t> user_data_folder,
                                           const WebView2EnvironmentOptions &options,
                                           rust::Box<CreateWebView2EnvironmentCompletedHandler> handler);

rust::Vec<std::uint16_t> get_available_webview2_browser_version_string(rust::Slice<const std::uint16_t> browser_executable_folder);

int8_t compare_browser_versions(rust::Slice<const std::uint16_t> version1, rust::Slice<const std::uint16_t> version2);

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
    const WebView2Controller &bounds(WebView2ControllerBounds value) const;
    WebView2ControllerBounds get_bounds() const;
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
    const WebView2 &navigate(rust::Slice<const std::uint16_t> url, rust::Box<NavigationCompletedHandler> handler) const;
    const WebView2 &navigate_to_string(rust::Slice<const std::uint16_t> html_content, rust::Box<NavigationCompletedHandler> handler) const;
    const WebView2 &add_script_to_execute_on_document_created(rust::Slice<const std::uint16_t> javascript, rust::Box<AddScriptToExecuteOnDocumentCreatedCompletedHandler> handler) const;
    const WebView2 &remove_script_to_execute_on_document_created(rust::Slice<const std::uint16_t> id) const;
    const WebView2 &execute_script(rust::Slice<const std::uint16_t> javascript, rust::Box<ExecuteScriptCompletedHandler> handler) const;
    const WebView2 &reload() const;
    const WebView2 &post_web_message(rust::Slice<const std::uint16_t> json_message) const;
    std::int64_t add_web_message_received(rust::Box<WebMessageReceivedHandler> handler) const;
    const WebView2 &remove_web_message_received(std::int64_t token) const;
    const WebView2 &stop() const;
    rust::Vec<std::uint16_t> get_document_title() const;
    const WebView2 &open_dev_tools_window() const;

private:
    std::unique_ptr<impl> m_pimpl;
};
