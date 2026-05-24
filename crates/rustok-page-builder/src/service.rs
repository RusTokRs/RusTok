use crate::dto::{
    BuilderNodePropertiesInput, BuilderNodePropertiesResult, BuilderTreeInput, BuilderTreeResult,
    PreviewPageBuilderInput, PreviewPageBuilderResult, PublishPageBuilderInput,
    PublishPageBuilderResult,
};
use async_trait::async_trait;

#[async_trait]
pub trait PageBuilderCapabilityService: Send + Sync {
    async fn preview(&self, input: PreviewPageBuilderInput) -> PageBuilderServiceResult<PreviewPageBuilderResult>;

    async fn tree(&self, input: BuilderTreeInput) -> PageBuilderServiceResult<BuilderTreeResult>;

    async fn properties(
        &self,
        input: BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<BuilderNodePropertiesResult>;

    async fn publish(&self, input: PublishPageBuilderInput)
        -> PageBuilderServiceResult<PublishPageBuilderResult>;
}

pub type PageBuilderServiceResult<T> = Result<T, PageBuilderServiceError>;

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderServiceError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}
