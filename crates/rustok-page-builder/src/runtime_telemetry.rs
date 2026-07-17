use crate::dto::{
    PageBuilderContractMetadata, PageBuilderErrorKind, PageBuilderModuleMetadata,
};
use crate::service::{
    PageBuilderAdapterCallEvidence, PageBuilderAdapterCallStatus, PageBuilderAdapterOperation,
    PageBuilderAdapterTelemetry, PageBuilderServiceError,
};
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

/// Telemetry for the current Page Builder module API.
///
/// Module semver is supplied by deployment/build metadata. Runtime evidence intentionally carries
/// neither document nor contract versions.
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

/// Compatibility bridge for host recorders implemented against the original telemetry trait.
///
/// The current pipeline remains versionless. The historical contract marker is introduced only
/// while adapting to the old recorder and disappears when that compatibility API is removed in a
/// future module major.
impl<T> PageBuilderRuntimeTelemetry for T
where
    T: PageBuilderAdapterTelemetry,
{
    fn record_runtime_call(&self, evidence: &PageBuilderRuntimeCallEvidence) {
        let operation = match evidence.operation {
            PageBuilderRuntimeOperation::LoadProject => PageBuilderAdapterOperation::LoadProject,
            PageBuilderRuntimeOperation::SaveProject => PageBuilderAdapterOperation::SaveProject,
            PageBuilderRuntimeOperation::RenderPreview => {
                PageBuilderAdapterOperation::RenderPreview
            }
        };
        let status = match evidence.status {
            PageBuilderRuntimeCallStatus::Started => PageBuilderAdapterCallStatus::Started,
            PageBuilderRuntimeCallStatus::Succeeded => PageBuilderAdapterCallStatus::Succeeded,
            PageBuilderRuntimeCallStatus::Failed => PageBuilderAdapterCallStatus::Failed,
        };
        self.record_adapter_call(&PageBuilderAdapterCallEvidence {
            module_slug: evidence.module_slug,
            contract: PageBuilderContractMetadata::BASELINE.contract,
            operation,
            status,
            tenant_id: evidence.tenant_id.clone(),
            page_id: evidence.page_id.clone(),
            revision_id: evidence.revision_id.clone(),
            correlation_id: evidence.correlation_id.clone(),
            error_kind: evidence.error_kind,
            stable_code: evidence.stable_code,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;
    use std::sync::Mutex;

    #[test]
    fn runtime_evidence_has_no_contract_or_schema_version() {
        let context = PortContext::new(
            "tenant-a",
            PortActor::user("editor-a"),
            "en",
            "correlation-a",
        );
        let value = serde_json::to_value(PageBuilderRuntimeCallEvidence::load_project(
            &context,
            "home",
        ))
        .expect("runtime evidence");

        assert_eq!(value["module_slug"], "page_builder");
        assert!(value.get("contract").is_none());
        assert!(value.get("schema_version").is_none());
        assert!(value.get("version").is_none());
    }

    #[derive(Default)]
    struct CompatibilityRecorder {
        evidence: Mutex<Vec<PageBuilderAdapterCallEvidence>>,
    }

    impl PageBuilderAdapterTelemetry for CompatibilityRecorder {
        fn record_adapter_call(&self, evidence: &PageBuilderAdapterCallEvidence) {
            self.evidence.lock().expect("recorder lock").push(evidence.clone());
        }
    }

    #[test]
    fn original_telemetry_implementations_remain_usable() {
        let context = PortContext::new(
            "tenant-a",
            PortActor::user("editor-a"),
            "en",
            "correlation-a",
        );
        let recorder = CompatibilityRecorder::default();
        recorder.record_runtime_call(&PageBuilderRuntimeCallEvidence::load_project(
            &context,
            "home",
        ));

        let evidence = recorder.evidence.lock().expect("recorder lock");
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].page_id, "home");
    }
}
