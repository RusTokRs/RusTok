use crate::dto::{PageBuilderErrorKind, PageBuilderModuleMetadata};
use crate::service::PageBuilderServiceError;
use rustok_api::PortContext;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageBuilderRuntimeOperation {
    LoadProject,
    SaveProject,
    RenderPreview,
}

impl PageBuilderRuntimeOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadProject => "load_project",
            Self::SaveProject => "save_project",
            Self::RenderPreview => "render_preview",
        }
    }
}

impl std::fmt::Display for PageBuilderRuntimeOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageBuilderRuntimeCallStatus {
    Started,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageBuilderRuntimeCallEvidence {
    pub module_slug: &'static str,
    pub operation: PageBuilderRuntimeOperation,
    pub status: PageBuilderRuntimeCallStatus,
    pub tenant_id: String,
    pub page_id: String,
    pub revision_id: Option<String>,
    pub correlation_id: String,
    pub error_kind: Option<PageBuilderErrorKind>,
    pub stable_code: Option<&'static str>,
}

impl PageBuilderRuntimeCallEvidence {
    pub fn load_project(context: &PortContext, page_id: impl Into<String>) -> Self {
        Self::new(
            PageBuilderRuntimeOperation::LoadProject,
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
            PageBuilderRuntimeOperation::SaveProject,
            context,
            page_id,
            Some(revision_id.into()),
        )
    }

    pub fn render_preview(context: &PortContext, page_id: impl Into<String>) -> Self {
        Self::new(
            PageBuilderRuntimeOperation::RenderPreview,
            context,
            page_id,
            None,
        )
    }

    fn new(
        operation: PageBuilderRuntimeOperation,
        context: &PortContext,
        page_id: impl Into<String>,
        revision_id: Option<String>,
    ) -> Self {
        Self {
            module_slug: PageBuilderModuleMetadata::CURRENT.module_slug,
            operation,
            status: PageBuilderRuntimeCallStatus::Started,
            tenant_id: context.tenant_id.clone(),
            page_id: page_id.into(),
            revision_id,
            correlation_id: context.correlation_id.clone(),
            error_kind: None,
            stable_code: None,
        }
    }

    pub fn succeeded(&self) -> Self {
        let mut evidence = self.clone();
        evidence.status = PageBuilderRuntimeCallStatus::Succeeded;
        evidence
    }

    pub fn failed(&self, error: &PageBuilderServiceError) -> Self {
        let mut evidence = self.clone();
        evidence.status = PageBuilderRuntimeCallStatus::Failed;
        evidence.error_kind = Some(error.kind());
        evidence.stable_code = error.stable_code();
        evidence
    }
}

pub trait PageBuilderRuntimeTelemetry: Send + Sync {
    fn record_runtime_call(&self, evidence: &PageBuilderRuntimeCallEvidence);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopPageBuilderRuntimeTelemetry;

impl PageBuilderRuntimeTelemetry for NoopPageBuilderRuntimeTelemetry {
    fn record_runtime_call(&self, _evidence: &PageBuilderRuntimeCallEvidence) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn runtime_evidence_contains_only_current_fields() {
        let context = PortContext::new(
            "tenant-a",
            PortActor::user("editor-a"),
            "en",
            "correlation-a",
        );
        let value = serde_json::to_value(PageBuilderRuntimeCallEvidence::load_project(
            &context, "home",
        ))
        .expect("runtime evidence");

        assert_eq!(value["module_slug"], "page_builder");
        assert!(value.get("contract").is_none());
        assert!(value.get("version").is_none());
    }
}
