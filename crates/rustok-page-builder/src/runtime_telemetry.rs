use crate::dto::{PageBuilderErrorKind, PageBuilderModuleMetadata};
use crate::health::{
    ProviderHealthOperation, ProviderHealthOutcome, record_provider_health_observation,
};
use crate::service::PageBuilderServiceError;
use rustok_api::PortContext;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const PROVIDER_HEALTH_PENDING_CALL_CAPACITY: usize = 1024;

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

#[derive(Debug)]
struct PendingProviderHealthCall {
    operation: PageBuilderRuntimeOperation,
    tenant_id: String,
    page_id: String,
    revision_id: Option<String>,
    correlation_id: String,
    started_at: Instant,
}

impl PendingProviderHealthCall {
    fn from_evidence(evidence: &PageBuilderRuntimeCallEvidence) -> Self {
        Self {
            operation: evidence.operation,
            tenant_id: evidence.tenant_id.clone(),
            page_id: evidence.page_id.clone(),
            revision_id: evidence.revision_id.clone(),
            correlation_id: evidence.correlation_id.clone(),
            started_at: Instant::now(),
        }
    }

    fn matches(&self, evidence: &PageBuilderRuntimeCallEvidence) -> bool {
        self.operation == evidence.operation
            && self.tenant_id == evidence.tenant_id
            && self.page_id == evidence.page_id
            && self.revision_id == evidence.revision_id
            && self.correlation_id == evidence.correlation_id
    }
}

/// Production-default runtime telemetry for the bounded process-local provider-health window.
///
/// It observes only canonical Fly adapter terminal calls already emitted by the provider service.
/// Load-project telemetry is intentionally excluded from the current Preview/Publish SLO contract.
/// A process restart clears pending calls and the health window remains unobserved until the
/// minimum sample floor is rebuilt.
#[derive(Debug, Clone, Default)]
pub struct ProviderHealthRuntimeTelemetry {
    pending: Arc<Mutex<VecDeque<PendingProviderHealthCall>>>,
}

impl ProviderHealthRuntimeTelemetry {
    fn provider_operation(
        operation: PageBuilderRuntimeOperation,
    ) -> Option<ProviderHealthOperation> {
        match operation {
            PageBuilderRuntimeOperation::RenderPreview => Some(ProviderHealthOperation::Preview),
            PageBuilderRuntimeOperation::SaveProject => Some(ProviderHealthOperation::Publish),
            PageBuilderRuntimeOperation::LoadProject => None,
        }
    }

    fn remember_started(&self, evidence: &PageBuilderRuntimeCallEvidence) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(index) = pending.iter().rposition(|call| call.matches(evidence)) {
            pending.remove(index);
        }
        if pending.len() >= PROVIDER_HEALTH_PENDING_CALL_CAPACITY {
            pending.pop_front();
        }
        pending.push_back(PendingProviderHealthCall::from_evidence(evidence));
    }

    fn finish(
        &self,
        evidence: &PageBuilderRuntimeCallEvidence,
        operation: ProviderHealthOperation,
    ) {
        let pending_call = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(index) = pending.iter().rposition(|call| call.matches(evidence)) else {
                return;
            };
            pending.remove(index)
        };
        let Some(pending_call) = pending_call else {
            return;
        };
        let outcome = match evidence.status {
            PageBuilderRuntimeCallStatus::Succeeded => ProviderHealthOutcome::Succeeded,
            PageBuilderRuntimeCallStatus::Failed => match evidence.error_kind {
                Some(PageBuilderErrorKind::Sanitize) => ProviderHealthOutcome::SanitizeFailed,
                Some(PageBuilderErrorKind::Runtime) => ProviderHealthOutcome::RuntimeFailed,
                _ => ProviderHealthOutcome::OtherFailed,
            },
            PageBuilderRuntimeCallStatus::Started => return,
        };
        record_provider_health_observation(operation, pending_call.started_at.elapsed(), outcome);
    }
}

impl PageBuilderRuntimeTelemetry for ProviderHealthRuntimeTelemetry {
    fn record_runtime_call(&self, evidence: &PageBuilderRuntimeCallEvidence) {
        let Some(operation) = Self::provider_operation(evidence.operation) else {
            return;
        };
        match evidence.status {
            PageBuilderRuntimeCallStatus::Started => self.remember_started(evidence),
            PageBuilderRuntimeCallStatus::Succeeded | PageBuilderRuntimeCallStatus::Failed => {
                self.finish(evidence, operation)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    fn context(correlation_id: &str) -> PortContext {
        PortContext::new(
            "tenant-a",
            PortActor::user("editor-a"),
            "en",
            correlation_id,
        )
    }

    #[test]
    fn runtime_evidence_contains_only_current_fields() {
        let context = context("correlation-a");
        let value = serde_json::to_value(PageBuilderRuntimeCallEvidence::load_project(
            &context, "home",
        ))
        .expect("runtime evidence");

        assert_eq!(value["module_slug"], "page_builder");
        assert!(value.get("contract").is_none());
        assert!(value.get("version").is_none());
    }

    #[test]
    fn provider_health_telemetry_ignores_load_project_and_closes_matching_preview() {
        let telemetry = ProviderHealthRuntimeTelemetry::default();
        let context = context("provider-health-preview");
        let load = PageBuilderRuntimeCallEvidence::load_project(&context, "home");
        telemetry.record_runtime_call(&load);
        telemetry.record_runtime_call(&load.succeeded());
        assert_eq!(
            telemetry
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            0
        );

        let preview = PageBuilderRuntimeCallEvidence::render_preview(&context, "home");
        telemetry.record_runtime_call(&preview);
        assert_eq!(
            telemetry
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            1
        );
        telemetry.record_runtime_call(&preview.succeeded());
        assert_eq!(
            telemetry
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            0
        );
    }
}
