use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_COMMENT_ASSETS").is_some() {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
        let root = manifest_dir.join("../..");
        let bootstrap = root.join("target/site/assets/blog-comment-bootstrap.js");
        let module = root.join("target/site/assets/blog-comment/rustok_storefront.js");
        let wasm = root.join("target/site/assets/blog-comment/rustok_storefront_bg.wasm");

        if let Some(parent) = bootstrap.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = module.parent() {
            fs::create_dir_all(parent)?;
        }

        if !bootstrap.exists() {
            let src = root.join("apps/storefront/public/assets/blog-comment-bootstrap.js");
            match fs::read(&src) {
                Ok(content) => fs::write(&bootstrap, content)?,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    fs::write(&bootstrap, b"")?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        if !module.exists() {
            fs::write(&module, b"// stub\n")?;
        }
        if !wasm.exists() {
            fs::write(&wasm, b"")?;
        }
    }

    Ok(())
}
