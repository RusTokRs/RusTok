use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_RICHTEXT_ASSETS");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("richtext_assets.rs");
    if env::var_os("CARGO_FEATURE_RICHTEXT_ASSETS").is_none() {
        fs::write(output, "").expect("write empty richtext asset bindings");
        return;
    }

    let dist = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../../packages/richtext/dist");
    let manifest_path = dist.join("asset-manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read richtext asset manifest"))
            .expect("parse richtext asset manifest");

    let frame = asset_name(&manifest, "frame");
    let script = asset_name(&manifest, "script");
    let style = asset_name(&manifest, "style");
    let adapter = asset_name(&manifest, "leptos_adapter");
    for name in [&frame, &script, &style, &adapter] {
        println!("cargo:rerun-if-changed={}", dist.join(name).display());
    }

    let source = format!(
        "pub const SCRIPT_NAME: &str = {script:?};\n\
         pub const STYLE_NAME: &str = {style:?};\n\
         pub const ADAPTER_NAME: &str = {adapter:?};\n\
         pub const FRAME_BYTES: &[u8] = include_bytes!({frame_path:?});\n\
         pub const SCRIPT_BYTES: &[u8] = include_bytes!({script_path:?});\n\
         pub const STYLE_BYTES: &[u8] = include_bytes!({style_path:?});\n\
         pub const ADAPTER_BYTES: &[u8] = include_bytes!({adapter_path:?});\n",
        frame_path = dist.join(&frame),
        script_path = dist.join(&script),
        style_path = dist.join(&style),
        adapter_path = dist.join(&adapter),
    );
    fs::write(output, source).expect("write richtext asset bindings");
}

fn asset_name(manifest: &serde_json::Value, key: &str) -> String {
    let name = manifest
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("richtext asset manifest is missing {key}"));
    assert!(
        !name.is_empty()
            && !name.contains('/')
            && !name.contains('\\')
            && name != "."
            && name != "..",
        "richtext asset manifest contains unsafe {key} filename"
    );
    name.to_string()
}
