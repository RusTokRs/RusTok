use crate::dto::PreviewPageBuilderInput;
use crate::service::PageBuilderServiceResult;
use async_trait::async_trait;
use rustok_api::PortContext;

/// Canonical preview rendering port.
///
/// The complete preview DTO is passed after Page Builder authorization, rollout checks and Fly
/// structural validation. Consumer renderers must not invent local runtime-context arguments.
#[async_trait]
pub trait PageBuilderPreviewRenderingPort: Send + Sync {
    async fn render_preview(
        &self,
        context: &PortContext,
        input: &PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<String>;
}
