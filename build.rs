use std::io;

fn main() -> io::Result<()> {
    let _webview2_path = webview2_nuget::install()?;

    Ok(())
}

mod webview2_nuget {
    use std::fs;
    use std::io;
    use std::process::Command;

    const WEBVIEW2_NAME: &str = "Microsoft.Web.WebView2";
    const WEBVIEW2_VERSION: &str = "1.0.774.44";

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
