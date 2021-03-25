use webview_official::{SizeHint, WebviewBuilder};

fn main() {
    let mut webview = WebviewBuilder::new()
        .debug(true)
        .title("TEST")
        .width(1024)
        .height(768)
        .resize(SizeHint::NONE)
        .init("window.x = 42")
        .dispatch(|w| {
            w.eval("console.log('The anwser is ' + window.x);");
            w.set_size(800, 600, SizeHint::MIN);
            println!("Hello World");
        })
        .url("https://google.com")
        .build();

    let w = webview.clone();
    webview.bind("xxx", move |seq, req| {
        println!("xxx called with {}", req);
        w.r#return(seq, 0, "{ result: 'We always knew it!' }");
    });
    webview.run();
}
