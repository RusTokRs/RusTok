use fly_browser::{BrowserAdapterConfig, FLY_BROWSER_ADAPTER_JS};
use leptos::prelude::*;
use rustok_page_builder::browser_host::{
    page_builder_browser_module_source, PAGE_BUILDER_BROWSER_ADAPTER,
};

fn browser_adapter_config_json(
    intent_endpoint: Option<String>,
    csrf_token: Option<String>,
) -> Result<String, serde_json::Error> {
    BrowserAdapterConfig {
        intent_endpoint,
        csrf_token,
        ..BrowserAdapterConfig::default()
    }
    .to_json()
}

/// Emits the standalone Fly browser bridge into a server-rendered Page Builder surface.
///
/// The Leptos component only renders the framework-neutral browser module source. Fly project
/// state, commands, validation, rendering, permissions, host bindings and persistence remain
/// outside the UI adapter.
#[component]
pub fn PageBuilderBrowserAdapter(
    #[prop(optional_no_strip)] intent_endpoint: Option<String>,
    #[prop(optional_no_strip)] csrf_token: Option<String>,
) -> impl IntoView {
    #[cfg(feature = "browser-js")]
    {
        let config = browser_adapter_config_json(intent_endpoint, csrf_token)
            .unwrap_or_else(|_| "{}".to_string());
        let source = page_builder_browser_module_source(&config, FLY_BROWSER_ADAPTER_JS);
        view! {
            <script
                type="module"
                data-fly-browser-adapter=PAGE_BUILDER_BROWSER_ADAPTER
                inner_html=source
            ></script>
        }
        .into_any()
    }

    #[cfg(not(feature = "browser-js"))]
    {
        let _ = (intent_endpoint, csrf_token);
        view! { <span hidden data-fly-browser-adapter="disabled"></span> }.into_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::{
        DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS, DEFAULT_MAX_BROWSER_MESSAGE_BYTES,
    };

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
        assert_eq!(
            value["maxMessageBytes"],
            DEFAULT_MAX_BROWSER_MESSAGE_BYTES
        );
        assert_eq!(
            value["maxGeometryComponents"],
            DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS
        );
        assert!(value.get("intent_endpoint").is_none());
        assert!(value.get("csrf_token").is_none());
    }

    #[test]
    fn framework_adapter_renders_the_shared_browser_host_contract() {
        let source = page_builder_browser_module_source("{}", FLY_BROWSER_ADAPTER_JS);
        assert_eq!(PAGE_BUILDER_BROWSER_ADAPTER, "fly_browser");
        assert!(source.contains("fly:browser-ready"));
        assert!(source.contains("data-fly-intent-form"));
        assert!(source.contains("adapter.abortController?.signal"));
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
