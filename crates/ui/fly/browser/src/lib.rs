//! SSR-first browser adapter distribution for Fly.
//!
//! This crate deliberately contains no WASM bindings, web platform APIs, Leptos, or DOM dependency.
//! Server-rendered hosts can embed the JavaScript asset and keep project state, commands,
//! validation, persistence, and HTML rendering in Rust.

// The entire Fly stack is safe Rust. `forbid` (not `deny`) so it cannot be re-enabled
// locally with an `allow` attribute.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FLY_BROWSER_PROTOCOL: &str = "fly_iframe";
pub const FLY_BROWSER_ADAPTER: &str = "fly_browser";
pub const DEFAULT_MAX_BROWSER_MESSAGE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_MAX_BROWSER_GEOMETRY_COMPONENTS: usize = 4096;
pub const DEFAULT_MAX_PENDING_INTENT_REQUESTS: usize = 8;
pub const DEFAULT_INTENT_REQUEST_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_BROWSER_RESOURCE_LIMIT_MESSAGE: &str = "Editor canvas resource limit reached.";
pub const DEFAULT_PENDING_INTENT_LIMIT_MESSAGE: &str = "Editor action limit reached.";
pub const DEFAULT_INTENT_REQUEST_TIMEOUT_MESSAGE: &str = "Editor action timed out";
pub const FLY_BROWSER_ADAPTER_JS: &str = include_str!("../assets/fly-browser.js");

/// Canonical header name used by Fly browser adapters to transmit CSRF tokens for intent requests.
pub const FLY_CSRF_HEADER_NAME: &str = "x-csrf-token";

/// Standalone action runtime for published pages (see `assets/fly-actions.js`). Hosts serve
/// or inline this asset on public pages that materialize `data-fly-action` attributes; the
/// editor bridge above never loads it.
pub const FLY_ACTIONS_JS: &str = include_str!("../assets/fly-actions.js");

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
    MovePage,
    UpsertStyleRule,
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
    SetThemeToken,
    RemoveThemeToken,
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
    pub const ALL: [Self; 52] = [
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
        Self::MovePage,
        Self::UpsertStyleRule,
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
        Self::SetThemeToken,
        Self::RemoveThemeToken,
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
            Self::MovePage => "move_page",
            Self::UpsertStyleRule => "upsert_style_rule",
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
            Self::SetThemeToken => "set_theme_token",
            Self::RemoveThemeToken => "remove_theme_token",
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
                | Self::MovePage
                | Self::UpsertStyleRule
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
                | Self::SetThemeToken
                | Self::RemoveThemeToken
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
    /// Origin that inbound canvas messages must match, or `None` when the host configured none.
    ///
    /// There is deliberately no default. The field used to default to `"null"` — the opaque
    /// origin of a sandboxed `srcdoc` iframe — so a host that never configured an origin silently
    /// accepted the sandboxed canvas rather than refusing to mount. The JavaScript bridge now
    /// fails closed on an unset origin, and a host whose canvas really is a sandboxed `srcdoc`
    /// iframe passes `"null"` explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_origin: Option<String>,
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
            expected_origin: None,
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
        self.expected_origin = normalize_optional(self.expected_origin);
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

    /// Builds an envelope for one intent. Adapter workspaces use the typed
    /// constructors below so the intent names and payload shapes they emit match the
    /// server dispatch contract by construction instead of by string convention.
    pub fn new(instance_id: impl Into<String>, kind: BrowserIntentKind, payload: Value) -> Self {
        Self {
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
            instance_id: instance_id.into(),
            intent: kind.as_str().to_string(),
            payload,
            sequence: None,
            page_id: None,
            revision: None,
            project_hash: None,
            draft_token: None,
            draft_generation: None,
        }
    }

    /// Page activation by index (and id when the page carries one), matching the
    /// server dispatch payload (`page_index`, optional `page_id`).
    pub fn activate_page(
        instance_id: impl Into<String>,
        page_index: usize,
        page_id: Option<&str>,
    ) -> Self {
        let mut payload = serde_json::json!({ "page_index": page_index });
        if let Some(page_id) = page_id {
            payload["page_id"] = serde_json::Value::String(page_id.to_string());
        }
        Self::new(instance_id, BrowserIntentKind::ActivatePage, payload)
    }

    /// One property edit on a component, in the `patch_component_property`
    /// contract the SSR inspector forms use: `kind` is `field`, `attribute`,
    /// or `style`; `remove` clears instead of setting.
    pub fn patch_component_property(
        instance_id: impl Into<String>,
        component_id: &str,
        kind: &str,
        name: &str,
        value: &str,
        remove: bool,
    ) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::PatchComponentProperty,
            serde_json::json!({
                "component_id": component_id,
                "kind": kind,
                "name": name,
                "value": value,
                "remove": remove,
            }),
        )
    }

    /// Document asset upsert in the `upsert_asset` SSR-form contract: source is
    /// required, id and name optional.
    pub fn upsert_asset(
        instance_id: impl Into<String>,
        asset_id: Option<&str>,
        source: &str,
        name: Option<&str>,
    ) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::UpsertAsset,
            serde_json::json!({
                "asset_id": asset_id.unwrap_or_default(),
                "source": source,
                "name": name.unwrap_or_default(),
            }),
        )
    }

    /// Document asset removal by id.
    pub fn remove_asset(instance_id: impl Into<String>, asset_id: &str) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::RemoveAsset,
            serde_json::json!({ "asset_id": asset_id }),
        )
    }

    /// Apply a document asset to a component attribute (`src` unless the host
    /// integration chose another attribute), in the `select_asset` contract.
    pub fn select_asset(
        instance_id: impl Into<String>,
        component_id: &str,
        asset_id: &str,
        source_attribute: &str,
    ) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::SelectAsset,
            serde_json::json!({
                "component_id": component_id,
                "asset_id": asset_id,
                "source_attribute": source_attribute,
            }),
        )
    }

    /// Page creation with a display name; the server assigns the unique id,
    /// appends the page, and activates it.
    pub fn create_page(instance_id: impl Into<String>, name: &str) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::CreatePage,
            serde_json::json!({ "name": name }),
        )
    }

    /// Page removal by index (and id when the page carries one).
    pub fn remove_page(
        instance_id: impl Into<String>,
        page_index: usize,
        page_id: Option<&str>,
    ) -> Self {
        let mut payload = serde_json::json!({ "page_index": page_index });
        if let Some(page_id) = page_id {
            payload["page_id"] = serde_json::Value::String(page_id.to_string());
        }
        Self::new(instance_id, BrowserIntentKind::RemovePage, payload)
    }

    /// Page identity patch: display name and, when non-empty, the page id.
    pub fn rename_page(
        instance_id: impl Into<String>,
        page_index: usize,
        page_id: Option<&str>,
        name: &str,
        new_page_id: Option<&str>,
    ) -> Self {
        let mut payload = serde_json::json!({ "page_index": page_index, "name": name });
        if let Some(page_id) = page_id {
            payload["page_id"] = serde_json::Value::String(page_id.to_string());
        }
        if let Some(new_page_id) = new_page_id {
            payload["new_page_id"] = serde_json::Value::String(new_page_id.to_string());
        }
        Self::new(instance_id, BrowserIntentKind::RenamePage, payload)
    }

    /// Full page-properties submit as one atomic rename intent: identity
    /// (name and, when non-empty, the new id) plus the serialized
    /// `fly::PageMetadata` projection, applied by the server as a single page
    /// patch — one command, one revision bump, one error slot.
    pub fn page_properties(
        instance_id: impl Into<String>,
        page_index: usize,
        page_id: Option<&str>,
        name: &str,
        new_page_id: Option<&str>,
        metadata: serde_json::Value,
    ) -> Self {
        let mut payload = serde_json::json!({
            "page_index": page_index,
            "name": name,
            "metadata": metadata,
        });
        if let Some(page_id) = page_id {
            payload["page_id"] = serde_json::Value::String(page_id.to_string());
        }
        if let Some(new_page_id) = new_page_id {
            payload["new_page_id"] = serde_json::Value::String(new_page_id.to_string());
        }
        Self::new(instance_id, BrowserIntentKind::RenamePage, payload)
    }

    /// SEO / Open Graph metadata patch for one page; `metadata` is the serialized
    /// `fly::PageMetadata` projection.
    pub fn patch_page_metadata(
        instance_id: impl Into<String>,
        page_index: usize,
        page_id: Option<&str>,
        metadata: serde_json::Value,
    ) -> Self {
        let mut payload = serde_json::json!({ "page_index": page_index, "metadata": metadata });
        if let Some(page_id) = page_id {
            payload["page_id"] = serde_json::Value::String(page_id.to_string());
        }
        Self::new(instance_id, BrowserIntentKind::PatchPageMetadata, payload)
    }

    /// Page reorder: move the page at `page_index` so it lands at `target_index`.
    pub fn move_page(
        instance_id: impl Into<String>,
        page_index: usize,
        page_id: Option<&str>,
        target_index: usize,
    ) -> Self {
        let mut payload = serde_json::json!({
            "page_index": page_index,
            "target_index": target_index,
        });
        if let Some(page_id) = page_id {
            payload["page_id"] = serde_json::Value::String(page_id.to_string());
        }
        Self::new(instance_id, BrowserIntentKind::MovePage, payload)
    }

    /// One scoped style declaration on a component: `scope_kind` is `media`
    /// (with the media query) or `state` (with the pseudo-class), matching the
    /// responsive / interaction-state style contract.
    pub fn upsert_style_rule(
        instance_id: impl Into<String>,
        component_id: &str,
        scope_kind: &str,
        scope_value: &str,
        property: &str,
        value: &str,
        remove: bool,
    ) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::UpsertStyleRule,
            serde_json::json!({
                "component_id": component_id,
                "scope_kind": scope_kind,
                "scope_value": scope_value,
                "property": property,
                "value": value,
                "remove": remove,
            }),
        )
    }

    /// Selection change; `None` clears the selection.
    pub fn select(instance_id: impl Into<String>, component_id: Option<&str>) -> Self {
        let payload = match component_id {
            Some(component_id) => serde_json::json!({ "component_id": component_id }),
            None => serde_json::json!({}),
        };
        Self::new(instance_id, BrowserIntentKind::Select, payload)
    }

    /// Palette insert of one registry block.
    pub fn insert_block(instance_id: impl Into<String>, block_id: &str) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::InsertBlock,
            serde_json::json!({ "block_id": block_id }),
        )
    }

    /// Inserts or updates one design token by name.
    pub fn set_theme_token(instance_id: impl Into<String>, name: &str, value: &str) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::SetThemeToken,
            serde_json::json!({ "name": name, "value": value }),
        )
    }

    /// Removes one design token by name.
    pub fn remove_theme_token(instance_id: impl Into<String>, name: &str) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::RemoveThemeToken,
            serde_json::json!({ "name": name }),
        )
    }

    pub fn undo(instance_id: impl Into<String>) -> Self {
        Self::new(instance_id, BrowserIntentKind::Undo, serde_json::json!({}))
    }

    pub fn redo(instance_id: impl Into<String>) -> Self {
        Self::new(instance_id, BrowserIntentKind::Redo, serde_json::json!({}))
    }

    pub fn remove_selected(instance_id: impl Into<String>) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::RemoveSelected,
            serde_json::json!({}),
        )
    }

    pub fn duplicate(instance_id: impl Into<String>) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::Duplicate,
            serde_json::json!({}),
        )
    }

    pub fn copy(instance_id: impl Into<String>) -> Self {
        Self::new(instance_id, BrowserIntentKind::Copy, serde_json::json!({}))
    }

    pub fn cut(instance_id: impl Into<String>) -> Self {
        Self::new(instance_id, BrowserIntentKind::Cut, serde_json::json!({}))
    }

    pub fn paste(instance_id: impl Into<String>) -> Self {
        Self::new(instance_id, BrowserIntentKind::Paste, serde_json::json!({}))
    }

    pub fn move_selected_up(instance_id: impl Into<String>) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::MoveSelectedUp,
            serde_json::json!({}),
        )
    }

    pub fn move_selected_down(instance_id: impl Into<String>) -> Self {
        Self::new(
            instance_id,
            BrowserIntentKind::MoveSelectedDown,
            serde_json::json!({}),
        )
    }

    /// Attaches the page/revision context the server uses for revision checks
    /// (lost-update protection) on mutating intents.
    pub fn with_page_context(
        mut self,
        page_id: Option<String>,
        revision: Option<String>,
        project_hash: Option<String>,
    ) -> Self {
        self.page_id = page_id;
        self.revision = revision;
        self.project_hash = project_hash;
        self
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

/// Deterministically derives a secret CSRF token from an authenticated session token.
///
/// An ambient session cookie (e.g. `SameSite=Lax`) is vulnerable to cross-origin forgery if
/// state-changing routes do not require an explicit credential that only same-origin JavaScript
/// can read and provide. Because cross-origin scripts cannot read the session cookie under SOP,
/// a SHA-256 derivation of the session token is unguessable by an attacker.
pub fn derive_csrf_token(session_token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"fly-intent-csrf-v1:");
    hasher.update(session_token.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Constant-time verification of a client-supplied CSRF token against the expected session token.
pub fn verify_csrf_token(session_token: &str, candidate_csrf_token: &str) -> bool {
    if session_token.trim().is_empty() || candidate_csrf_token.trim().is_empty() {
        return false;
    }
    let expected = derive_csrf_token(session_token);
    constant_time_eq(expected.as_bytes(), candidate_csrf_token.as_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn defaults_are_ssr_host_friendly() {
        let config = BrowserAdapterConfig::default();
        // No origin default: the JavaScript bridge refuses to mount until a host states one.
        assert_eq!(config.expected_origin, None);
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
            expected_origin: Some(" null ".to_string()),
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
        assert_eq!(config.expected_origin.as_deref(), Some("null"));
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
        assert!(!FLY_BROWSER_ADAPTER_JS.contains(&format!("{}_{}", "wasm", "bindgen")));
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
    fn typed_constructors_match_the_server_dispatch_contract() {
        let insert = BrowserIntentEnvelope::insert_block("workspace-1", "shop.product_card")
            .normalized()
            .expect("insert envelope");
        assert_eq!(insert.kind(), Some(BrowserIntentKind::InsertBlock));
        assert_eq!(insert.payload["block_id"], "shop.product_card");
        assert!(insert.is_mutating());

        let select = BrowserIntentEnvelope::select("workspace-1", Some("hero"))
            .normalized()
            .expect("select envelope");
        assert_eq!(select.kind(), Some(BrowserIntentKind::Select));
        assert_eq!(select.payload["component_id"], "hero");

        let clear = BrowserIntentEnvelope::select("workspace-1", None)
            .normalized()
            .expect("clear envelope");
        assert!(clear.payload.get("component_id").is_none());

        let set_token = BrowserIntentEnvelope::set_theme_token("workspace-1", "brand", "#0b5fff")
            .normalized()
            .expect("set token envelope");
        assert_eq!(set_token.kind(), Some(BrowserIntentKind::SetThemeToken));
        assert_eq!(set_token.payload["name"], "brand");
        assert_eq!(set_token.payload["value"], "#0b5fff");
        assert!(set_token.is_mutating());

        let remove_token = BrowserIntentEnvelope::remove_theme_token("workspace-1", "brand")
            .normalized()
            .expect("remove token envelope");
        assert_eq!(
            remove_token.kind(),
            Some(BrowserIntentKind::RemoveThemeToken)
        );
        assert_eq!(remove_token.payload["name"], "brand");
        assert!(remove_token.is_mutating());

        for envelope in [
            BrowserIntentEnvelope::undo("workspace-1"),
            BrowserIntentEnvelope::redo("workspace-1"),
        ] {
            let envelope = envelope.normalized().expect("history envelope");
            assert!(envelope.payload.is_object());
        }

        let activate = BrowserIntentEnvelope::activate_page("workspace-1", 2, Some("about"))
            .normalized()
            .expect("activate envelope");
        assert_eq!(activate.kind(), Some(BrowserIntentKind::ActivatePage));
        assert_eq!(activate.payload["page_index"], 2);
        assert_eq!(activate.payload["page_id"], "about");

        let create = BrowserIntentEnvelope::create_page("workspace-1", "About us")
            .normalized()
            .expect("create envelope");
        assert_eq!(create.kind(), Some(BrowserIntentKind::CreatePage));
        assert_eq!(create.payload["name"], "About us");

        let remove = BrowserIntentEnvelope::remove_page("workspace-1", 3, Some("legacy"))
            .normalized()
            .expect("remove envelope");
        assert_eq!(remove.kind(), Some(BrowserIntentKind::RemovePage));
        assert_eq!(remove.payload["page_index"], 3);
        assert_eq!(remove.payload["page_id"], "legacy");

        let rename = BrowserIntentEnvelope::rename_page(
            "workspace-1",
            1,
            Some("old-id"),
            "Renamed",
            Some("new-id"),
        )
        .normalized()
        .expect("rename envelope");
        assert_eq!(rename.kind(), Some(BrowserIntentKind::RenamePage));
        assert_eq!(rename.payload["name"], "Renamed");
        assert_eq!(rename.payload["new_page_id"], "new-id");
        assert!(rename.is_mutating());

        let upsert = BrowserIntentEnvelope::upsert_asset(
            "workspace-1",
            Some("hero-img"),
            "https://cdn.example/hero.png",
            None,
        )
        .normalized()
        .expect("upsert asset envelope");
        assert_eq!(upsert.kind(), Some(BrowserIntentKind::UpsertAsset));
        assert_eq!(upsert.payload["asset_id"], "hero-img");
        assert!(upsert.is_mutating());

        let removed_asset = BrowserIntentEnvelope::remove_asset("workspace-1", "hero-img")
            .normalized()
            .expect("remove asset envelope");
        assert_eq!(removed_asset.kind(), Some(BrowserIntentKind::RemoveAsset));

        let applied = BrowserIntentEnvelope::select_asset("workspace-1", "hero", "hero-img", "src")
            .normalized()
            .expect("select asset envelope");
        assert_eq!(applied.kind(), Some(BrowserIntentKind::SelectAsset));
        assert_eq!(applied.payload["source_attribute"], "src");

        let styled = BrowserIntentEnvelope::upsert_style_rule(
            "workspace-1",
            "hero",
            "media",
            "(max-width: 767px)",
            "padding",
            "8px",
            false,
        )
        .normalized()
        .expect("style rule envelope");
        assert_eq!(styled.kind(), Some(BrowserIntentKind::UpsertStyleRule));
        assert_eq!(styled.payload["scope_kind"], "media");
        assert_eq!(styled.payload["property"], "padding");
        assert!(styled.is_mutating());

        let property = BrowserIntentEnvelope::patch_component_property(
            "workspace-1",
            "hero",
            "attribute",
            "aria-label",
            "Hero section",
            false,
        )
        .normalized()
        .expect("property envelope");
        assert_eq!(
            property.kind(),
            Some(BrowserIntentKind::PatchComponentProperty)
        );
        assert_eq!(property.payload["component_id"], "hero");
        assert_eq!(property.payload["kind"], "attribute");
        assert_eq!(property.payload["remove"], false);
        assert!(property.is_mutating());

        let properties = BrowserIntentEnvelope::page_properties(
            "workspace-1",
            1,
            Some("old-id"),
            "Renamed",
            Some("new-id"),
            serde_json::json!({ "slug": "renamed" }),
        )
        .normalized()
        .expect("properties envelope");
        assert_eq!(properties.kind(), Some(BrowserIntentKind::RenamePage));
        assert_eq!(properties.payload["metadata"]["slug"], "renamed");
        assert_eq!(properties.payload["new_page_id"], "new-id");

        let metadata_patch = BrowserIntentEnvelope::patch_page_metadata(
            "workspace-1",
            1,
            Some("about"),
            serde_json::json!({ "slug": "about" }),
        )
        .normalized()
        .expect("metadata envelope");
        assert_eq!(
            metadata_patch.kind(),
            Some(BrowserIntentKind::PatchPageMetadata)
        );
        assert_eq!(metadata_patch.payload["metadata"]["slug"], "about");

        let moved = BrowserIntentEnvelope::move_page("workspace-1", 1, None, 3)
            .normalized()
            .expect("move envelope");
        assert_eq!(moved.kind(), Some(BrowserIntentKind::MovePage));
        assert_eq!(moved.payload["target_index"], 3);
        assert!(moved.payload.get("page_id").is_none());

        let activate_anonymous = BrowserIntentEnvelope::activate_page("workspace-1", 0, None)
            .normalized()
            .expect("activate anonymous envelope");
        assert!(activate_anonymous.payload.get("page_id").is_none());

        let contextual = BrowserIntentEnvelope::undo("workspace-1")
            .with_page_context(
                Some("home".to_string()),
                Some("rev-2".to_string()),
                Some("abc".to_string()),
            )
            .normalized()
            .expect("contextual envelope");
        assert_eq!(contextual.page_id.as_deref(), Some("home"));
        assert_eq!(contextual.revision.as_deref(), Some("rev-2"));
        assert_eq!(contextual.project_hash.as_deref(), Some("abc"));
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
            BrowserIntentKind::SetInternalPageLink,
            BrowserIntentKind::RemoveInternalPageLink,
            BrowserIntentKind::SetComponentAction,
            BrowserIntentKind::RemoveComponentAction,
            BrowserIntentKind::SetComponentForm,
            BrowserIntentKind::RemoveComponentForm,
            BrowserIntentKind::SetNativeFormField,
            BrowserIntentKind::UpsertTranslation,
            BrowserIntentKind::RemoveTranslation,
            BrowserIntentKind::Save,
        ] {
            assert!(
                kind.is_mutating(),
                "{} must be revision protected",
                kind.as_str()
            );
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

    #[test]
    fn csrf_token_derivation_and_verification() {
        let session = "test-session-secret-token-12345";
        let csrf = derive_csrf_token(session);
        assert!(!csrf.is_empty());
        assert_eq!(csrf.len(), 64);
        assert!(verify_csrf_token(session, &csrf));

        // Mismatched token
        assert!(!verify_csrf_token(session, "invalid-csrf-token"));
        assert!(!verify_csrf_token("different-session", &csrf));

        // Empty cases
        assert!(!verify_csrf_token("", &csrf));
        assert!(!verify_csrf_token(session, ""));
        assert!(!verify_csrf_token("   ", &csrf));
    }
}
