use crate::dto::{BuilderTreeNode, PageBuilderCapabilityRequest};
use crate::transport::{PageBuilderTransportError, PageBuilderTransportSuccess};
use fly::{
    validate_project, ComponentNode, GrapesJsCodec, ProjectDocument, RegistrySet,
    ValidationDiagnostic, ValidationLimits, ValidationReport,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "server")]
use crate::service::{
    AuthorizedPageBuilderHandlers, PageBuilderCapabilityService, PageBuilderRequestAuth,
    PageBuilderServiceError,
};
#[cfg(feature = "server")]
use crate::transport::{dispatch_graphql_envelope, dispatch_leptos_server_function_envelope};
#[cfg(feature = "server")]
use rustok_api::PortContext;

#[cfg(feature = "server")]
pub mod fly_service;
#[cfg(feature = "server")]
pub use fly_service::FlyAdapterBackedPageBuilderService;

/// Decoded Fly view of the current page-builder project.
///
/// The inspection keeps the original lossless Fly document and its validation report together so
/// callers cannot accidentally validate one value and traverse another value. External format
/// decoding stays at this adapter boundary and does not version the domain model.
#[derive(Debug, Clone, PartialEq)]
pub struct FlyProjectInspection {
    document: ProjectDocument,
    validation: ValidationReport,
}

impl FlyProjectInspection {
    pub fn decode(project_data: &Value) -> FlyProjectAdapterResult<Self> {
        Self::decode_with(
            project_data,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        )
    }

    pub fn decode_with(
        project_data: &Value,
        registries: &RegistrySet,
        limits: ValidationLimits,
    ) -> FlyProjectAdapterResult<Self> {
        let document = GrapesJsCodec::decode_value(project_data.clone())
            .map_err(|error| FlyProjectAdapterError::Decode(error.to_string()))?;
        let validation = validate_project(&document, registries, limits);
        Ok(Self {
            document,
            validation,
        })
    }

    pub fn document(&self) -> &ProjectDocument {
        &self.document
    }

    pub fn validation(&self) -> &ValidationReport {
        &self.validation
    }

    pub fn require_valid(&self) -> FlyProjectAdapterResult<()> {
        if self.validation.is_valid() {
            Ok(())
        } else {
            Err(FlyProjectAdapterError::Validation {
                diagnostics: self.validation.errors().cloned().collect(),
            })
        }
    }

    /// Convert the actual GrapesJS hierarchy (`pages[].component.components`) to the stable
    /// page-builder tree DTO. Opaque text nodes remain in the project document but are omitted from
    /// the structural layers tree.
    pub fn tree_nodes(&self) -> Vec<BuilderTreeNode> {
        self.document
            .project
            .pages
            .iter()
            .enumerate()
            .filter_map(|(page_index, page)| {
                page.component.as_ref().and_then(|component| {
                    component_to_tree(component, &format!("pages[{page_index}].component"))
                })
            })
            .collect()
    }

    /// Return the lossless serialized properties for one component, excluding its child list so a
    /// property panel cannot accidentally replace the component subtree.
    pub fn component_properties(&self, component_id: &str) -> FlyProjectAdapterResult<Value> {
        let component = self
            .document
            .component(component_id)
            .ok_or_else(|| FlyProjectAdapterError::ComponentNotFound(component_id.to_string()))?;
        let mut value = serde_json::to_value(component)
            .map_err(|error| FlyProjectAdapterError::Encode(error.to_string()))?;
        if let Some(object) = value.as_object_mut() {
            object.remove("components");
        }
        Ok(value)
    }

    pub fn project_hash(&self) -> String {
        self.document.hash().hex()
    }
}

fn component_to_tree(component: &ComponentNode, path: &str) -> Option<BuilderTreeNode> {
    let object = component.as_object()?;
    let id = object
        .id()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("fly:{path}"));
    let label = object
        .extensions
        .get("label")
        .or_else(|| object.extensions.get("name"))
        .and_then(Value::as_str)
        .or_else(|| object.attributes.get("aria-label").and_then(Value::as_str))
        .or(object.tag_name.as_deref())
        .unwrap_or_else(|| object.component_type())
        .to_string();
    let children = object
        .children()
        .iter()
        .enumerate()
        .filter_map(|(index, child)| {
            component_to_tree(child, &format!("{path}.components[{index}]"))
        })
        .collect();

    Some(BuilderTreeNode {
        id,
        label,
        children,
    })
}

pub type FlyProjectAdapterResult<T> = Result<T, FlyProjectAdapterError>;

#[derive(Debug, Clone, PartialEq)]
pub enum FlyProjectAdapterError {
    Decode(String),
    Encode(String),
    Validation {
        diagnostics: Vec<ValidationDiagnostic>,
    },
    ComponentNotFound(String),
}

impl std::fmt::Display for FlyProjectAdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(message) => write!(formatter, "Fly project decode failed: {message}"),
            Self::Encode(message) => write!(formatter, "Fly project encode failed: {message}"),
            Self::Validation { diagnostics } => {
                write!(formatter, "Fly project validation failed")?;
                for diagnostic in diagnostics {
                    write!(
                        formatter,
                        "; {} at {}: {}",
                        diagnostic.code, diagnostic.path, diagnostic.message
                    )?;
                }
                Ok(())
            }
            Self::ComponentNotFound(id) => {
                write!(formatter, "Fly component `{id}` was not found")
            }
        }
    }
}

impl std::error::Error for FlyProjectAdapterError {}

#[cfg(feature = "server")]
impl From<FlyProjectAdapterError> for PageBuilderServiceError {
    fn from(value: FlyProjectAdapterError) -> Self {
        PageBuilderServiceError::Validation(value.to_string())
    }
}

/// Framework-neutral GraphQL endpoint payload for the page-builder capability bridge.
///
/// Host GraphQL schemas should expose this shape (or a one-to-one generated equivalent) and
/// delegate execution to [`handle_page_builder_graphql_endpoint`] instead of calling provider
/// services directly. Keeping the endpoint request tagged by `PageBuilderCapabilityRequest`
/// prevents GraphQL-local aliases for capability names or payload variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderGraphqlEndpointInput {
    pub request: PageBuilderCapabilityRequest,
}

/// Framework-neutral Leptos `#[server]` endpoint payload for the page-builder capability bridge.
///
/// The actual Leptos server function wrapper is expected to deserialize this payload and call
/// [`handle_page_builder_leptos_server_function_endpoint`], preserving the same canonical
/// request/response envelope used by GraphQL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderLeptosServerFunctionInput {
    pub request: PageBuilderCapabilityRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderEndpointSuccess {
    pub envelope: PageBuilderTransportSuccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBuilderEndpointError {
    pub envelope: PageBuilderTransportError,
}

pub type PageBuilderEndpointResult = Result<PageBuilderEndpointSuccess, PageBuilderEndpointError>;

impl From<PageBuilderTransportSuccess> for PageBuilderEndpointSuccess {
    fn from(envelope: PageBuilderTransportSuccess) -> Self {
        Self { envelope }
    }
}

impl From<PageBuilderTransportError> for PageBuilderEndpointError {
    fn from(envelope: PageBuilderTransportError) -> Self {
        Self { envelope }
    }
}

/// Canonical GraphQL endpoint handler seam.
#[cfg(feature = "server")]
pub async fn handle_page_builder_graphql_endpoint<S>(
    handlers: &AuthorizedPageBuilderHandlers<S>,
    context: &PortContext,
    auth: &PageBuilderRequestAuth,
    input: PageBuilderGraphqlEndpointInput,
) -> PageBuilderEndpointResult
where
    S: PageBuilderCapabilityService,
{
    dispatch_graphql_envelope(handlers, context, auth, input.request)
        .await
        .map(PageBuilderEndpointSuccess::from)
        .map_err(PageBuilderEndpointError::from)
}

/// Canonical Leptos `#[server]` endpoint handler seam.
#[cfg(feature = "server")]
pub async fn handle_page_builder_leptos_server_function_endpoint<S>(
    handlers: &AuthorizedPageBuilderHandlers<S>,
    context: &PortContext,
    auth: &PageBuilderRequestAuth,
    input: PageBuilderLeptosServerFunctionInput,
) -> PageBuilderEndpointResult
where
    S: PageBuilderCapabilityService,
{
    dispatch_leptos_server_function_envelope(handlers, context, auth, input.request)
        .await
        .map(PageBuilderEndpointSuccess::from)
        .map_err(PageBuilderEndpointError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn baseline() -> Value {
        serde_json::from_str(include_str!("../../fly/fixtures/grapesjs/baseline.json"))
            .expect("baseline fixture must be valid JSON")
    }

    #[test]
    fn fly_inspection_reads_real_grapesjs_tree() {
        let inspection =
            FlyProjectInspection::decode(&baseline()).expect("fixture must decode through Fly");
        inspection.require_valid().expect("fixture must be valid");

        let tree = inspection.tree_nodes();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].id, "root");
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].id, "hero");
        assert_eq!(tree[0].children[0].children[0].id, "hero-title");
    }

    #[test]
    fn fly_inspection_exposes_properties_without_children() {
        let inspection =
            FlyProjectInspection::decode(&baseline()).expect("fixture must decode through Fly");
        let properties = inspection
            .component_properties("hero")
            .expect("hero component must exist");

        assert_eq!(properties["type"], "section");
        assert_eq!(properties["attributes"]["class"], "rtk-hero");
        assert!(properties.get("components").is_none());
    }

    #[test]
    fn fly_inspection_rejects_duplicate_ids() {
        let project = json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        {"id": "duplicate", "type": "section"},
                        {"id": "duplicate", "type": "section"}
                    ]
                }
            }]
        });
        let inspection =
            FlyProjectInspection::decode(&project).expect("structural decode should succeed");
        let error = inspection
            .require_valid()
            .expect_err("duplicate IDs must fail validation");

        assert!(matches!(error, FlyProjectAdapterError::Validation { .. }));
    }
}
