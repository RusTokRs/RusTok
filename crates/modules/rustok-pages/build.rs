use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_INLINE_EDIT_ASSETS").is_some() {
        let manifest_dir =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
        let root = manifest_dir.join("../..");
        let bootstrap = root.join("target/site/assets/pages-inline-edit-bootstrap.js");
        let module = root.join("target/site/assets/pages-inline-edit/rustok_storefront.js");
        let wasm = root.join("target/site/assets/pages-inline-edit/rustok_storefront_bg.wasm");

        if let Some(parent) = bootstrap.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Some(parent) = module.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if !bootstrap.exists() {
            let src = root.join("apps/storefront/public/assets/pages-inline-edit-bootstrap.js");
            let content = fs::read(&src).unwrap_or_default();
            let _ = fs::write(&bootstrap, content);
        }
        if !module.exists() {
            let _ = fs::write(&module, b"// stub\n");
        }
        if !wasm.exists() {
            let _ = fs::write(&wasm, b"");
        }
    }
}
