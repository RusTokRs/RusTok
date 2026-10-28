use fly::{
    diff_runtime_context_contracts_with_policy, FlyResult, GrapesJsCodec,
    RuntimeContextCompatibilityPolicy, RuntimeContextCompatibilityReport,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractCompatibilityRequest {
    /// Document an existing runtime context payload was authored against.
    pub source_project_data: Value,
    /// Candidate document that must keep accepting those payloads.
    pub target_project_data: Value,
    #[serde(default)]
    pub policy: RuntimeContextCompatibilityPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractCompatibilityResponse {
    pub report: RuntimeContextCompatibilityReport,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeContractCompatibilityInspector;

impl PageBuilderRuntimeContractCompatibilityInspector {
    pub fn compare(
        &self,
        request: PageBuilderRuntimeContractCompatibilityRequest,
    ) -> FlyResult<PageBuilderRuntimeContractCompatibilityResponse> {
        let source = GrapesJsCodec::decode_value(request.source_project_data)?;
        let target = GrapesJsCodec::decode_value(request.target_project_data)?;
        Ok(PageBuilderRuntimeContractCompatibilityResponse {
            report: diff_runtime_context_contracts_with_policy(&source, &target, request.policy),
        })
    }
}

pub fn compare_page_builder_runtime_context_contracts(
    request: PageBuilderRuntimeContractCompatibilityRequest,
) -> FlyResult<PageBuilderRuntimeContractCompatibilityResponse> {
    PageBuilderRuntimeContractCompatibilityInspector.compare(request)
}
