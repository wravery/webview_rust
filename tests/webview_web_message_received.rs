mod common;

use futures::{channel::oneshot, executor, task::LocalSpawnExt};
use std::{sync::mpsc, thread};
use webview_official::bridge;

#[test]
fn webview_add_web_message_received() {
    common::initialize_com();

    let (tx_wait, rx_wait) = mpsc::channel();
    let (tx_fut, rx_fut) = oneshot::channel();
    let context = Box::new(common::MessageLoopCompletedContext::new(tx_fut));
    let waiter = thread::spawn(move || {
        let (source, message) = rx_wait.recv().expect("receive payload over mpsc channel");
        context.send((source, message));
    });
    let mut pool = executor::LocalPool::new();
    let spawner = pool.spawner();
    let output = spawner
        .spawn_local_with_handle(rx_fut)
        .expect("spawn_local_with_handle");

    {
        let frame = common::create_test_window("webview_add_web_message_received");
        let webview = common::navigate_to_test_html(&frame);
        let token = webview
            .add_web_message_received(Box::new(bridge::WebMessageReceivedHandler::new(Box::new(
                move |source, message| {
                    tx_wait
                        .send((source, message))
                        .expect("send the message to the waiter");
                },
            ))))
            .expect("call add_web_message_received");

        common::execute_test_script(
            &webview,
            r#"window.chrome.webview.postMessage({ foo: "bar", baz: true });"#,
        );

        common::run_message_loop(&mut pool);

        waiter.join().expect("join the waiter");
        webview
            .remove_web_message_received(token)
            .expect("call remove_web_message_received");

        let (source, message) = pool.run_until(output).expect("received the message");

        println!("WebMessageReceived (from {}): {}", source, message);

        assert_eq!(source, "about:blank");
        assert_eq!(message, r#"{"foo":"bar","baz":true}"#);
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
