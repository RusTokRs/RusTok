#![cfg(feature = "ssr")]

use std::collections::HashMap;

use rustok_storefront::render_shell;

#[tokio::test]
async fn anonymous_pages_host_renders_without_executable_client_bootstrap() {
    let document = render_shell("en", HashMap::new()).await;

    assert!(document.starts_with("<!DOCTYPE html>"));
    assert!(document.contains("<div id=\"app\">"));
    assert!(document.contains("<link rel=\"stylesheet\" href=\"/assets/app.css\" />"));

    for forbidden in [
        "<script src=",
        "<script type=\"module\"",
        "rel=\"modulepreload\"",
        "wasm-bindgen",
        "__wbindgen",
        ".wasm",
        "/pkg/",
        "hydrate_body",
        "mount_to_body",
        "rustok-pages-admin",
        "rustok-page-builder-admin",
        "fly-browser",
        "fly-ui",
        "fly-leptos",
        "PagesFlyBuilder",
        "PageBuilderAdmin",
    ] {
        assert!(
            !document.contains(forbidden),
            "anonymous SSR document unexpectedly contains executable/authoring marker {forbidden}"
        );
    }
}
