mod common;

use futures::{channel::oneshot, executor, task::LocalSpawnExt};
use webview_official::bridge;

#[test]
fn webview_script_to_execute_on_document_created() {
    common::initialize_com();

    {
        let frame = common::create_test_window("webview_script_to_execute_on_document_created");
        let webview = common::create_test_webview(&frame);
        let (tx, rx) = oneshot::channel();
        let mut pool = executor::LocalPool::new();
        let spawner = pool.spawner();
        let output = spawner
            .spawn_local_with_handle(rx)
            .expect("spawn_local_with_handle");
        let javascript =
            bridge::to_utf16(r#"window.addScriptToExecuteOnDocumentCreated = "Make it so!";"#);
        webview
            .add_script_to_execute_on_document_created(
                &javascript,
                Box::new(
                    bridge::AddScriptToExecuteOnDocumentCreatedCompletedHandler::new(Box::new(
                        move |id| {
                            tx.send(id).expect("send the message to the waiter");
                        },
                    )),
                ),
            )
            .expect("call add_script_to_execute_on_document_created");

        common::run_message_loop(&mut pool);

        let script_id = pool.run_until(output).expect("received the script id");
        let script_id = bridge::to_utf16(&&script_id);

        common::navigate_test_webview(&webview);

        let result =
            common::execute_test_script(&webview, r#"window.addScriptToExecuteOnDocumentCreated"#);

        webview
            .remove_script_to_execute_on_document_created(&script_id)
            .expect("call remove_script_to_execute_on_document_created");

        println!("window.addScriptToExecuteOnDocumentCreated: {}", result);

        assert_eq!(result, r#""Make it so!""#);
    }

    // Wait until the webview has gone out of scope before calling CoUninitialize.
    common::uninitialize_com();
}
