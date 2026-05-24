use crate::dto::{
    BuilderCapabilityKind, BuilderNodePropertiesInput, BuilderNodePropertiesResult,
    BuilderTreeInput, BuilderTreeResult, PreviewPageBuilderInput, PreviewPageBuilderResult,
    PublishPageBuilderInput, PublishPageBuilderResult,
};
use crate::rollout::{ensure_capability, BuilderCapabilityFlags, BuilderRolloutError};
use async_trait::async_trait;

#[async_trait]
pub trait PageBuilderCapabilityService: Send + Sync {
    async fn preview(
        &self,
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult>;

    async fn tree(&self, input: BuilderTreeInput) -> PageBuilderServiceResult<BuilderTreeResult>;

    async fn properties(
        &self,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult>;

    async fn publish(
        &self,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult>;
}

pub type PageBuilderServiceResult<T> = Result<T, PageBuilderServiceError>;

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderServiceError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("capability disabled: {0}")]
    CapabilityDisabled(String),
    #[error("runtime error: {0}")]
    Runtime(String),
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
        input: PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Preview)?;
        self.inner.preview(input).await
    }

    async fn tree(&self, input: BuilderTreeInput) -> PageBuilderServiceResult<BuilderTreeResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Tree)?;
        self.inner.tree(input).await
    }

    async fn properties(
        &self,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Properties)?;
        self.inner.properties(input).await
    }

    async fn publish(
        &self,
        input: PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
        ensure_capability(&self.flags, BuilderCapabilityKind::Publish)?;
        self.inner.publish(input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubService;

    #[async_trait]
    impl PageBuilderCapabilityService for StubService {
        async fn preview(
            &self,
            input: PreviewPageBuilderInput,
        ) -> PageBuilderServiceResult<PreviewPageBuilderResult> {
            Ok(PreviewPageBuilderResult {
                page_id: input.page_id,
                html: "<div/>".to_string(),
            })
        }

        async fn tree(
            &self,
            input: BuilderTreeInput,
        ) -> PageBuilderServiceResult<BuilderTreeResult> {
            Ok(BuilderTreeResult {
                page_id: input.page_id,
                nodes: vec![],
            })
        }

        async fn properties(
            &self,
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
            input: PublishPageBuilderInput,
        ) -> PageBuilderServiceResult<PublishPageBuilderResult> {
            Ok(PublishPageBuilderResult {
                page_id: input.page_id,
                revision_id: input.revision_id,
                published: true,
            })
        }
    }

    #[tokio::test]
    async fn guarded_service_blocks_disabled_publish() {
        let flags = BuilderCapabilityFlags {
            builder_enabled: true,
            preview_enabled: true,
            properties_enabled: true,
            publish_enabled: false,
            legacy_bridge_readonly: false,
        };
        let service = CapabilityGuardedService::new(StubService, flags);

        let err = service
            .publish(PublishPageBuilderInput {
                page_id: "home".to_string(),
                revision_id: "rev-1".to_string(),
                schema_version: "grapesjs_v1".to_string(),
                project_data: serde_json::json!({}),
            })
            .await
            .expect_err("publish should be blocked");

        match err {
            PageBuilderServiceError::CapabilityDisabled(name) => assert_eq!(name, "publish"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
