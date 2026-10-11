use crate::{ProjectDocument, ValidationDiagnostic};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const FLY_ACTION_FIELD: &str = "flyAction";
pub const FLY_FORM_FIELD: &str = "flyForm";
pub const FLY_ACTION_DATA_ATTRIBUTE: &str = "data-fly-action";
pub const FLY_ACTION_KIND_ATTRIBUTE: &str = "data-fly-action-kind";

pub(super) const GENERATED_INTERACTION_ATTRIBUTES: &[&str] = &[
    "href",
    "target",
    "rel",
    "type",
    "form",
    "action",
    "method",
    "enctype",
    "novalidate",
    FLY_ACTION_DATA_ATTRIBUTE,
    FLY_ACTION_KIND_ATTRIBUTE,
    "data-fly-form-provider",
    "data-fly-form-action",
    "data-fly-form-input",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComponentAction {
    NavigatePage {
        page_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback_href: Option<String>,
    },
    NavigateUrl {
        href: String,
        #[serde(default)]
        new_window: bool,
    },
    SubmitForm {
        form_id: String,
    },
    EmitEvent {
        event: String,
        #[serde(default)]
        payload: Value,
    },
    ProviderAction {
        provider: String,
        action: String,
        #[serde(default)]
        input: Value,
    },
}

impl ComponentAction {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::NavigatePage { .. } => "navigate_page",
            Self::NavigateUrl { .. } => "navigate_url",
            Self::SubmitForm { .. } => "submit_form",
            Self::EmitEvent { .. } => "emit_event",
            Self::ProviderAction { .. } => "provider_action",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormMethod {
    #[default]
    Get,
    Post,
    Dialog,
}

impl FormMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Dialog => "dialog",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FormEncoding {
    #[default]
    UrlEncoded,
    Multipart,
    TextPlain,
}

impl FormEncoding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UrlEncoded => "application/x-www-form-urlencoded",
            Self::Multipart => "multipart/form-data",
            Self::TextPlain => "text/plain",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentForm {
    pub id: String,
    #[serde(default)]
    pub method: FormMethod,
    #[serde(default)]
    pub encoding: FormEncoding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub novalidate: bool,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

/// Resolves the native submission endpoint a host serves for a provider form.
///
/// A provider form materializes as a `data-fly-form-provider` marker without a native target, so
/// it posts back to the page URL unless the host answers where its provider runs. Callers that
/// render for a provider host pass a resolver to
/// [`materialize_component_actions_with_providers`](crate::materialize_component_actions_with_providers)
/// or [`render_page_with_runtime_context_with_providers`](crate::render_page_with_runtime_context_with_providers);
/// every recognized `provider.operation` then receives a native `action` (POST, URL-encoded),
/// while unregistered providers keep the marker-only rendering and the document is never
/// rewritten.
pub trait FormProviderEndpointResolver: Send + Sync {
    /// Returns the intake URL for `provider.operation` bound to a Fly `form_id`, or `None` when
    /// the host does not serve that provider interaction.
    fn form_endpoint(&self, provider: &str, operation: &str, form_id: &str) -> Option<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionMaterialization {
    pub document: ProjectDocument,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub materialized_forms: usize,
    pub native_actions: usize,
    pub custom_actions: usize,
    pub fallback_actions: usize,
    pub unresolved_actions: usize,
}
