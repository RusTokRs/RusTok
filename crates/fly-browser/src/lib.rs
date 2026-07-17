//! SSR-first browser adapter distribution for Fly.
//!
//! This crate deliberately contains no `wasm-bindgen`, `web-sys`, Leptos, or DOM dependency.
//! Server-rendered hosts can embed the JavaScript asset and keep project state, commands,
//! validation, persistence, and HTML rendering in Rust.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FLY_BROWSER_PROTOCOL: &str = "fly_iframe";
pub const FLY_BROWSER_ADAPTER: &str = "fly_browser";
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
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "select" => Self::Select,
            "focus_requested" => Self::FocusRequested,
            "hover" => Self::Hover,
            "hover_requested" => Self::HoverRequested,
            "activate_page" => Self::ActivatePage,
            "cancel_drag" => Self::CancelDrag,
            "cancel_drag_requested" => Self::CancelDragRequested,
            "begin_palette_drag" => Self::BeginPaletteDrag,
            "begin_selected_move" => Self::BeginSelectedMove,
            "drag_moved" => Self::DragMoved,
            "drop" => Self::Drop,
            "drop_requested" => Self::DropRequested,
            "insert_block" => Self::InsertBlock,
            "remove_selected" => Self::RemoveSelected,
            "move_selected" => Self::MoveSelected,
            "move_selected_up" => Self::MoveSelectedUp,
            "move_selected_down" => Self::MoveSelectedDown,
            "patch_selected" => Self::PatchSelected,
            "patch_component_property" => Self::PatchComponentProperty,
            "patch_page_metadata" => Self::PatchPageMetadata,
            "create_page" => Self::CreatePage,
            "rename_page" => Self::RenamePage,
            "remove_page" => Self::RemovePage,
            "upsert_translation" => Self::UpsertTranslation,
            "remove_translation" => Self::RemoveTranslation,
            "set_locale_policy" => Self::SetLocalePolicy,
            "clear_locale_policy" => Self::ClearLocalePolicy,
            "upsert_localized_page_metadata" => Self::UpsertLocalizedPageMetadata,
            "set_internal_page_link" => Self::SetInternalPageLink,
            "remove_internal_page_link" => Self::RemoveInternalPageLink,
            "set_component_action" => Self::SetComponentAction,
            "remove_component_action" => Self::RemoveComponentAction,
            "set_component_form" => Self::SetComponentForm,
            "remove_component_form" => Self::RemoveComponentForm,
            "set_native_form_field" => Self::SetNativeFormField,
            "upsert_asset" => Self::UpsertAsset,
            "remove_asset" => Self::RemoveAsset,
            "select_asset" => Self::SelectAsset,
            "set_runtime_context" => Self::SetRuntimeContext,
            "set_runtime_locale" => Self::SetRuntimeLocale,
            "undo" => Self::Undo,
            "redo" => Self::Redo,
            "copy" => Self::Copy,
            "cut" => Self::Cut,
            "paste" => Self::Paste,
            "duplicate" => Self::Duplicate,
            "key_stroke" => Self::KeyStroke,
            "save" => Self::Save,
            _ => return None,
        })
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

fn default_true() -> bool {
    true
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
        assert_eq!(
            config.intent_endpoint.as_deref(),
            Some("/admin/fly/intents")
        );
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
    fn command_producing_and_draft_intents_are_mutating() {
        for kind in [
            BrowserIntentKind::InsertBlock,
            BrowserIntentKind::Drop,
            BrowserIntentKind::DropRequested,
            BrowserIntentKind::RemoveSelected,
            BrowserIntentKind::MoveSelectedUp,
            BrowserIntentKind::MoveSelectedDown,
            BrowserIntentKind::PatchComponentProperty,
            BrowserIntentKind::PatchPageMetadata,
            BrowserIntentKind::CreatePage,
            BrowserIntentKind::RenamePage,
            BrowserIntentKind::RemovePage,
            BrowserIntentKind::UpsertTranslation,
            BrowserIntentKind::RemoveTranslation,
            BrowserIntentKind::SetLocalePolicy,
            BrowserIntentKind::ClearLocalePolicy,
            BrowserIntentKind::UpsertLocalizedPageMetadata,
            BrowserIntentKind::SetInternalPageLink,
            BrowserIntentKind::RemoveInternalPageLink,
            BrowserIntentKind::SetComponentAction,
            BrowserIntentKind::RemoveComponentAction,
            BrowserIntentKind::SetComponentForm,
            BrowserIntentKind::RemoveComponentForm,
            BrowserIntentKind::SetNativeFormField,
            BrowserIntentKind::UpsertAsset,
            BrowserIntentKind::RemoveAsset,
            BrowserIntentKind::SelectAsset,
            BrowserIntentKind::SetRuntimeContext,
            BrowserIntentKind::SetRuntimeLocale,
            BrowserIntentKind::Undo,
            BrowserIntentKind::Redo,
            BrowserIntentKind::Cut,
            BrowserIntentKind::Paste,
            BrowserIntentKind::Duplicate,
            BrowserIntentKind::KeyStroke,
            BrowserIntentKind::Save,
        ] {
            assert!(kind.is_mutating(), "{}", kind.as_str());
        }
        for kind in [
            BrowserIntentKind::Select,
            BrowserIntentKind::FocusRequested,
            BrowserIntentKind::Hover,
            BrowserIntentKind::HoverRequested,
            BrowserIntentKind::ActivatePage,
            BrowserIntentKind::CancelDrag,
            BrowserIntentKind::CancelDragRequested,
            BrowserIntentKind::BeginPaletteDrag,
            BrowserIntentKind::BeginSelectedMove,
            BrowserIntentKind::DragMoved,
            BrowserIntentKind::Copy,
        ] {
            assert!(!kind.is_mutating(), "{}", kind.as_str());
        }
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
