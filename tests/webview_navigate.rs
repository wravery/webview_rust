mod common;

use futures::{channel::oneshot, executor, task::LocalSpawnExt};
use webview_official::bridge;

#[test]
fn webview_navigate() {
    common::initialize_com();

    let (tx, rx) = oneshot::channel();
    let mut pool = executor::LocalPool::new();
    let spawner = pool.spawner();
    let context = Box::new(common::MessageLoopCompletedContext::new(tx));
    let output = spawner
        .spawn_local_with_handle(rx)
        .expect("spawn_local_with_handle");

    {
        let webview = common::create_test_webview();
        let url = bridge::to_utf16("https://aka.ms/webview2");
        webview
            .navigate(
                &url,
                Box::new(bridge::NavigationCompletedHandler::new(Box::new(
                    |_webview| {
                        context.send(());
                    },
                ))),
            )
            .expect("call navigate");

        common::run_message_loop(&mut pool);

        pool.run_until(output).expect("completed the navigation");
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
