//! SSR-first browser adapter distribution for Fly.
//!
//! This crate deliberately contains no `wasm-bindgen`, `web-sys`, Leptos, or DOM dependency.
//! Server-rendered hosts can embed the JavaScript asset and keep project state, commands,
//! validation, persistence, and HTML rendering in Rust.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Normalized request posted by the standalone JavaScript bridge to a consumer-owned SSR endpoint.
///
/// The endpoint is intentionally transport-neutral. A host may expose it through Axum, Actix,
/// a Leptos server function, or its existing REST facade, then load the consumer-owned draft and
/// apply the intent through the Fly engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserIntentEnvelope {
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub instance_id: String,
    pub intent: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_generation: Option<u64>,
}

impl BrowserIntentEnvelope {
    pub fn normalized(mut self) -> Result<Self, BrowserIntentError> {
        self.protocol = self.protocol.trim().to_string();
        self.instance_id = self.instance_id.trim().to_string();
        self.intent = self.intent.trim().to_ascii_lowercase();
        self.page_id = normalize_optional(self.page_id);
        self.revision = normalize_optional(self.revision);
        self.project_hash = normalize_optional(self.project_hash);
        self.draft_token = normalize_optional(self.draft_token);
        if self.protocol != FLY_BROWSER_PROTOCOL_V1 {
            return Err(BrowserIntentError::Protocol(self.protocol));
        }
        if self.instance_id.is_empty() {
            return Err(BrowserIntentError::MissingInstanceId);
        }
        if self.intent.is_empty() {
            return Err(BrowserIntentError::MissingIntent);
        }
        Ok(self)
    }

    pub fn is_mutating(&self) -> bool {
        matches!(
            self.intent.as_str(),
            "insert_block"
                | "drop"
                | "drop_requested"
                | "remove_selected"
                | "move_selected"
                | "move_selected_up"
                | "move_selected_down"
                | "patch_selected"
                | "patch_component_property"
                | "patch_page_metadata"
                | "create_page"
                | "rename_page"
                | "remove_page"
                | "upsert_translation"
                | "remove_translation"
                | "set_locale_policy"
                | "clear_locale_policy"
                | "set_internal_page_link"
                | "remove_internal_page_link"
                | "set_component_action"
                | "remove_component_action"
                | "set_component_form"
                | "remove_component_form"
                | "set_native_form_field"
                | "set_runtime_context"
                | "set_runtime_locale"
                | "undo"
                | "redo"
                | "cut"
                | "paste"
                | "duplicate"
                | "key_stroke"
                | "save"
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserIntentError {
    Protocol(String),
    MissingInstanceId,
    MissingIntent,
}

impl std::fmt::Display for BrowserIntentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Protocol(protocol) => write!(
                formatter,
                "unsupported Fly browser protocol `{protocol}`; expected `{FLY_BROWSER_PROTOCOL_V1}`"
            ),
            Self::MissingInstanceId => formatter.write_str("Fly browser instance id is required"),
            Self::MissingIntent => formatter.write_str("Fly browser intent is required"),
        }
    }
}

impl std::error::Error for BrowserIntentError {}

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

fn default_protocol() -> String {
    FLY_BROWSER_PROTOCOL_V1.to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn browser_intent_rejects_cross_protocol_requests() {
        let error = BrowserIntentEnvelope {
            protocol: "future".to_string(),
            instance_id: "canvas-a".to_string(),
            intent: "select".to_string(),
            payload: json!({}),
            sequence: Some(1),
            page_id: None,
            revision: None,
            project_hash: None,
            draft_token: None,
            draft_generation: None,
        }
        .normalized()
        .expect_err("protocol mismatch");
        assert!(matches!(error, BrowserIntentError::Protocol(_)));
    }

    #[test]
    fn draft_token_is_normalized_without_becoming_project_state() {
        let request = BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
            instance_id: "canvas-a".to_string(),
            intent: "copy".to_string(),
            payload: json!({}),
            sequence: None,
            page_id: Some("home".to_string()),
            revision: Some("rev-1".to_string()),
            project_hash: Some("abc".to_string()),
            draft_token: Some("  token  ".to_string()),
            draft_generation: Some(4),
        }
        .normalized()
        .expect("normalized");
        assert_eq!(request.draft_token.as_deref(), Some("token"));
        assert_eq!(request.draft_generation, Some(4));
    }

    #[test]
    fn command_producing_and_draft_intents_are_mutating() {
        for intent in [
            "insert_block",
            "drop",
            "drop_requested",
            "remove_selected",
            "move_selected_up",
            "move_selected_down",
            "patch_component_property",
            "patch_page_metadata",
            "create_page",
            "rename_page",
            "remove_page",
            "upsert_translation",
            "remove_translation",
            "set_locale_policy",
            "clear_locale_policy",
            "set_internal_page_link",
            "remove_internal_page_link",
            "set_component_action",
            "remove_component_action",
            "set_component_form",
            "remove_component_form",
            "set_native_form_field",
            "set_runtime_context",
            "set_runtime_locale",
            "undo",
            "redo",
            "cut",
            "paste",
            "duplicate",
            "key_stroke",
            "save",
        ] {
            let request = BrowserIntentEnvelope {
                protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
                instance_id: "canvas-a".to_string(),
                intent: intent.to_string(),
                payload: json!({}),
                sequence: None,
                page_id: Some("home".to_string()),
                revision: Some("rev-1".to_string()),
                project_hash: Some("abc".to_string()),
                draft_token: None,
                draft_generation: None,
            };
            assert!(request.is_mutating(), "{intent}");
        }
        let selection = BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
            instance_id: "canvas-a".to_string(),
            intent: "select".to_string(),
            payload: json!({}),
            sequence: None,
            page_id: None,
            revision: None,
            project_hash: None,
            draft_token: None,
            draft_generation: None,
        };
        assert!(!selection.is_mutating());
    }
}
