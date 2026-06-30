use crate::dto::{
    BuilderCapabilityKind, BuilderNodePropertiesInput, BuilderNodePropertiesResult,
    BuilderTreeInput, BuilderTreeNode, BuilderTreeResult, PageBuilderCapabilityRequest,
    PageBuilderCapabilityResponse, PageBuilderContractMetadata, PageBuilderErrorKind,
    PreviewPageBuilderInput, PreviewPageBuilderResult, PublishPageBuilderInput,
    PublishPageBuilderResult, PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE,
};
use crate::rollout::{ensure_capability, BuilderCapabilityFlags, BuilderRolloutError};
use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortErrorKind};
use rustok_core::{Action, Permission, Resource};
use serde::Serialize;

#[async_trait]
pub trait PageBuilderCapabilityService: Send + Sync {
    async fn preview(
        &self,
        context: &PortContext,
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult>;

    async fn tree(
        &self,
        context: &PortContext,
        input: BuilderTreeInput,
    ) -> PageBuilderServiceResult<BuilderTreeResult>;

    async fn properties(
        &self,
        context: &PortContext,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult>;

    async fn publish(
        &self,
        context: &PortContext,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult>;
}

pub type PageBuilderServiceResult<T> = Result<T, PageBuilderServiceError>;

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderServiceError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("sanitize failed: {0}")]
    Sanitize(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("capability disabled: {0}")]
    CapabilityDisabled(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}

impl PageBuilderServiceError {
    pub fn kind(&self) -> PageBuilderErrorKind {
        match self {
            Self::Validation(_) => PageBuilderErrorKind::Validation,
            Self::Sanitize(_) => PageBuilderErrorKind::Sanitize,
            Self::Forbidden(_) | Self::Runtime(_) => PageBuilderErrorKind::Runtime,
            Self::CapabilityDisabled(_) => PageBuilderErrorKind::FeatureDisabled,
        }
    }

    pub fn stable_code(&self) -> Option<&'static str> {
        match self {
            Self::CapabilityDisabled(_) => Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE),
            _ => None,
        }
    }

    pub fn from_port_error(error: rustok_api::PortError) -> Self {
        match error.kind {
            PortErrorKind::Validation => Self::Validation(error.message),
            PortErrorKind::Forbidden => Self::Forbidden(error.message),
            PortErrorKind::Timeout => Self::Runtime(error.message),
            _ => Self::Runtime(error.message),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageBuilderCapabilityPermissions {
    pub preview: Permission,
    pub tree: Permission,
    pub properties: Permission,
    pub publish: Permission,
}

impl Default for PageBuilderCapabilityPermissions {
    fn default() -> Self {
        Self {
            preview: Permission::new(Resource::Pages, Action::Read),
            tree: Permission::new(Resource::Pages, Action::Read),
            properties: Permission::new(Resource::Pages, Action::Update),
            publish: Permission::new(Resource::Pages, Action::Publish),
        }
    }
}

impl PageBuilderCapabilityPermissions {
    pub fn required_for(self, capability: BuilderCapabilityKind) -> Permission {
        match capability {
            BuilderCapabilityKind::Preview => self.preview,
            BuilderCapabilityKind::Tree => self.tree,
            BuilderCapabilityKind::Properties => self.properties,
            BuilderCapabilityKind::Publish => self.publish,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderRequestAuth {
    pub permissions: Vec<Permission>,
}

impl PageBuilderRequestAuth {
    pub fn new(permissions: impl Into<Vec<Permission>>) -> Self {
        Self {
            permissions: permissions.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderCapabilityAuthorizer {
    required: PageBuilderCapabilityPermissions,
}

impl PageBuilderCapabilityAuthorizer {
    pub fn new(required: PageBuilderCapabilityPermissions) -> Self {
        Self { required }
    }

    pub fn required_permission(&self, capability: BuilderCapabilityKind) -> Permission {
        self.required.required_for(capability)
    }

    pub fn authorize(
        &self,
        auth: &PageBuilderRequestAuth,
        capability: BuilderCapabilityKind,
    ) -> PageBuilderServiceResult<()> {
        let required = self.required_permission(capability);

        if has_effective_permission(&auth.permissions, required) {
            Ok(())
        } else {
            Err(PageBuilderServiceError::Forbidden(format!(
                "{} capability requires {}",
                capability, required
            )))
        }
    }
}

impl Default for PageBuilderCapabilityAuthorizer {
    fn default() -> Self {
        Self::new(PageBuilderCapabilityPermissions::default())
    }
}

fn has_effective_permission(permissions: &[Permission], required: Permission) -> bool {
    permissions.contains(&required)
        || permissions.contains(&Permission::new(required.resource, Action::Manage))
}

impl From<BuilderRolloutError> for PageBuilderServiceError {
    fn from(value: BuilderRolloutError) -> Self {
        match value {
            BuilderRolloutError::CapabilityDisabled(capability) => {
                Self::CapabilityDisabled(capability.to_string())
            }
            BuilderRolloutError::InvalidFlagCombination(message) => Self::Validation(message),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageBuilderAdapterOperation {
    LoadProject,
    SaveProject,
    RenderPreview,
}

impl PageBuilderAdapterOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadProject => "load_project",
            Self::SaveProject => "save_project",
            Self::RenderPreview => "render_preview",
        }
    }
}

impl std::fmt::Display for PageBuilderAdapterOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageBuilderAdapterCallEvidence {
    pub module_slug: &'static str,
    pub contract: &'static str,
    pub operation: PageBuilderAdapterOperation,
    pub tenant_id: String,
    pub page_id: String,
    pub revision_id: Option<String>,
    pub correlation_id: String,
}

impl PageBuilderAdapterCallEvidence {
    pub fn load_project(context: &PortContext, page_id: impl Into<String>) -> Self {
        Self::new(
            PageBuilderAdapterOperation::LoadProject,
            context,
            page_id,
            None,
        )
    }

    pub fn save_project(
        context: &PortContext,
        page_id: impl Into<String>,
        revision_id: impl Into<String>,
    ) -> Self {
        Self::new(
            PageBuilderAdapterOperation::SaveProject,
            context,
            page_id,
            Some(revision_id.into()),
        )
    }

    pub fn render_preview(context: &PortContext, page_id: impl Into<String>) -> Self {
        Self::new(
            PageBuilderAdapterOperation::RenderPreview,
            context,
            page_id,
            None,
        )
    }

    fn new(
        operation: PageBuilderAdapterOperation,
        context: &PortContext,
        page_id: impl Into<String>,
        revision_id: Option<String>,
    ) -> Self {
        Self {
            module_slug: PageBuilderContractMetadata::BASELINE.module_slug,
            contract: PageBuilderContractMetadata::BASELINE.contract,
            operation,
            tenant_id: context.tenant_id.clone(),
            page_id: page_id.into(),
            revision_id,
            correlation_id: context.correlation_id.clone(),
        }
    }
}

pub trait PageBuilderAdapterTelemetry: Send + Sync {
    fn record_adapter_call(&self, evidence: &PageBuilderAdapterCallEvidence);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopPageBuilderAdapterTelemetry;

impl PageBuilderAdapterTelemetry for NoopPageBuilderAdapterTelemetry {
    fn record_adapter_call(&self, _evidence: &PageBuilderAdapterCallEvidence) {}
}

/// Minimal persistence seam for hosts that store `grapesjs_v1` project snapshots outside
/// the reference provider. Implementations must keep tenant isolation in the supplied
/// [`PortContext`] and must not change the canonical DTO/envelope contract.
#[async_trait]
pub trait PageBuilderProjectStore: Send + Sync {
    async fn load_project(
        &self,
        context: &PortContext,
        page_id: &str,
    ) -> PageBuilderServiceResult<Option<serde_json::Value>>;

    async fn save_project(
        &self,
        context: &PortContext,
        page_id: &str,
        revision_id: &str,
        project_data: serde_json::Value,
    ) -> PageBuilderServiceResult<()>;
}

/// Rendering adapter seam for hosts that need production HTML/CSS rendering while keeping
/// the baseline `grapesjs_v1` validation and sanitize behaviour in this crate.
#[async_trait]
pub trait PageBuilderRenderingAdapter: Send + Sync {
    async fn render_preview(
        &self,
        context: &PortContext,
        project_data: &serde_json::Value,
    ) -> PageBuilderServiceResult<String>;
}

#[derive(Debug, Clone, Default)]
pub struct ReferencePageBuilderRenderingAdapter;

#[async_trait]
impl PageBuilderRenderingAdapter for ReferencePageBuilderRenderingAdapter {
    async fn render_preview(
        &self,
        _context: &PortContext,
        project_data: &serde_json::Value,
    ) -> PageBuilderServiceResult<String> {
        render_preview_html(project_data)
    }
}

pub struct AdapterBackedPageBuilderService<S, R, T = NoopPageBuilderAdapterTelemetry> {
    store: S,
    renderer: R,
    telemetry: T,
}

impl<S, R> AdapterBackedPageBuilderService<S, R, NoopPageBuilderAdapterTelemetry> {
    pub fn new(store: S, renderer: R) -> Self {
        Self {
            store,
            renderer,
            telemetry: NoopPageBuilderAdapterTelemetry,
        }
    }
}

impl<S, R, T> AdapterBackedPageBuilderService<S, R, T> {
    pub fn with_telemetry(store: S, renderer: R, telemetry: T) -> Self {
        Self {
            store,
            renderer,
            telemetry,
        }
    }
}

#[async_trait]
impl<S, R, T> PageBuilderCapabilityService for AdapterBackedPageBuilderService<S, R, T>
where
    S: PageBuilderProjectStore,
    R: PageBuilderRenderingAdapter,
    T: PageBuilderAdapterTelemetry,
{
    async fn preview(
        &self,
        context: &PortContext,
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
        validate_grapesjs_payload(&input.page_id, &input.schema_version, &input.project_data)?;
        let evidence = PageBuilderAdapterCallEvidence::render_preview(context, &input.page_id);
        self.telemetry.record_adapter_call(&evidence);
        let html = self
            .renderer
            .render_preview(context, &input.project_data)
            .await?;

        Ok(PreviewPageBuilderResult {
            page_id: input.page_id,
            html,
        })
    }

    async fn tree(
        &self,
        context: &PortContext,
        input: BuilderTreeInput,
    ) -> PageBuilderServiceResult<BuilderTreeResult> {
        validate_non_empty("page_id", &input.page_id)?;
        let evidence = PageBuilderAdapterCallEvidence::load_project(context, &input.page_id);
        self.telemetry.record_adapter_call(&evidence);
        let nodes = match self.store.load_project(context, &input.page_id).await? {
            Some(project_data) => extract_tree_nodes(&project_data),
            None => Vec::new(),
        };

        Ok(BuilderTreeResult {
            page_id: input.page_id,
            nodes,
        })
    }

    async fn properties(
        &self,
        _context: &PortContext,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
        validate_non_empty("page_id", &input.page_id)?;
        validate_non_empty("node_id", &input.node_id)?;
        ensure_object_payload("properties", &input.properties)?;

        Ok(BuilderNodePropertiesResult {
            page_id: input.page_id,
            node_id: input.node_id,
            properties: input.properties,
        })
    }

    async fn publish(
        &self,
        context: &PortContext,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
        validate_non_empty("revision_id", &input.revision_id)?;
        validate_grapesjs_payload(&input.page_id, &input.schema_version, &input.project_data)?;
        let evidence = PageBuilderAdapterCallEvidence::save_project(
            context,
            &input.page_id,
            &input.revision_id,
        );
        self.telemetry.record_adapter_call(&evidence);
        self.store
            .save_project(
                context,
                &input.page_id,
                &input.revision_id,
                input.project_data,
            )
            .await?;

        Ok(PublishPageBuilderResult {
            page_id: input.page_id,
            revision_id: input.revision_id,
            published: true,
        })
    }
}

fn extract_tree_nodes(project_data: &serde_json::Value) -> Vec<BuilderTreeNode> {
    project_data
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| {
                    let id = node.get("id")?.as_str()?.to_string();
                    let label = node
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(&id)
                        .to_string();
                    Some(BuilderTreeNode {
                        id,
                        label,
                        children: Vec::new(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default)]
pub struct ReferencePageBuilderService;

#[async_trait]
impl PageBuilderCapabilityService for ReferencePageBuilderService {
    async fn preview(
        &self,
        _context: &PortContext,
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
        validate_grapesjs_payload(&input.page_id, &input.schema_version, &input.project_data)?;

        Ok(PreviewPageBuilderResult {
            page_id: input.page_id,
            html: render_preview_html(&input.project_data)?,
        })
    }

    async fn tree(
        &self,
        _context: &PortContext,
        input: BuilderTreeInput,
    ) -> PageBuilderServiceResult<BuilderTreeResult> {
        validate_non_empty("page_id", &input.page_id)?;

        Ok(BuilderTreeResult {
            page_id: input.page_id,
            nodes: Vec::new(),
        })
    }

    async fn properties(
        &self,
        _context: &PortContext,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
        validate_non_empty("page_id", &input.page_id)?;
        validate_non_empty("node_id", &input.node_id)?;
        ensure_object_payload("properties", &input.properties)?;

        Ok(BuilderNodePropertiesResult {
            page_id: input.page_id,
            node_id: input.node_id,
            properties: input.properties,
        })
    }

    async fn publish(
        &self,
        _context: &PortContext,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
        validate_non_empty("revision_id", &input.revision_id)?;
        validate_grapesjs_payload(&input.page_id, &input.schema_version, &input.project_data)?;

        Ok(PublishPageBuilderResult {
            page_id: input.page_id,
            revision_id: input.revision_id,
            published: true,
        })
    }
}

fn validate_grapesjs_payload(
    page_id: &str,
    schema_version: &str,
    project_data: &serde_json::Value,
) -> PageBuilderServiceResult<()> {
    validate_non_empty("page_id", page_id)?;
    if schema_version != PageBuilderContractMetadata::BASELINE.contract {
        return Err(PageBuilderServiceError::Validation(format!(
            "schema_version must be {}",
            PageBuilderContractMetadata::BASELINE.contract
        )));
    }
    ensure_object_payload("project_data", project_data)
}

fn validate_non_empty(field: &str, value: &str) -> PageBuilderServiceResult<()> {
    if value.trim().is_empty() {
        Err(PageBuilderServiceError::Validation(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn ensure_object_payload(field: &str, value: &serde_json::Value) -> PageBuilderServiceResult<()> {
    if value.is_object() {
        Ok(())
    } else {
        Err(PageBuilderServiceError::Validation(format!(
            "{field} must be a JSON object"
        )))
    }
}

fn render_preview_html(project_data: &serde_json::Value) -> PageBuilderServiceResult<String> {
    let body = project_data
        .get("html")
        .and_then(serde_json::Value::as_str)
        .or_else(|| project_data.get("body").and_then(serde_json::Value::as_str))
        .unwrap_or("");

    if contains_script_tag(body) {
        return Err(PageBuilderServiceError::Sanitize(
            "preview html contains a forbidden script tag".to_string(),
        ));
    }

    Ok(format!(
        "<div data-rustok-page-builder=\"grapesjs_v1\">{body}</div>"
    ))
}

fn contains_script_tag(value: &str) -> bool {
    value.to_ascii_lowercase().contains("<script")
}

pub struct CapabilityGuardedService<S> {
    inner: S,
    flags: BuilderCapabilityFlags,
}

impl<S> CapabilityGuardedService<S> {
    pub fn new(inner: S, flags: BuilderCapabilityFlags) -> Self {
        Self { inner, flags }
    }
}

#[async_trait]
impl<S> PageBuilderCapabilityService for CapabilityGuardedService<S>
where
    S: PageBuilderCapabilityService,
{
    async fn preview(
        &self,
        context: &PortContext,
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Preview)?;
        self.inner.preview(context, input).await
    }

    async fn tree(
        &self,
        context: &PortContext,
        input: BuilderTreeInput,
    ) -> PageBuilderServiceResult<BuilderTreeResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Tree)?;
        self.inner.tree(context, input).await
    }

    async fn properties(
        &self,
        context: &PortContext,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Properties)?;
        self.inner.properties(context, input).await
    }

    async fn publish(
        &self,
        context: &PortContext,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Publish)?;
        context
            .require_policy(PortCallPolicy::write())
            .map_err(PageBuilderServiceError::from_port_error)?;
        self.inner.publish(context, input).await
    }
}

pub struct AuthorizedPageBuilderHandlers<S> {
    service: S,
    authorizer: PageBuilderCapabilityAuthorizer,
}

impl<S> AuthorizedPageBuilderHandlers<S> {
    pub fn new(service: S) -> Self {
        Self {
            service,
            authorizer: PageBuilderCapabilityAuthorizer::default(),
        }
    }

    pub fn with_authorizer(service: S, authorizer: PageBuilderCapabilityAuthorizer) -> Self {
        Self {
            service,
            authorizer,
        }
    }
}

impl<S> AuthorizedPageBuilderHandlers<S>
where
    S: PageBuilderCapabilityService,
{
    pub async fn preview(
        &self,
        context: &PortContext,
        auth: &PageBuilderRequestAuth,
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
        self.authorizer
            .authorize(auth, BuilderCapabilityKind::Preview)?;
        self.service.preview(context, input).await
    }

    pub async fn tree(
        &self,
        context: &PortContext,
        auth: &PageBuilderRequestAuth,
        input: BuilderTreeInput,
    ) -> PageBuilderServiceResult<BuilderTreeResult> {
        self.authorizer
            .authorize(auth, BuilderCapabilityKind::Tree)?;
        self.service.tree(context, input).await
    }

    pub async fn properties(
        &self,
        context: &PortContext,
        auth: &PageBuilderRequestAuth,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
        self.authorizer
            .authorize(auth, BuilderCapabilityKind::Properties)?;
        self.service.properties(context, input).await
    }

    pub async fn publish(
        &self,
        context: &PortContext,
        auth: &PageBuilderRequestAuth,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
        self.authorizer
            .authorize(auth, BuilderCapabilityKind::Publish)?;
        self.service.publish(context, input).await
    }

    pub async fn handle(
        &self,
        context: &PortContext,
        auth: &PageBuilderRequestAuth,
        request: PageBuilderCapabilityRequest,
    ) -> PageBuilderServiceResult<PageBuilderCapabilityResponse> {
        match request {
            PageBuilderCapabilityRequest::Preview(input) => self
                .preview(context, auth, input)
                .await
                .map(PageBuilderCapabilityResponse::Preview),
            PageBuilderCapabilityRequest::Tree(input) => self
                .tree(context, auth, input)
                .await
                .map(PageBuilderCapabilityResponse::Tree),
            PageBuilderCapabilityRequest::Properties(input) => self
                .properties(context, auth, input)
                .await
                .map(PageBuilderCapabilityResponse::Properties),
            PageBuilderCapabilityRequest::Publish(input) => self
                .publish(context, auth, input)
                .await
                .map(PageBuilderCapabilityResponse::Publish),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rollout::BuilderToggleProfile;
    use rustok_api::PortActor;

    struct StubService;

    #[async_trait]
    impl PageBuilderCapabilityService for StubService {
        async fn preview(
            &self,
            _context: &PortContext,
            input: PreviewPageBuilderInput,
        ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
            Ok(PreviewPageBuilderResult {
                page_id: input.page_id,
                html: "<div/>".to_string(),
            })
        }

        async fn tree(
            &self,
            _context: &PortContext,
            input: BuilderTreeInput,
        ) -> PageBuilderServiceResult<BuilderTreeResult> {
            Ok(BuilderTreeResult {
                page_id: input.page_id,
                nodes: vec![],
            })
        }

        async fn properties(
            &self,
            _context: &PortContext,
            input: BuilderNodePropertiesInput,
        ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
            Ok(BuilderNodePropertiesResult {
                page_id: input.page_id,
                node_id: input.node_id,
                properties: input.properties,
            })
        }

        async fn publish(
            &self,
            _context: &PortContext,
            input: PublishPageBuilderInput,
        ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
            Ok(PublishPageBuilderResult {
                page_id: input.page_id,
                revision_id: input.revision_id,
                published: true,
            })
        }
    }

    fn read_context() -> PortContext {
        PortContext::new("tenant-a", PortActor::user("editor-a"), "ru", "corr-read")
    }

    fn write_context() -> PortContext {
        PortContext::new("tenant-a", PortActor::user("editor-a"), "ru", "corr-write")
            .with_idempotency_key("idem-a")
            .with_deadline(std::time::Duration::from_secs(3))
    }

    fn preview_input() -> PreviewPageBuilderInput {
        PreviewPageBuilderInput {
            page_id: "home".to_string(),
            schema_version: "grapesjs_v1".to_string(),
            project_data: serde_json::json!({}),
        }
    }

    fn tree_input() -> BuilderTreeInput {
        BuilderTreeInput {
            page_id: "home".to_string(),
        }
    }

    fn properties_input() -> BuilderNodePropertiesInput {
        BuilderNodePropertiesInput {
            page_id: "home".to_string(),
            node_id: "hero".to_string(),
            properties: serde_json::json!({ "title": "Welcome" }),
        }
    }

    fn publish_input() -> PublishPageBuilderInput {
        PublishPageBuilderInput {
            page_id: "home".to_string(),
            revision_id: "rev-1".to_string(),
            schema_version: "grapesjs_v1".to_string(),
            project_data: serde_json::json!({}),
        }
    }

    fn auth_with(permissions: Vec<Permission>) -> PageBuilderRequestAuth {
        PageBuilderRequestAuth::new(permissions)
    }

    fn assert_disabled<T: std::fmt::Debug>(result: PageBuilderServiceResult<T>, capability: &str) {
        match result.expect_err("capability should be disabled") {
            PageBuilderServiceError::CapabilityDisabled(name) => assert_eq!(name, capability),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn guarded_service_blocks_disabled_publish_before_write_semantics() {
        let flags = BuilderCapabilityFlags {
            builder_enabled: true,
            preview_enabled: true,
            properties_enabled: true,
            publish_enabled: false,
            legacy_bridge_readonly: false,
        };
        let service = CapabilityGuardedService::new(StubService, flags);

        let err = service
            .publish(&read_context(), publish_input())
            .await
            .expect_err("publish should be blocked by capability before context validation");

        match err {
            PageBuilderServiceError::CapabilityDisabled(name) => {
                assert_eq!(name, "publish");
                assert_eq!(
                    PageBuilderServiceError::CapabilityDisabled(name).stable_code(),
                    Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE)
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn publish_requires_write_port_semantics() {
        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::AllOn.flags());

        let err = service
            .publish(&read_context(), publish_input())
            .await
            .expect_err("publish requires write semantics");

        match err {
            PageBuilderServiceError::Validation(message) => {
                assert!(message.contains("idempotency key"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn guarded_service_fallback_profiles_enforce_capability_outcomes() {
        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::AllOn.flags());
        service
            .preview(&read_context(), preview_input())
            .await
            .expect("preview enabled");
        service
            .tree(&read_context(), tree_input())
            .await
            .expect("tree enabled");
        service
            .properties(&read_context(), properties_input())
            .await
            .expect("properties enabled");
        service
            .publish(&write_context(), publish_input())
            .await
            .expect("publish enabled");

        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::PublishOff.flags());
        service
            .preview(&read_context(), preview_input())
            .await
            .expect("preview enabled");
        service
            .tree(&read_context(), tree_input())
            .await
            .expect("tree enabled");
        service
            .properties(&read_context(), properties_input())
            .await
            .expect("properties enabled");
        assert_disabled(
            service.publish(&write_context(), publish_input()).await,
            "publish",
        );

        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::PreviewOff.flags());
        assert_disabled(
            service.preview(&read_context(), preview_input()).await,
            "preview",
        );
        service
            .tree(&read_context(), tree_input())
            .await
            .expect("tree enabled");
        service
            .properties(&read_context(), properties_input())
            .await
            .expect("properties enabled");
        assert_disabled(
            service.publish(&write_context(), publish_input()).await,
            "publish",
        );

        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::BuilderOff.flags());
        assert_disabled(
            service.preview(&read_context(), preview_input()).await,
            "preview",
        );
        assert_disabled(service.tree(&read_context(), tree_input()).await, "tree");
        assert_disabled(
            service
                .properties(&read_context(), properties_input())
                .await,
            "properties",
        );
        assert_disabled(
            service.publish(&write_context(), publish_input()).await,
            "publish",
        );
    }

    #[test]
    fn authorizer_maps_capabilities_to_stable_page_permissions() {
        let authorizer = PageBuilderCapabilityAuthorizer::default();

        assert_eq!(
            authorizer.required_permission(BuilderCapabilityKind::Preview),
            Permission::new(Resource::Pages, Action::Read)
        );
        assert_eq!(
            authorizer.required_permission(BuilderCapabilityKind::Tree),
            Permission::new(Resource::Pages, Action::Read)
        );
        assert_eq!(
            authorizer.required_permission(BuilderCapabilityKind::Properties),
            Permission::new(Resource::Pages, Action::Update)
        );
        assert_eq!(
            authorizer.required_permission(BuilderCapabilityKind::Publish),
            Permission::new(Resource::Pages, Action::Publish)
        );
    }

    #[tokio::test]
    async fn authorized_handlers_enforce_permissions_before_service_call() {
        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::AllOn.flags());
        let handlers = AuthorizedPageBuilderHandlers::new(service);
        let auth = auth_with(vec![Permission::new(Resource::Pages, Action::Read)]);

        handlers
            .preview(&read_context(), &auth, preview_input())
            .await
            .expect("preview is allowed by pages:read");
        handlers
            .tree(&read_context(), &auth, tree_input())
            .await
            .expect("tree is allowed by pages:read");

        let err = handlers
            .properties(&read_context(), &auth, properties_input())
            .await
            .expect_err("properties requires pages:update");
        match err {
            PageBuilderServiceError::Forbidden(message) => {
                assert!(message.contains("properties capability requires pages:update"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn service_errors_expose_typed_catalog_kind_and_code() {
        let validation = PageBuilderServiceError::Validation("bad payload".to_string());
        assert_eq!(validation.kind(), PageBuilderErrorKind::Validation);
        assert_eq!(validation.stable_code(), None);

        let disabled =
            PageBuilderServiceError::from(BuilderRolloutError::CapabilityDisabled("publish"));
        assert_eq!(disabled.kind(), PageBuilderErrorKind::FeatureDisabled);
        assert_eq!(
            disabled.stable_code(),
            Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE)
        );
    }

    #[tokio::test]
    async fn authorized_handlers_honor_manage_as_effective_permission() {
        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::AllOn.flags());
        let handlers = AuthorizedPageBuilderHandlers::new(service);
        let auth = auth_with(vec![Permission::new(Resource::Pages, Action::Manage)]);

        handlers
            .properties(&read_context(), &auth, properties_input())
            .await
            .expect("pages:manage grants properties");
        handlers
            .publish(&write_context(), &auth, publish_input())
            .await
            .expect("pages:manage grants publish");
    }

    #[tokio::test]
    async fn reference_service_renders_preview_and_validates_schema_contract() {
        let service = ReferencePageBuilderService;

        let preview = service
            .preview(
                &read_context(),
                PreviewPageBuilderInput {
                    page_id: "landing".to_string(),
                    schema_version: "grapesjs_v1".to_string(),
                    project_data: serde_json::json!({ "html": "<main>Welcome</main>" }),
                },
            )
            .await
            .expect("reference preview should render sanitized html wrapper");

        assert_eq!(preview.page_id, "landing");
        assert_eq!(
            preview.html,
            "<div data-rustok-page-builder=\"grapesjs_v1\"><main>Welcome</main></div>"
        );

        let err = service
            .preview(
                &read_context(),
                PreviewPageBuilderInput {
                    page_id: "landing".to_string(),
                    schema_version: "legacy_blocks".to_string(),
                    project_data: serde_json::json!({}),
                },
            )
            .await
            .expect_err("unsupported schema should fail validation");

        assert_eq!(err.kind(), PageBuilderErrorKind::Validation);
    }

    #[tokio::test]
    async fn reference_service_exposes_sanitize_error_for_script_payloads() {
        let service = ReferencePageBuilderService;

        let err = service
            .preview(
                &read_context(),
                PreviewPageBuilderInput {
                    page_id: "landing".to_string(),
                    schema_version: "grapesjs_v1".to_string(),
                    project_data: serde_json::json!({ "html": "<script>alert(1)</script>" }),
                },
            )
            .await
            .expect_err("script payload should be rejected by sanitize guard");

        assert_eq!(err.kind(), PageBuilderErrorKind::Sanitize);
    }

    #[tokio::test]
    async fn reference_service_publishes_only_valid_contract_payloads() {
        let service = ReferencePageBuilderService;

        let result = service
            .publish(
                &write_context(),
                PublishPageBuilderInput {
                    page_id: "landing".to_string(),
                    revision_id: "rev-2".to_string(),
                    schema_version: "grapesjs_v1".to_string(),
                    project_data: serde_json::json!({ "html": "<main>Ready</main>" }),
                },
            )
            .await
            .expect("valid reference publish should return a typed publish result");

        assert_eq!(result.page_id, "landing");
        assert_eq!(result.revision_id, "rev-2");
        assert!(result.published);
    }

    #[tokio::test]
    async fn transport_neutral_handler_dispatches_tagged_capability_requests() {
        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::AllOn.flags());
        let handlers = AuthorizedPageBuilderHandlers::new(service);
        let auth = auth_with(vec![Permission::new(Resource::Pages, Action::Manage)]);

        let response = handlers
            .handle(
                &write_context(),
                &auth,
                PageBuilderCapabilityRequest::Publish(publish_input()),
            )
            .await
            .expect("publish request is dispatched");

        match response {
            PageBuilderCapabilityResponse::Publish(result) => {
                assert_eq!(result.page_id, "home");
                assert!(result.published);
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[test]
    fn adapter_call_evidence_carries_port_context_and_contract_markers() {
        let evidence =
            PageBuilderAdapterCallEvidence::save_project(&write_context(), "home", "rev-1");

        assert_eq!(evidence.module_slug, "page_builder");
        assert_eq!(evidence.contract, "grapesjs_v1");
        assert_eq!(evidence.operation.as_str(), "save_project");
        assert_eq!(evidence.tenant_id, "tenant-a");
        assert_eq!(evidence.page_id, "home");
        assert_eq!(evidence.revision_id.as_deref(), Some("rev-1"));
        assert_eq!(evidence.correlation_id, "corr-write");
    }
}
