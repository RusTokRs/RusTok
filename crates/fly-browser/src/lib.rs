//! SSR-first browser adapter distribution for Fly.
//!
//! This crate deliberately contains no `wasm-bindgen`, `web-sys`, Leptos, or DOM dependency.
//! Server-rendered hosts can embed the JavaScript asset and keep project state, commands,
//! validation, persistence, and HTML rendering in Rust.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FLY_BROWSER_PROTOCOL: &str = "fly_iframe";
pub const FLY_BROWSER_ADAPTER: &str = "fly_browser";
pub const DEFAULT_MAX_BROWSER_MESSAGE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS: usize = 4096;
pub const DEFAULT_MAX_PENDING_INTENT_REQUESTS: usize = 8;
pub const DEFAULT_INTENT_REQUEST_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_BROWSER_RESOURCE_LIMIT_MESSAGE: &str =
    "Editor canvas resource limit reached.";
pub const DEFAULT_PENDING_INTENT_LIMIT_MESSAGE: &str = "Editor action limit reached.";
pub const DEFAULT_INTENT_REQUEST_TIMEOUT_MESSAGE: &str = "Editor action timed out";
pub const FLY_BROWSER_ADAPTER_JS: &str = include_str!("../assets/fly-browser.js");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BrowserIntentKind {
    Select,
    FocusRequested,
    Hover,
    HoverRequested,
    ActivatePage,
    CancelDrag,
    CancelDragRequested,
    BeginPaletteDrag,
    BeginSelectedMove,
    DragMoved,
    Drop,
    DropRequested,
    InsertBlock,
    RemoveSelected,
    MoveSelected,
    MoveSelectedUp,
    MoveSelectedDown,
    PatchSelected,
    PatchComponentProperty,
    PatchPageMetadata,
    CreatePage,
    RenamePage,
    RemovePage,
    UpsertTranslation,
    RemoveTranslation,
    SetLocalePolicy,
    ClearLocalePolicy,
    UpsertLocalizedPageMetadata,
    SetInternalPageLink,
    RemoveInternalPageLink,
    SetComponentAction,
    RemoveComponentAction,
    SetComponentForm,
    RemoveComponentForm,
    SetNativeFormField,
    UpsertAsset,
    RemoveAsset,
    SelectAsset,
    SetRuntimeContext,
    SetRuntimeLocale,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Duplicate,
    KeyStroke,
    Save,
}

impl BrowserIntentKind {
    pub const ALL: [Self; 48] = [
        Self::Select,
        Self::FocusRequested,
        Self::Hover,
        Self::HoverRequested,
        Self::ActivatePage,
        Self::CancelDrag,
        Self::CancelDragRequested,
        Self::BeginPaletteDrag,
        Self::BeginSelectedMove,
        Self::DragMoved,
        Self::Drop,
        Self::DropRequested,
        Self::InsertBlock,
        Self::RemoveSelected,
        Self::MoveSelected,
        Self::MoveSelectedUp,
        Self::MoveSelectedDown,
        Self::PatchSelected,
        Self::PatchComponentProperty,
        Self::PatchPageMetadata,
        Self::CreatePage,
        Self::RenamePage,
        Self::RemovePage,
        Self::UpsertTranslation,
        Self::RemoveTranslation,
        Self::SetLocalePolicy,
        Self::ClearLocalePolicy,
        Self::UpsertLocalizedPageMetadata,
        Self::SetInternalPageLink,
        Self::RemoveInternalPageLink,
        Self::SetComponentAction,
        Self::RemoveComponentAction,
        Self::SetComponentForm,
        Self::RemoveComponentForm,
        Self::SetNativeFormField,
        Self::UpsertAsset,
        Self::RemoveAsset,
        Self::SelectAsset,
        Self::SetRuntimeContext,
        Self::SetRuntimeLocale,
        Self::Undo,
        Self::Redo,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Duplicate,
        Self::KeyStroke,
        Self::Save,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value.trim().to_ascii_lowercase())
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::FocusRequested => "focus_requested",
            Self::Hover => "hover",
            Self::HoverRequested => "hover_requested",
            Self::ActivatePage => "activate_page",
            Self::CancelDrag => "cancel_drag",
            Self::CancelDragRequested => "cancel_drag_requested",
            Self::BeginPaletteDrag => "begin_palette_drag",
            Self::BeginSelectedMove => "begin_selected_move",
            Self::DragMoved => "drag_moved",
            Self::Drop => "drop",
            Self::DropRequested => "drop_requested",
            Self::InsertBlock => "insert_block",
            Self::RemoveSelected => "remove_selected",
            Self::MoveSelected => "move_selected",
            Self::MoveSelectedUp => "move_selected_up",
            Self::MoveSelectedDown => "move_selected_down",
            Self::PatchSelected => "patch_selected",
            Self::PatchComponentProperty => "patch_component_property",
            Self::PatchPageMetadata => "patch_page_metadata",
            Self::CreatePage => "create_page",
            Self::RenamePage => "rename_page",
            Self::RemovePage => "remove_page",
            Self::UpsertTranslation => "upsert_translation",
            Self::RemoveTranslation => "remove_translation",
            Self::SetLocalePolicy => "set_locale_policy",
            Self::ClearLocalePolicy => "clear_locale_policy",
            Self::UpsertLocalizedPageMetadata => "upsert_localized_page_metadata",
            Self::SetInternalPageLink => "set_internal_page_link",
            Self::RemoveInternalPageLink => "remove_internal_page_link",
            Self::SetComponentAction => "set_component_action",
            Self::RemoveComponentAction => "remove_component_action",
            Self::SetComponentForm => "set_component_form",
            Self::RemoveComponentForm => "remove_component_form",
            Self::SetNativeFormField => "set_native_form_field",
            Self::UpsertAsset => "upsert_asset",
            Self::RemoveAsset => "remove_asset",
            Self::SelectAsset => "select_asset",
            Self::SetRuntimeContext => "set_runtime_context",
            Self::SetRuntimeLocale => "set_runtime_locale",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Copy => "copy",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::Duplicate => "duplicate",
            Self::KeyStroke => "key_stroke",
            Self::Save => "save",
        }
    }

    pub const fn is_mutating(self) -> bool {
        matches!(
            self,
            Self::Drop
                | Self::DropRequested
                | Self::InsertBlock
                | Self::RemoveSelected
                | Self::MoveSelected
                | Self::MoveSelectedUp
                | Self::MoveSelectedDown
                | Self::PatchSelected
                | Self::PatchComponentProperty
                | Self::PatchPageMetadata
                | Self::CreatePage
                | Self::RenamePage
                | Self::RemovePage
                | Self::UpsertTranslation
                | Self::RemoveTranslation
                | Self::SetLocalePolicy
                | Self::ClearLocalePolicy
                | Self::UpsertLocalizedPageMetadata
                | Self::SetInternalPageLink
                | Self::RemoveInternalPageLink
                | Self::SetComponentAction
                | Self::RemoveComponentAction
                | Self::SetComponentForm
                | Self::RemoveComponentForm
                | Self::SetNativeFormField
                | Self::UpsertAsset
                | Self::RemoveAsset
                | Self::SelectAsset
                | Self::SetRuntimeContext
                | Self::SetRuntimeLocale
                | Self::Undo
                | Self::Redo
                | Self::Cut
                | Self::Paste
                | Self::Duplicate
                | Self::KeyStroke
                | Self::Save
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    #[serde(default = "default_max_browser_message_bytes")]
    pub max_message_bytes: usize,
    #[serde(default = "default_max_browser_geometry_components")]
    pub max_geometry_components: usize,
    #[serde(default = "default_max_pending_intent_requests")]
    pub max_pending_intent_requests: usize,
    #[serde(default = "default_intent_request_timeout_ms")]
    pub intent_request_timeout_ms: u64,
    #[serde(default = "default_browser_resource_limit_message")]
    pub resource_limit_message: String,
    #[serde(default = "default_pending_intent_limit_message")]
    pub pending_intent_limit_message: String,
    #[serde(default = "default_intent_request_timeout_message")]
    pub intent_request_timeout_message: String,
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
            max_message_bytes: DEFAULT_MAX_BROWSER_MESSAGE_BYTES,
            max_geometry_components: DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS,
            max_pending_intent_requests: DEFAULT_MAX_PENDING_INTENT_REQUESTS,
            intent_request_timeout_ms: DEFAULT_INTENT_REQUEST_TIMEOUT_MS,
            resource_limit_message: default_browser_resource_limit_message(),
            pending_intent_limit_message: default_pending_intent_limit_message(),
            intent_request_timeout_message: default_intent_request_timeout_message(),
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
        if self.max_message_bytes == 0 {
            self.max_message_bytes = DEFAULT_MAX_BROWSER_MESSAGE_BYTES;
        }
        if self.max_geometry_components == 0 {
            self.max_geometry_components = DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS;
        }
        if self.max_pending_intent_requests == 0 {
            self.max_pending_intent_requests = DEFAULT_MAX_PENDING_INTENT_REQUESTS;
        }
        if self.intent_request_timeout_ms == 0 {
            self.intent_request_timeout_ms = DEFAULT_INTENT_REQUEST_TIMEOUT_MS;
        }
        self.resource_limit_message = non_empty(
            self.resource_limit_message,
            default_browser_resource_limit_message(),
        );
        self.pending_intent_limit_message = non_empty(
            self.pending_intent_limit_message,
            default_pending_intent_limit_message(),
        );
        self.intent_request_timeout_message = non_empty(
            self.intent_request_timeout_message,
            default_intent_request_timeout_message(),
        );
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.clone().normalized())
    }
}

/// Normalized request posted by the standalone JavaScript bridge to a consumer-owned SSR endpoint.
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
        if self.protocol != FLY_BROWSER_PROTOCOL {
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

    pub fn kind(&self) -> Option<BrowserIntentKind> {
        BrowserIntentKind::parse(&self.intent)
    }

    pub fn is_mutating(&self) -> bool {
        self.kind().is_some_and(BrowserIntentKind::is_mutating)
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
                "unsupported Fly browser protocol `{protocol}`; expected `{FLY_BROWSER_PROTOCOL}`"
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
    FLY_BROWSER_PROTOCOL.to_string()
}

const fn default_true() -> bool {
    true
}

const fn default_max_browser_message_bytes() -> usize {
    DEFAULT_MAX_BROWSER_MESSAGE_BYTES
}

const fn default_max_browser_geometry_components() -> usize {
    DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS
}

const fn default_max_pending_intent_requests() -> usize {
    DEFAULT_MAX_PENDING_INTENT_REQUESTS
}

const fn default_intent_request_timeout_ms() -> u64 {
    DEFAULT_INTENT_REQUEST_TIMEOUT_MS
}

fn default_browser_resource_limit_message() -> String {
    DEFAULT_BROWSER_RESOURCE_LIMIT_MESSAGE.to_string()
}

fn default_pending_intent_limit_message() -> String {
    DEFAULT_PENDING_INTENT_LIMIT_MESSAGE.to_string()
}

fn default_intent_request_timeout_message() -> String {
    DEFAULT_INTENT_REQUEST_TIMEOUT_MESSAGE.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn defaults_are_ssr_host_friendly() {
        let config = BrowserAdapterConfig::default();
        assert_eq!(config.expected_origin, "null");
        assert!(config.root_selector.contains("data-fly-browser-root"));
        assert!(config.iframe_selector.contains("data-fly-iframe-canvas"));
        assert!(config.intent_endpoint.is_none());
        assert_eq!(config.max_message_bytes, DEFAULT_MAX_BROWSER_MESSAGE_BYTES);
        assert_eq!(
            config.max_geometry_components,
            DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS
        );
        assert_eq!(
            config.max_pending_intent_requests,
            DEFAULT_MAX_PENDING_INTENT_REQUESTS
        );
        assert_eq!(
            config.intent_request_timeout_ms,
            DEFAULT_INTENT_REQUEST_TIMEOUT_MS
        );
    }

    #[test]
    fn normalization_restores_safe_limits_and_messages() {
        let config = BrowserAdapterConfig {
            root_selector: "  ".to_string(),
            iframe_selector: " iframe ".to_string(),
            expected_origin: " null ".to_string(),
            intent_endpoint: Some("  /admin/fly/intents  ".to_string()),
            csrf_token: Some("   ".to_string()),
            max_message_bytes: 0,
            max_geometry_components: 0,
            max_pending_intent_requests: 0,
            intent_request_timeout_ms: 0,
            resource_limit_message: "   ".to_string(),
            pending_intent_limit_message: "   ".to_string(),
            intent_request_timeout_message: "   ".to_string(),
            ..BrowserAdapterConfig::default()
        }
        .normalized();
        assert_eq!(config.root_selector, "[data-fly-browser-root]");
        assert_eq!(config.iframe_selector, "iframe");
        assert_eq!(
            config.intent_endpoint.as_deref(),
            Some("/admin/fly/intents")
        );
        assert_eq!(config.csrf_token, None);
        assert_eq!(config.max_message_bytes, DEFAULT_MAX_BROWSER_MESSAGE_BYTES);
        assert_eq!(
            config.max_geometry_components,
            DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS
        );
        assert_eq!(
            config.max_pending_intent_requests,
            DEFAULT_MAX_PENDING_INTENT_REQUESTS
        );
        assert_eq!(
            config.intent_request_timeout_ms,
            DEFAULT_INTENT_REQUEST_TIMEOUT_MS
        );
        assert_eq!(
            config.resource_limit_message,
            DEFAULT_BROWSER_RESOURCE_LIMIT_MESSAGE
        );
        assert_eq!(
            config.pending_intent_limit_message,
            DEFAULT_PENDING_INTENT_LIMIT_MESSAGE
        );
        assert_eq!(
            config.intent_request_timeout_message,
            DEFAULT_INTENT_REQUEST_TIMEOUT_MESSAGE
        );
    }

    #[test]
    fn browser_config_uses_only_camel_case() {
        let json = BrowserAdapterConfig {
            intent_endpoint: Some("/admin/fly/intents".to_string()),
            csrf_token: Some("csrf-token".to_string()),
            max_message_bytes: 2048,
            max_geometry_components: 32,
            max_pending_intent_requests: 3,
            intent_request_timeout_ms: 1_500,
            pending_intent_limit_message: "Pending limit".to_string(),
            intent_request_timeout_message: "Request timeout".to_string(),
            ..BrowserAdapterConfig::default()
        }
        .to_json()
        .expect("browser config");
        let value: Value = serde_json::from_str(&json).expect("JSON");
        assert_eq!(value["intentEndpoint"], "/admin/fly/intents");
        assert_eq!(value["csrfToken"], "csrf-token");
        assert_eq!(value["maxMessageBytes"], 2048);
        assert_eq!(value["maxGeometryComponents"], 32);
        assert_eq!(value["maxPendingIntentRequests"], 3);
        assert_eq!(value["intentRequestTimeoutMs"], 1_500);
        assert_eq!(value["pendingIntentLimitMessage"], "Pending limit");
        assert_eq!(value["intentRequestTimeoutMessage"], "Request timeout");
        assert!(serde_json::from_value::<BrowserAdapterConfig>(json!({
            "root_selector": "#unsupported"
        }))
        .is_err());
    }

    #[test]
    fn public_bundle_is_single_current_runtime() {
        assert!(FLY_BROWSER_ADAPTER_JS.contains("class FlyBrowserAdapter"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-resource-limit"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("PENDING_INTENT_LIMIT"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("INTENT_REQUEST_TIMEOUT"));
        assert!(FLY_BROWSER_ADAPTER_JS.contains("pendingIntentRequests = new Map()"));
        assert!(!FLY_BROWSER_ADAPTER_JS.contains("Adapter.prototype"));
        assert!(!FLY_BROWSER_ADAPTER_JS.contains("__flyResourceGuardInstalled"));
        assert!(!FLY_BROWSER_ADAPTER_JS.contains("wasm_bindgen"));
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
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
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
    fn intent_kind_names_are_unique_and_round_trip() {
        let mut names = BTreeSet::new();
        for kind in BrowserIntentKind::ALL {
            assert!(names.insert(kind.as_str()), "{}", kind.as_str());
            assert_eq!(BrowserIntentKind::parse(kind.as_str()), Some(kind));
            assert_eq!(
                serde_json::to_value(kind).expect("serialize kind"),
                Value::String(kind.as_str().to_string())
            );
        }
        assert_eq!(names.len(), BrowserIntentKind::ALL.len());
    }

    #[test]
    fn envelope_uses_typed_kind_without_rejecting_extensions() {
        let known = BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
            instance_id: "canvas-a".to_string(),
            intent: "  RENAME_PAGE ".to_string(),
            payload: json!({}),
            sequence: None,
            page_id: None,
            revision: None,
            project_hash: None,
            draft_token: None,
            draft_generation: None,
        }
        .normalized()
        .expect("known intent");
        assert_eq!(known.kind(), Some(BrowserIntentKind::RenamePage));
        assert!(known.is_mutating());

        let extension = BrowserIntentEnvelope {
            intent: "plugin_custom_preview".to_string(),
            ..known
        };
        assert_eq!(extension.kind(), None);
        assert!(!extension.is_mutating());
    }
}
