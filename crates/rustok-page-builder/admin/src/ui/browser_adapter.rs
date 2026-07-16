use fly_browser::{BrowserAdapterConfig, FLY_BROWSER_ADAPTER_JS};
use leptos::prelude::*;

/// Emits the standalone Fly browser bridge into a server-rendered Page Builder surface.
///
/// The script owns only DOM identity checks, geometry overlays, pointer/keyboard forwarding,
/// and optional POST delivery to a consumer-owned intent endpoint. Fly project state, commands,
/// validation, rendering, permissions, and persistence remain in Rust.
#[component]
pub fn PageBuilderBrowserAdapter(
    #[prop(optional)] intent_endpoint: Option<String>,
    #[prop(optional)] csrf_token: Option<String>,
) -> impl IntoView {
    #[cfg(feature = "browser-js")]
    {
        let config = BrowserAdapterConfig {
            intent_endpoint,
            csrf_token,
            ..BrowserAdapterConfig::default()
        };
        let config = config
            .to_json()
            .unwrap_or_else(|_| "{}".to_string());
        let source = format!(
            "globalThis.__FLY_BROWSER_CONFIG__ = Object.freeze({config});\n{FLY_BROWSER_ADAPTER_JS}\nglobalThis.FlyBrowser?.mountAll(globalThis.__FLY_BROWSER_CONFIG__);"
        );
        view! {
            <script
                type="module"
                data-fly-browser-adapter="fly_browser_v1"
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

    #[test]
    fn adapter_asset_does_not_depend_on_wasm_runtime() {
        assert!(FLY_BROWSER_ADAPTER_JS.contains("class FlyBrowserAdapter"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-intent"));
        assert!(!FLY_BROWSER_ADAPTER_JS.contains("wasm_bindgen"));
    }
}
