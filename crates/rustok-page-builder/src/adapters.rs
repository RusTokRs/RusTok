use crate::dto::{
    BuilderTreeNode, PageBuilderCapabilityRequest, PageBuilderContractMetadata,
};
use crate::transport::{PageBuilderTransportError, PageBuilderTransportSuccess};
use fly::{
    validate_project, ComponentNode, GrapesJsV1Codec, ProjectDocument, RegistrySet,
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
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_api::PortContext;

/// Decoded Fly view of a canonical `grapesjs_v1` project.
///
/// The inspection keeps the original lossless Fly document and its validation report together so
/// callers cannot accidentally validate one value and traverse another value.
#[derive(Debug, Clone, PartialEq)]
pub struct FlyProjectInspection {
    document: ProjectDocument,
    validation: ValidationReport,
}

impl FlyProjectInspection {
    /// Decode and validate a project using the generic Fly registries and default resource limits.
    pub fn decode(schema_version: &str, project_data: &Value) -> FlyProjectAdapterResult<Self> {
        Self::decode_with(
            schema_version,
            project_data,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        )
    }

    /// Decode and validate a project using caller-supplied registries and limits.
    ///
    /// Unknown component providers remain warnings and their payloads remain lossless. Structural
    /// errors such as duplicate IDs or configured resource-limit violations are retained in the
    /// report and can be rejected with [`Self::require_valid`].
    pub fn decode_with(
        schema_version: &str,
        project_data: &Value,
        registries: &RegistrySet,
        limits: ValidationLimits,
    ) -> FlyProjectAdapterResult<Self> {
        let expected = PageBuilderContractMetadata::BASELINE.contract;
        if schema_version != expected {
            return Err(FlyProjectAdapterError::UnsupportedSchema {
                expected,
                actual: schema_version.to_string(),
            });
        }

        let document = GrapesJsV1Codec::decode_value(project_data.clone())
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
                    component_to_tree(
                        component,
                        &format!("pages[{page_index}].component"),
                    )
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
    UnsupportedSchema {
        expected: &'static str,
        actual: String,
    },
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
            Self::UnsupportedSchema { expected, actual } => write!(
                formatter,
                "unsupported page-builder schema `{actual}`; expected `{expected}`"
            ),
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

/// Service decorator that makes Fly the structural validator for preview and publish calls while
/// preserving the existing transport envelopes, authorization, persistence, rendering, and rollout
/// behaviour of the wrapped service.
#[cfg(feature = "server")]
pub struct FlyValidatedPageBuilderService<S> {
    inner: S,
    registries: RegistrySet,
    limits: ValidationLimits,
}

#[cfg(feature = "server")]
impl<S> FlyValidatedPageBuilderService<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            registries: RegistrySet::with_builtins(),
            limits: ValidationLimits::default(),
        }
    }

    pub fn with_policy(inner: S, registries: RegistrySet, limits: ValidationLimits) -> Self {
        Self {
            inner,
            registries,
            limits,
        }
    }

    pub fn inner(&self) -> &S {
        &self.inner
    }

    fn validate_project(
        &self,
        schema_version: &str,
        project_data: &Value,
    ) -> Result<(), PageBuilderServiceError> {
        let inspection = FlyProjectInspection::decode_with(
            schema_version,
            project_data,
            &self.registries,
            self.limits,
        )?;
        inspection.require_valid()?;
        Ok(())
    }
}

#[cfg(feature = "server")]
#[async_trait]
impl<S> PageBuilderCapabilityService for FlyValidatedPageBuilderService<S>
where
    S: PageBuilderCapabilityService,
{
    async fn preview(
        &self,
        context: &PortContext,
        input: crate::dto::PreviewPageBuilderInput,
    ) -> crate::service::PageBuilderServiceResult<crate::dto::PreviewPageBuilderResult> {
        self.validate_project(&input.schema_version, &input.project_data)?;
        self.inner.preview(context, input).await
    }

    async fn tree(
        &self,
        context: &PortContext,
        input: crate::dto::BuilderTreeInput,
    ) -> crate::service::PageBuilderServiceResult<crate::dto::BuilderTreeResult> {
        self.inner.tree(context, input).await
    }

    async fn properties(
        &self,
        context: &PortContext,
        input: crate::dto::BuilderNodePropertiesInput,
    ) -> crate::service::PageBuilderServiceResult<crate::dto::BuilderNodePropertiesResult> {
        self.inner.properties(context, input).await
    }

    async fn publish(
        &self,
        context: &PortContext,
        input: crate::dto::PublishPageBuilderInput,
    ) -> crate::service::PageBuilderServiceResult<crate::dto::PublishPageBuilderResult> {
        self.validate_project(&input.schema_version, &input.project_data)?;
        self.inner.publish(context, input).await
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
        serde_json::from_str(include_str!(
            "../../fly/fixtures/grapesjs/baseline.json"
        ))
        .expect("baseline fixture must be valid JSON")
    }

    #[test]
    fn fly_inspection_reads_real_grapesjs_tree() {
        let inspection = FlyProjectInspection::decode("grapesjs_v1", &baseline())
            .expect("fixture must decode through Fly");
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
        let inspection = FlyProjectInspection::decode("grapesjs_v1", &baseline())
            .expect("fixture must decode through Fly");
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
        let inspection = FlyProjectInspection::decode("grapesjs_v1", &project)
            .expect("structural decode should succeed");
        let error = inspection
            .require_valid()
            .expect_err("duplicate IDs must fail validation");

        assert!(matches!(
            error,
            FlyProjectAdapterError::Validation { .. }
        ));
    }

    #[test]
    fn fly_inspection_rejects_contract_drift() {
        let error = FlyProjectInspection::decode("fly_v2", &baseline())
            .expect_err("unknown contract must fail");
        assert!(matches!(
            error,
            FlyProjectAdapterError::UnsupportedSchema { .. }
        ));
    }
}
