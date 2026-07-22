use crate::dto::{
    BuilderCapabilityKind, BuilderNodePropertiesInput, BuilderNodePropertiesResult,
    BuilderTreeInput, BuilderTreeResult, PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE,
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse, PageBuilderErrorKind,
    PreviewPageBuilderInput, PreviewPageBuilderResult, PublishPageBuilderInput,
    PublishPageBuilderResult,
};
use crate::rollout::{BuilderCapabilityFlags, BuilderRolloutError, ensure_capability};
use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_api::{PortCallPolicy, PortContext, PortErrorKind};
use serde::Serialize;

pub const PAGE_BUILDER_PAGES_READ_PERMISSION: &str = "pages:read";
pub const PAGE_BUILDER_PAGES_UPDATE_PERMISSION: &str = "pages:update";
pub const PAGE_BUILDER_PAGES_PUBLISH_PERMISSION: &str = "pages:publish";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PageBuilderCapabilityPermissionDescriptor {
    pub capability: BuilderCapabilityKind,
    pub permission: &'static str,
}

pub const PAGE_BUILDER_CAPABILITY_PERMISSIONS: [PageBuilderCapabilityPermissionDescriptor; 4] = [
    PageBuilderCapabilityPermissionDescriptor {
        capability: BuilderCapabilityKind::Preview,
        permission: PAGE_BUILDER_PAGES_READ_PERMISSION,
    },
    PageBuilderCapabilityPermissionDescriptor {
        capability: BuilderCapabilityKind::Tree,
        permission: PAGE_BUILDER_PAGES_READ_PERMISSION,
    },
    PageBuilderCapabilityPermissionDescriptor {
        capability: BuilderCapabilityKind::Properties,
        permission: PAGE_BUILDER_PAGES_UPDATE_PERMISSION,
    },
    PageBuilderCapabilityPermissionDescriptor {
        capability: BuilderCapabilityKind::Publish,
        permission: PAGE_BUILDER_PAGES_PUBLISH_PERMISSION,
    },
];

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

pub const PAGE_BUILDER_READ_POLICY_NAME: &str = "read_deadline_required";
pub const PAGE_BUILDER_WRITE_POLICY_NAME: &str = "write_deadline_and_idempotency_required";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PageBuilderCapabilityPortPolicyDescriptor {
    pub capability: BuilderCapabilityKind,
    pub policy_name: &'static str,
}

pub const PAGE_BUILDER_CAPABILITY_PORT_POLICIES: [PageBuilderCapabilityPortPolicyDescriptor; 4] = [
    PageBuilderCapabilityPortPolicyDescriptor {
        capability: BuilderCapabilityKind::Preview,
        policy_name: PAGE_BUILDER_READ_POLICY_NAME,
    },
    PageBuilderCapabilityPortPolicyDescriptor {
        capability: BuilderCapabilityKind::Tree,
        policy_name: PAGE_BUILDER_READ_POLICY_NAME,
    },
    PageBuilderCapabilityPortPolicyDescriptor {
        capability: BuilderCapabilityKind::Properties,
        policy_name: PAGE_BUILDER_READ_POLICY_NAME,
    },
    PageBuilderCapabilityPortPolicyDescriptor {
        capability: BuilderCapabilityKind::Publish,
        policy_name: PAGE_BUILDER_WRITE_POLICY_NAME,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageBuilderCapabilityPortPolicies {
    pub preview: PortCallPolicy,
    pub tree: PortCallPolicy,
    pub properties: PortCallPolicy,
    pub publish: PortCallPolicy,
}

impl Default for PageBuilderCapabilityPortPolicies {
    fn default() -> Self {
        Self {
            preview: PortCallPolicy::read(),
            tree: PortCallPolicy::read(),
            properties: PortCallPolicy::read(),
            publish: PortCallPolicy::write(),
        }
    }
}

impl PageBuilderCapabilityPortPolicies {
    pub fn required_for(self, capability: BuilderCapabilityKind) -> PortCallPolicy {
        match capability {
            BuilderCapabilityKind::Preview => self.preview,
            BuilderCapabilityKind::Tree => self.tree,
            BuilderCapabilityKind::Properties => self.properties,
            BuilderCapabilityKind::Publish => self.publish,
        }
    }

    pub const fn policy_name_for(capability: BuilderCapabilityKind) -> &'static str {
        match capability {
            BuilderCapabilityKind::Preview
            | BuilderCapabilityKind::Tree
            | BuilderCapabilityKind::Properties => PAGE_BUILDER_READ_POLICY_NAME,
            BuilderCapabilityKind::Publish => PAGE_BUILDER_WRITE_POLICY_NAME,
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

/// Persistence port used by the current Fly-backed Page Builder service.
/// Implementations own storage and must preserve tenant isolation from [`PortContext`].
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

/// Preview rendering port used after Fly decode and validation.
#[async_trait]
pub trait PageBuilderRenderingAdapter: Send + Sync {
    async fn render_preview(
        &self,
        context: &PortContext,
        project_data: &serde_json::Value,
    ) -> PageBuilderServiceResult<String>;
}

pub struct CapabilityGuardedService<S> {
    inner: S,
    flags: BuilderCapabilityFlags,
    policies: PageBuilderCapabilityPortPolicies,
}

impl<S> CapabilityGuardedService<S> {
    pub fn new(inner: S, flags: BuilderCapabilityFlags) -> Self {
        Self {
            inner,
            flags,
            policies: PageBuilderCapabilityPortPolicies::default(),
        }
    }

    pub fn with_policies(
        inner: S,
        flags: BuilderCapabilityFlags,
        policies: PageBuilderCapabilityPortPolicies,
    ) -> Self {
        Self {
            inner,
            flags,
            policies,
        }
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
        context
            .require_policy(self.policies.required_for(BuilderCapabilityKind::Preview))
            .map_err(PageBuilderServiceError::from_port_error)?;
        self.inner.preview(context, input).await
    }

    async fn tree(
        &self,
        context: &PortContext,
        input: BuilderTreeInput,
    ) -> PageBuilderServiceResult<BuilderTreeResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Tree)?;
        context
            .require_policy(self.policies.required_for(BuilderCapabilityKind::Tree))
            .map_err(PageBuilderServiceError::from_port_error)?;
        self.inner.tree(context, input).await
    }

    async fn properties(
        &self,
        context: &PortContext,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Properties)?;
        context
            .require_policy(
                self.policies
                    .required_for(BuilderCapabilityKind::Properties),
            )
            .map_err(PageBuilderServiceError::from_port_error)?;
        self.inner.properties(context, input).await
    }

    async fn publish(
        &self,
        context: &PortContext,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Publish)?;
        context
            .require_policy(self.policies.required_for(BuilderCapabilityKind::Publish))
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
            .with_deadline(std::time::Duration::from_secs(3))
    }

    fn no_deadline_context() -> PortContext {
        PortContext::new(
            "tenant-a",
            PortActor::user("editor-a"),
            "ru",
            "corr-no-deadline",
        )
    }

    fn write_context() -> PortContext {
        PortContext::new("tenant-a", PortActor::user("editor-a"), "ru", "corr-write")
            .with_idempotency_key("idem-a")
            .with_deadline(std::time::Duration::from_secs(3))
    }

    fn preview_input() -> PreviewPageBuilderInput {
        PreviewPageBuilderInput {
            page_id: "home".to_string(),
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
    async fn read_capabilities_require_deadline_semantics() {
        let service =
            CapabilityGuardedService::new(StubService, BuilderToggleProfile::AllOn.flags());

        let err = service
            .preview(&no_deadline_context(), preview_input())
            .await
            .expect_err("preview requires read deadline semantics");

        match err {
            PageBuilderServiceError::Runtime(message) => {
                assert!(message.contains("deadline"));
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
        let descriptors: Vec<_> = PAGE_BUILDER_CAPABILITY_PERMISSIONS
            .iter()
            .map(|descriptor| (descriptor.capability, descriptor.permission))
            .collect();

        assert_eq!(
            descriptors,
            vec![
                (
                    BuilderCapabilityKind::Preview,
                    PAGE_BUILDER_PAGES_READ_PERMISSION
                ),
                (
                    BuilderCapabilityKind::Tree,
                    PAGE_BUILDER_PAGES_READ_PERMISSION
                ),
                (
                    BuilderCapabilityKind::Properties,
                    PAGE_BUILDER_PAGES_UPDATE_PERMISSION
                ),
                (
                    BuilderCapabilityKind::Publish,
                    PAGE_BUILDER_PAGES_PUBLISH_PERMISSION
                ),
            ]
        );

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

    #[test]
    fn port_policy_names_match_fba_registry_contract() {
        let descriptors: Vec<_> = PAGE_BUILDER_CAPABILITY_PORT_POLICIES
            .iter()
            .map(|descriptor| (descriptor.capability, descriptor.policy_name))
            .collect();

        assert_eq!(
            descriptors,
            vec![
                (
                    BuilderCapabilityKind::Preview,
                    PAGE_BUILDER_READ_POLICY_NAME
                ),
                (BuilderCapabilityKind::Tree, PAGE_BUILDER_READ_POLICY_NAME),
                (
                    BuilderCapabilityKind::Properties,
                    PAGE_BUILDER_READ_POLICY_NAME
                ),
                (
                    BuilderCapabilityKind::Publish,
                    PAGE_BUILDER_WRITE_POLICY_NAME
                ),
            ]
        );
        assert_eq!(
            PageBuilderCapabilityPortPolicies::policy_name_for(BuilderCapabilityKind::Preview),
            PAGE_BUILDER_READ_POLICY_NAME
        );
        assert_eq!(
            PageBuilderCapabilityPortPolicies::policy_name_for(BuilderCapabilityKind::Tree),
            PAGE_BUILDER_READ_POLICY_NAME
        );
        assert_eq!(
            PageBuilderCapabilityPortPolicies::policy_name_for(BuilderCapabilityKind::Properties),
            PAGE_BUILDER_READ_POLICY_NAME
        );
        assert_eq!(
            PageBuilderCapabilityPortPolicies::policy_name_for(BuilderCapabilityKind::Publish),
            PAGE_BUILDER_WRITE_POLICY_NAME
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
    fn runtime_call_evidence_carries_port_context() {
        let evidence = crate::runtime_telemetry::PageBuilderRuntimeCallEvidence::save_project(
            &write_context(),
            "home",
            "rev-1",
        );

        assert_eq!(evidence.module_slug, "page_builder");
        assert_eq!(evidence.operation.as_str(), "save_project");
        assert_eq!(
            evidence.status,
            crate::runtime_telemetry::PageBuilderRuntimeCallStatus::Started
        );
        assert_eq!(evidence.tenant_id, "tenant-a");
        assert_eq!(evidence.page_id, "home");
        assert_eq!(evidence.revision_id.as_deref(), Some("rev-1"));
        assert_eq!(evidence.correlation_id, "corr-write");

        let failed = evidence.failed(&PageBuilderServiceError::Sanitize(
            "blocked script".to_string(),
        ));
        assert_eq!(
            failed.status,
            crate::runtime_telemetry::PageBuilderRuntimeCallStatus::Failed
        );
        assert_eq!(failed.error_kind, Some(PageBuilderErrorKind::Sanitize));
    }
}
