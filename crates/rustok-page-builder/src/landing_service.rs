use crate::dto::{
    BuilderNodePropertiesInput, BuilderNodePropertiesResult, BuilderTreeInput, BuilderTreeResult,
    PreviewPageBuilderInput, PreviewPageBuilderResult, PublishPageBuilderInput,
    PublishPageBuilderResult,
};
use crate::landing::{LandingProjectInspection, LandingProjectResult};
use crate::service::{
    PageBuilderCapabilityService, PageBuilderServiceError, PageBuilderServiceResult,
};
use async_trait::async_trait;
use fly::{LandingReadinessPolicy, RegistrySet, RenderPolicy, ValidationLimits};
use rustok_api::PortContext;
use serde_json::Value;

/// Service decorator for the active landing pipeline.
pub struct LandingValidatedPageBuilderService<S> {
    inner: S,
    registries: RegistrySet,
    limits: ValidationLimits,
    readiness_policy: LandingReadinessPolicy,
    render_policy: RenderPolicy,
}

impl<S> LandingValidatedPageBuilderService<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            registries: RegistrySet::with_builtins(),
            limits: ValidationLimits::default(),
            readiness_policy: LandingReadinessPolicy::default(),
            render_policy: RenderPolicy::default(),
        }
    }

    pub fn with_policy(
        inner: S,
        registries: RegistrySet,
        limits: ValidationLimits,
        readiness_policy: LandingReadinessPolicy,
        render_policy: RenderPolicy,
    ) -> Self {
        Self {
            inner,
            registries,
            limits,
            readiness_policy,
            render_policy,
        }
    }

    pub fn inner(&self) -> &S {
        &self.inner
    }

    pub fn inspect(
        &self,
        project_data: &Value,
    ) -> LandingProjectResult<LandingProjectInspection> {
        LandingProjectInspection::decode_with(project_data, &self.registries, self.limits)
    }

    fn validate_preview(&self, project_data: &Value) -> PageBuilderServiceResult<()> {
        let inspection = self.inspect(project_data).map_err(validation_error)?;
        inspection
            .require_contract_valid()
            .map_err(validation_error)
    }

    fn validate_publish(&self, project_data: &Value) -> PageBuilderServiceResult<()> {
        let inspection = self.inspect(project_data).map_err(validation_error)?;
        inspection
            .require_contract_valid()
            .map_err(validation_error)?;
        let build = inspection
            .build_static(
                &self.registries,
                self.readiness_policy,
                &self.render_policy,
            )
            .map_err(validation_error)?;
        if build.ready && build.artifact.is_some() {
            return Ok(());
        }
        let blocking = build
            .readiness
            .blocking_issues()
            .map(|issue| issue.diagnostic.code.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        Err(PageBuilderServiceError::Validation(if blocking.is_empty() {
            "landing publish artifact is not ready".to_string()
        } else {
            format!("landing publish readiness failed: {blocking}")
        }))
    }
}

fn validation_error(error: impl std::fmt::Display) -> PageBuilderServiceError {
    PageBuilderServiceError::Validation(error.to_string())
}

#[async_trait]
impl<S> PageBuilderCapabilityService for LandingValidatedPageBuilderService<S>
where
    S: PageBuilderCapabilityService,
{
    async fn preview(
        &self,
        context: &PortContext,
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
        self.validate_preview(&input.project_data)?;
        self.inner.preview(context, input).await
    }

    async fn tree(
        &self,
        context: &PortContext,
        input: BuilderTreeInput,
    ) -> PageBuilderServiceResult<BuilderTreeResult> {
        self.inner.tree(context, input).await
    }

    async fn properties(
        &self,
        context: &PortContext,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
        self.inner.properties(context, input).await
    }

    async fn publish(
        &self,
        context: &PortContext,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
        self.validate_publish(&input.project_data)?;
        self.inner.publish(context, input).await
    }
}
