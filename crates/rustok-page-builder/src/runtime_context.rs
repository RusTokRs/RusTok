use fly::{
    extract_runtime_context_contract, preflight_runtime_context, FlyResult, GrapesJsV1Codec,
    RuntimeContextContract, RuntimeContextPreflight, RuntimeContextPreflightPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractRequest {
    pub project_data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeContractResponse {
    pub contract: RuntimeContextContract,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimePreflightRequest {
    pub project_data: Value,
    #[serde(default)]
    pub context: Value,
    #[serde(default)]
    pub policy: RuntimeContextPreflightPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimePreflightResponse {
    pub preflight: RuntimeContextPreflight,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeContextInspector;

impl PageBuilderRuntimeContextInspector {
    pub fn contract(
        &self,
        request: PageBuilderRuntimeContractRequest,
    ) -> FlyResult<PageBuilderRuntimeContractResponse> {
        let document = GrapesJsV1Codec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeContractResponse {
            contract: extract_runtime_context_contract(&document),
        })
    }

    pub fn preflight(
        &self,
        request: PageBuilderRuntimePreflightRequest,
    ) -> FlyResult<PageBuilderRuntimePreflightResponse> {
        let document = GrapesJsV1Codec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimePreflightResponse {
            preflight: preflight_runtime_context(&document, &request.context, request.policy),
        })
    }
}

pub fn inspect_page_builder_runtime_contract(
    request: PageBuilderRuntimeContractRequest,
) -> FlyResult<PageBuilderRuntimeContractResponse> {
    PageBuilderRuntimeContextInspector.contract(request)
}

pub fn preflight_page_builder_runtime_context(
    request: PageBuilderRuntimePreflightRequest,
) -> FlyResult<PageBuilderRuntimePreflightResponse> {
    PageBuilderRuntimeContextInspector.preflight(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project_data() -> Value {
        json!({
            "pages": [{
                "component": { "id": "root", "type": "wrapper" }
            }],
            "flyRuntimeContextSchema": [{
                "id": "title",
                "path": "page.title",
                "kind": "string",
                "required": true
            }, {
                "id": "currency",
                "path": "shop.currency",
                "kind": "string",
                "default": "EUR"
            }],
            "flyRuntimeComputed": [{
                "id": "label",
                "path": "page.label",
                "expression": {
                    "op": "format",
                    "template": "{{shop.currency}} {{page.title}}"
                }
            }]
        })
    }

    #[test]
    fn consumer_can_extract_contract_without_rendering() {
        let response = inspect_page_builder_runtime_contract(PageBuilderRuntimeContractRequest {
            project_data: project_data(),
        })
        .expect("contract response");
        assert!(response.contract.is_valid());
        assert_eq!(response.contract.required_paths, vec!["page.title"]);
        assert_eq!(response.contract.defaulted_paths, vec!["shop.currency"]);
        assert_eq!(response.contract.computed_paths, vec!["page.label"]);
    }

    #[test]
    fn strict_preflight_rejects_missing_required_context() {
        let response = preflight_page_builder_runtime_context(
            PageBuilderRuntimePreflightRequest {
                project_data: project_data(),
                context: json!({}),
                policy: RuntimeContextPreflightPolicy::default(),
            },
        )
        .expect("preflight response");
        assert!(!response.preflight.accepted);
        assert_eq!(response.preflight.missing_required, 1);
        assert_eq!(response.preflight.defaults_applied, 1);
        assert_eq!(response.preflight.unresolved_computed, 1);
    }

    #[test]
    fn preflight_returns_effective_context_for_valid_input() {
        let response = preflight_page_builder_runtime_context(
            PageBuilderRuntimePreflightRequest {
                project_data: project_data(),
                context: json!({ "page": { "title": "Welcome" } }),
                policy: RuntimeContextPreflightPolicy::default(),
            },
        )
        .expect("preflight response");
        assert!(response.preflight.accepted);
        assert_eq!(response.preflight.effective_context["shop"]["currency"], "EUR");
        assert_eq!(response.preflight.effective_context["page"]["label"], "EUR Welcome");
    }
}
