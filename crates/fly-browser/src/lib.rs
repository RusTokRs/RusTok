//! SSR-first browser adapter distribution for Fly.
//!
//! This crate deliberately contains no `wasm-bindgen`, `web-sys`, Leptos, or DOM dependency.
//! Server-rendered hosts can embed the JavaScript asset and keep project state, commands,
//! validation, persistence, and HTML rendering in Rust.

use serde::{Deserialize, Serialize};

pub const FLY_BROWSER_PROTOCOL_V1: &str = "fly_iframe_v1";
pub const FLY_BROWSER_ADAPTER_VERSION: &str = "fly_browser_v1";
pub const FLY_BROWSER_ADAPTER_JS: &str = include_str!("../assets/fly-browser.js");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserAdapterConfig {
    #[serde(default = "default_root_selector")]
    pub root_selector: String,
    #[serde(default = "default_iframe_selector")]
    pub iframe_selector: String,
    #[serde(default = "default_expected_origin")]
    pub expected_origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
    #[serde(default = "default_true")]
    pub auto_mount: bool,
    #[serde(default = "default_true")]
    pub draw_overlays: bool,
    #[serde(default = "default_true")]
    pub post_intents: bool,
}

impl Default for BrowserAdapterConfig {
    fn default() -> Self {
        Self {
            root_selector: default_root_selector(),
            iframe_selector: default_iframe_selector(),
            expected_origin: default_expected_origin(),
            intent_endpoint: None,
            csrf_token: None,
            auto_mount: true,
            draw_overlays: true,
            post_intents: true,
        }
    }
}

impl BrowserAdapterConfig {
    pub fn normalized(mut self) -> Self {
        self.root_selector = non_empty(self.root_selector, default_root_selector());
        self.iframe_selector = non_empty(self.iframe_selector, default_iframe_selector());
        self.expected_origin = non_empty(self.expected_origin, default_expected_origin());
        self.intent_endpoint = normalize_optional(self.intent_endpoint);
        self.csrf_token = normalize_optional(self.csrf_token);
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.clone().normalized())
    }
}

fn non_empty(value: String, fallback: String) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback
    } else {
        value.to_string()
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn default_root_selector() -> String {
    "[data-fly-browser-root]".to_string()
}

fn default_iframe_selector() -> String {
    "iframe[data-fly-iframe-canvas]".to_string()
}

fn default_expected_origin() -> String {
    "null".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_ssr_host_friendly() {
        let config = BrowserAdapterConfig::default();
        assert_eq!(config.expected_origin, "null");
        assert!(config.root_selector.contains("data-fly-browser-root"));
        assert!(config.iframe_selector.contains("data-fly-iframe-canvas"));
        assert!(config.intent_endpoint.is_none());
    }

    #[test]
    fn normalization_removes_empty_optional_values() {
        let config = BrowserAdapterConfig {
            root_selector: "  ".to_string(),
            iframe_selector: " iframe ".to_string(),
            expected_origin: " null ".to_string(),
            intent_endpoint: Some("  /admin/fly/intents  ".to_string()),
            csrf_token: Some("   ".to_string()),
            ..BrowserAdapterConfig::default()
        }
        .normalized();
        assert_eq!(config.root_selector, "[data-fly-browser-root]");
        assert_eq!(config.iframe_selector, "iframe");
        assert_eq!(config.intent_endpoint.as_deref(), Some("/admin/fly/intents"));
        assert_eq!(config.csrf_token, None);
    }
}
