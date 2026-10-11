#[cfg(any(feature = "browser-js", test))]
use fly_browser::{BrowserAdapterConfig, FLY_BROWSER_ADAPTER_JS};
use leptos::prelude::*;
#[cfg(any(feature = "browser-js", test))]
use rustok_page_builder::browser_host::{
    PageBuilderBrowserModuleOptions, page_builder_browser_module,
};

/// Expected origin of the sandboxed authoring canvas iframe.
///
/// [`crate::editor::isolated_canvas`] renders the canvas with `sandbox="allow-scripts"` and
/// `srcdoc`, which gives the document an opaque origin: browsers report every `postMessage` from
/// it as the literal string `"null"` — [`fly_web::OPAQUE_ORIGIN`]. Fly's browser adapter fails
/// closed when no origin is configured, so the value is passed explicitly instead of relying on a
/// default, and the Rust-side subscription in [`crate::editor::isolated_canvas`] compares against
/// this same constant.
///
/// The sandbox attribute and this value are one contract: if the attribute changes (for example
/// to add `allow-same-origin`), this constant must change with it.
pub const PAGE_BUILDER_CANVAS_EXPECTED_ORIGIN: &str = fly_web::OPAQUE_ORIGIN;

#[cfg(any(feature = "browser-js", test))]
fn browser_adapter_config_json(
    intent_endpoint: Option<String>,
    csrf_token: Option<String>,
) -> Result<String, serde_json::Error> {
    BrowserAdapterConfig {
        expected_origin: Some(PAGE_BUILDER_CANVAS_EXPECTED_ORIGIN.to_string()),
        intent_endpoint,
        csrf_token,
        ..BrowserAdapterConfig::default()
    }
    .to_json()
}

/// Emits the standalone Fly browser bridge into a server-rendered Page Builder surface.
///
/// The Leptos component only renders the framework-neutral browser module descriptor. Fly project
/// state, commands, validation, rendering, permissions, host bindings and persistence remain
/// outside the UI adapter.
#[component]
pub fn PageBuilderBrowserAdapter(
    #[prop(optional_no_strip)] intent_endpoint: Option<String>,
    #[prop(optional_no_strip)] csrf_token: Option<String>,
    #[prop(optional_no_strip)] script_nonce: Option<String>,
) -> impl IntoView {
    #[cfg(feature = "browser-js")]
    {
        let config = browser_adapter_config_json(intent_endpoint, csrf_token)
            .unwrap_or_else(|_| "{}".to_string());
        let module = page_builder_browser_module(
            &config,
            FLY_BROWSER_ADAPTER_JS,
            PageBuilderBrowserModuleOptions {
                nonce: script_nonce,
            },
        );
        let script_type = module.script_type;
        let adapter = module.adapter;
        let nonce = module.nonce;
        let source = module.source;
        view! {
            <script
                type=script_type
                data-fly-browser-adapter=adapter
                nonce=nonce
                inner_html=source
            ></script>
        }
        .into_any()
    }

    #[cfg(not(feature = "browser-js"))]
    {
        let _ = (intent_endpoint, csrf_token, script_nonce);
        view! { <span hidden data-fly-browser-adapter="disabled"></span> }.into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::{DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS, DEFAULT_MAX_BROWSER_MESSAGE_BYTES};

    #[test]
    fn adapter_asset_does_not_depend_on_wasm_runtime() {
        assert!(FLY_BROWSER_ADAPTER_JS.contains("class FlyBrowserAdapter"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-intent"));
        assert!(!FLY_BROWSER_ADAPTER_JS.contains("wasm_bindgen"));
    }

    #[test]
    fn browser_config_uses_the_public_javascript_contract() {
        let json = browser_adapter_config_json(
            Some("/admin/fly/intents".to_string()),
            Some("csrf-token".to_string()),
        )
        .expect("browser config");
        let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
        assert_eq!(value["intentEndpoint"], "/admin/fly/intents");
        assert_eq!(value["csrfToken"], "csrf-token");
        // The canvas iframe is sandboxed with `allow-scripts` only, so its messages carry the
        // opaque origin. Fly fails closed without an explicit value; this host states it.
        assert_eq!(value["expectedOrigin"], "null");
        assert_eq!(value["maxMessageBytes"], DEFAULT_MAX_BROWSER_MESSAGE_BYTES);
        assert_eq!(
            value["maxGeometryComponents"],
            DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS
        );
        assert!(value.get("intent_endpoint").is_none());
        assert!(value.get("csrf_token").is_none());
    }

    #[test]
    fn framework_adapter_renders_the_shared_browser_module_descriptor() {
        let module = page_builder_browser_module(
            "{}",
            FLY_BROWSER_ADAPTER_JS,
            PageBuilderBrowserModuleOptions {
                nonce: Some("csp-nonce".to_string()),
            },
        );
        assert_eq!(module.script_type, "module");
        assert_eq!(module.adapter, "fly_browser");
        assert_eq!(module.nonce.as_deref(), Some("csp-nonce"));
        assert!(module.source.contains(&format!("{}:browser-ready", "fly")));
        assert!(module.source.contains("data-fly-intent-form"));
    }

    #[test]
    fn public_adapter_bundle_emits_typed_accessible_resource_limits() {
        assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-resource-limit"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("message_bytes"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("geometry_components"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("aria-live"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("role"));
    }
}
