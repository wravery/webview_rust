#pragma once

#include "rust/cxx.h"

#include <memory>

struct WebView2EnvironmentOptions;

struct CreateWebView2EnvironmentCompletedHandler;
struct CreateWebView2ControllerCompletedHandler;

void new_webview2_environment(rust::Box<CreateWebView2EnvironmentCompletedHandler> handler);

void new_webview2_environment_with_options(rust::Slice<const uint16_t> browser_executable_folder,
                                           rust::Slice<const uint16_t> user_data_folder,
                                           const WebView2EnvironmentOptions &options,
                                           rust::Box<CreateWebView2EnvironmentCompletedHandler> handler);

rust::Vec<uint16_t> get_available_webview2_browser_version_string(rust::Slice<const uint16_t> browser_executable_folder);

int8_t compare_browser_versions(rust::Slice<const uint16_t> version1, rust::Slice<const uint16_t> version2);

class WebView2Environment
{
public:
    WebView2Environment();
    ~WebView2Environment();

    void create_webview2_controller(ptrdiff_t parent_window, rust::Box<CreateWebView2ControllerCompletedHandler> handler) const;

    class impl;
    std::unique_ptr<impl> m_pimpl;
};

class WebView2Controller
{
public:
    WebView2Controller();
    ~WebView2Controller();

    class impl;
    std::unique_ptr<impl> m_pimpl;
};

class WebView2
{
public:
    WebView2();
    ~WebView2();

    class impl;
    std::unique_ptr<impl> m_pimpl;
};

