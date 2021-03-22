use std::io;

fn main() -> io::Result<()> {
    let webview2_arch = webview2_nuget::get_arch()?;
    let webview2_path = webview2_nuget::install()?;
    let source_path = webview2_nuget::link_dll(&webview2_path, &webview2_arch)?;
    let target_path = webview2_nuget::get_target_path()?;
    webview2_nuget::copy_dll(&source_path, &target_path)
}

mod webview2_nuget {
    use std::env;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::process::Command;

    const WEBVIEW2_NAME: &str = "Microsoft.Web.WebView2";
    const WEBVIEW2_VERSION: &str = "1.0.774.44";
    const WEBVIEW2_DLL: &str = "WebView2Loader.dll";

    pub fn install() -> io::Result<String> {
        if !check_nuget_dir()? {
            Command::new("./tools/nuget.exe")
                .args(&[
                    "install",
                    WEBVIEW2_NAME,
                    "-OutputDirectory",
                    ".",
                    "-Version",
                    WEBVIEW2_VERSION,
                ])
                .output()?;

            if !check_nuget_dir()? {
                return Err(io::Error::from(io::ErrorKind::NotFound));
            }
        }

        Ok(format!(
            "{}.{}/build/native",
            WEBVIEW2_NAME, WEBVIEW2_VERSION
        ))
    }

    pub fn get_arch() -> io::Result<String> {
        match env::var("TARGET") {
            Ok(target) => {
                if target.contains("x86_x64") {
                    Ok(String::from("x64"))
                } else {
                    Ok(String::from("x86"))
                }
            }
            Err(_) => Err(io::Error::from(io::ErrorKind::InvalidInput)),
        }
    }

    pub fn link_dll(webview2_path: &str, webview2_arch: &str) -> io::Result<PathBuf> {
        // calculate full path to WebView2Loader.dll
        let mut webview2_path_buf = PathBuf::from(webview2_path);
        webview2_path_buf.push(webview2_arch);
        let webview2_dir = webview2_path_buf.as_path().to_str().unwrap();

        println!("cargo:rustc-link-search={}", webview2_dir);
        println!("cargo:rustc-link-lib={}", WEBVIEW2_DLL);

        webview2_path_buf.push(WEBVIEW2_DLL);
        Ok(webview2_path_buf)
    }

    pub fn get_target_path() -> io::Result<PathBuf> {
        match env::var("OUT_DIR") {
            Ok(out_dir) => {
                // we want to be able to calculate C:\crate\root\target\debug\
                //           while we can get this ^^^^^^^^^^^^^   and  ^^^^^ from env::current_dir() and %PROFILE% respectively
                // there's no way to get this (reliably)         ^^^^^^
                // we can, however, use %OUT_DIR% (C:\crate\root\target\debug\build\webview_rust-xxxx\out\)
                // and navigate backwards to here  ^^^^^^^^^^^^^^^^^^^^^^^^^^
                let mut target_path = PathBuf::from(out_dir);
                target_path.pop();
                target_path.pop();
                target_path.pop();
                target_path.push(WEBVIEW2_DLL);

                Ok(target_path)
            }
            Err(_) => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }

    pub fn copy_dll(source_path: &PathBuf, target_path: &PathBuf) -> io::Result<()> {
        fs::copy(source_path.as_path(), target_path.as_path())?;
        Ok(())
    }

    fn check_nuget_dir() -> io::Result<bool> {
        let nuget_path = format!("{}.{}", WEBVIEW2_NAME, WEBVIEW2_VERSION);
        let mut dir_iter = fs::read_dir(".")?.filter(|dir| match dir {
            Ok(dir) => match dir.file_type() {
                Ok(file_type) => {
                    file_type.is_dir()
                        && match dir.file_name().to_str() {
                            Some(name) => name.eq_ignore_ascii_case(&nuget_path),
                            None => false,
                        }
                }
                Err(_) => false,
            },
            Err(_) => false,
        });
        Ok(dir_iter.next().is_some())
    }
}
