use crate::dto::{BuilderTreeNode, PageBuilderCapabilityRequest};
use crate::transport::{PageBuilderTransportError, PageBuilderTransportSuccess};
use fly::{
    ComponentNode, ComponentObject, GrapesJsCodec, ProjectDocument, RegistrySet, ValidationDiagnostic,
    ValidationLimits, ValidationReport, validate_project,
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
        let document = GrapesJsCodec::decode_ref(project_data)
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
            .project()
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
    /// Anonymous components can be addressed by the snapshot-local path IDs returned by
    /// `tree_nodes`; those IDs are not persistent component identities.
    pub fn component_properties(&self, component_id: &str) -> FlyProjectAdapterResult<Value> {
        let component = self
            .document
            .component(component_id)
            .or_else(|| anonymous_component_at_tree_id(&self.document, component_id))
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

// The brackets in this namespace cannot occur in a valid explicit Fly component ID.
// Resolve only canonical paths emitted by component_to_tree, without treating a path
// as an alternative ID for an already identified component or for an opaque node.
fn anonymous_component_at_tree_id<'a>(
    document: &'a ProjectDocument,
    tree_id: &str,
) -> Option<&'a ComponentObject> {
    fn index_and_tail(value: &str) -> Option<(usize, &str)> {
        let (digits, tail) = value.split_once(']')?;
        let index = digits.parse::<usize>().ok()?;
        (index.to_string() == digits).then_some((index, tail))
    }

    let (page_index, tail) = index_and_tail(tree_id.strip_prefix("fly:pages[")?)?;
    let mut tail = tail.strip_prefix(".component")?;
    let mut component = document
        .project()
        .pages
        .get(page_index)?
        .component
        .as_ref()?
        .as_object()?;
    while !tail.is_empty() {
        let (index, rest) = index_and_tail(tail.strip_prefix(".components[")?)?;
        component = component.children().get(index)?.as_object()?;
        tail = rest;
    }
    component.id().is_none().then_some(component)
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
        serde_json::from_str(include_str!(
            "../../../ui/fly/fixtures/grapesjs/baseline.json"
        ))
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
    fn tree_ids_resolve_anonymous_components_without_aliasing_named_or_opaque_nodes() {
        let project = json!({
            "pages": [
                {
                    "id": "home",
                    "component": {
                        "type": "wrapper",
                        "components": [
                            "opaque text",
                            {
                                "type": "section",
                                "attributes": { "class": "anonymous" },
                                "components": [{ "id": "title", "type": "text" }]
                            }
                        ]
                    }
                },
                {
                    "id": "other",
                    "component": { "type": "wrapper" }
                }
            ]
        });
        let inspection = FlyProjectInspection::decode(&project).expect("decode");
        inspection.require_valid().expect("anonymous nodes are allowed");
        let tree = inspection.tree_nodes();
        assert_eq!(tree[0].id, "fly:pages[0].component");
        let section_id = &tree[0].children[0].id;
        assert_eq!(section_id, "fly:pages[0].component.components[1]");
        let properties = inspection.component_properties(section_id).expect("section");
        assert_eq!(properties["attributes"]["class"], "anonymous");
        assert!(properties.get("components").is_none());
        assert!(properties.get("id").is_none());
        for root in &tree {
            assert_eq!(
                inspection.component_properties(&root.id).expect("root")["type"],
                "wrapper"
            );
        }
        assert_eq!(inspection.component_properties("title").expect("named")["type"], "text");
        for invalid in [
            "fly:pages[0].component.components[0]",
            "fly:pages[0].component.components[1].components[0]",
            "fly:pages[0].component.components[2]",
            "fly:pages[00].component",
            "fly:pages[9].component",
            "fly:pages[0].component.extra",
        ] {
            assert!(matches!(
                inspection.component_properties(invalid),
                Err(FlyProjectAdapterError::ComponentNotFound(_))
            ), "unexpectedly resolved {invalid}");
        }
    }

    #[test]
    fn fly_inspection_rejects_excessive_json_depth() {
        let mut nested = Value::Null;
        for _ in 0..256 {
            nested = Value::Array(vec![nested]);
        }
        let project = Value::Object(serde_json::Map::from_iter([
            ("payload".to_string(), nested),
        ]));
        assert!(matches!(
            FlyProjectInspection::decode(&project),
            Err(FlyProjectAdapterError::Decode(_))
        ));
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
