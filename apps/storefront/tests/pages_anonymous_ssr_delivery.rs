#![cfg(feature = "ssr")]

const HOST_SOURCE: &str = include_str!("../src/lib.rs");

fn render_document_source() -> &'static str {
    let start = HOST_SOURCE
        .find("fn render_document(")
        .expect("render_document source");
    let end = HOST_SOURCE[start..]
        .find("#[cfg(feature = \"ssr\")]\nasync fn read_storefront_modules")
        .map(|offset| start + offset)
        .expect("render_document end");
    &HOST_SOURCE[start..end]
}

#[test]
fn anonymous_pages_host_source_has_no_executable_client_bootstrap() {
    let document = render_document_source();

    assert!(document.contains("<!DOCTYPE html>"));
    assert!(document.contains("<div id=\"app\">{app_html}</div>"));
    assert!(document.contains("<link rel=\"stylesheet\" href=\"/assets/app.css\" />"));
    assert!(document.contains("{extra_head}"));

    for forbidden in [
        "<script src=",
        "rel=\"modulepreload\"",
        ".wasm",
        "/pkg/",
        "hydrate_body",
        "mount_to_body",
    ] {
        assert!(
            !document.contains(forbidden),
            "anonymous SSR document source unexpectedly contains executable marker {forbidden}"
        );
    }

    for forbidden in [
        "#[wasm_bindgen(start)]",
        "wasm_bindgen(start)",
        "mount_to_body(",
        "hydrate_body(",
        "rustok-pages-admin",
        "rustok-page-builder-admin",
        "fly-browser",
        "fly-ui",
        "fly-leptos",
        "PagesFlyBuilder",
        "PageBuilderAdminHostContext",
        "PageBuilderAdmin",
    ] {
        assert!(
            !HOST_SOURCE.contains(forbidden),
            "anonymous storefront host source unexpectedly contains client/authoring marker {forbidden}"
        );
    }

    assert!(document.contains("data-blog-comment-island=\"true\""));
    assert!(document.contains("csp_nonce.map"));
    assert!(document.contains("/assets/blog-comment-bootstrap.js"));
}

#[test]
fn executable_island_bootstraps_require_a_response_nonce() {
    let document = render_document_source();
    assert!(document.contains("csp_nonce.map"));
    assert!(document.contains("nonce=\"{}\""));
    assert!(document.contains(".unwrap_or_default()"));
}
