use fly::{
    FlyResult, GrapesJsCodec, RuntimeContextContract, RuntimeContextExample,
    RuntimeContextExamplePolicy, RuntimeContextJsonSchema, RuntimeContextPreflight,
    RuntimeContextPreflightPolicy, RuntimeContextScenario, RuntimePublishGateEvaluation,
    RuntimePublishGatePolicy, evaluate_runtime_publish_gate, export_runtime_context_json_schema,
    extract_runtime_context_contract, generate_runtime_context_example, preflight_runtime_context,
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
pub struct PageBuilderRuntimeJsonSchemaRequest {
    pub project_data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeJsonSchemaResponse {
    pub schema: RuntimeContextJsonSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeExampleRequest {
    pub project_data: Value,
    #[serde(default)]
    pub policy: RuntimeContextExamplePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeExampleResponse {
    pub example: RuntimeContextExample,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimePublishGateRequest {
    pub project_data: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    #[serde(default)]
    pub scenarios: Vec<RuntimeContextScenario>,
    #[serde(default)]
    pub policy: RuntimePublishGatePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimePublishGateResponse {
    pub evaluation: RuntimePublishGateEvaluation,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRuntimeContextInspector;

impl PageBuilderRuntimeContextInspector {
    pub fn contract(
        &self,
        request: PageBuilderRuntimeContractRequest,
    ) -> FlyResult<PageBuilderRuntimeContractResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeContractResponse {
            contract: extract_runtime_context_contract(&document),
        })
    }

    pub fn json_schema(
        &self,
        request: PageBuilderRuntimeJsonSchemaRequest,
    ) -> FlyResult<PageBuilderRuntimeJsonSchemaResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeJsonSchemaResponse {
            schema: export_runtime_context_json_schema(&document),
        })
    }

    pub fn example(
        &self,
        request: PageBuilderRuntimeExampleRequest,
    ) -> FlyResult<PageBuilderRuntimeExampleResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimeExampleResponse {
            example: generate_runtime_context_example(&document, request.policy),
        })
    }

    pub fn preflight(
        &self,
        request: PageBuilderRuntimePreflightRequest,
    ) -> FlyResult<PageBuilderRuntimePreflightResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimePreflightResponse {
            preflight: preflight_runtime_context(&document, &request.context, request.policy),
        })
    }

    pub fn publish_gate(
        &self,
        request: PageBuilderRuntimePublishGateRequest,
    ) -> FlyResult<PageBuilderRuntimePublishGateResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        Ok(PageBuilderRuntimePublishGateResponse {
            evaluation: evaluate_runtime_publish_gate(
                &document,
                request.context.as_ref(),
                &request.scenarios,
                &request.policy,
            ),
        })
    }
}

pub fn inspect_page_builder_runtime_contract(
    request: PageBuilderRuntimeContractRequest,
) -> FlyResult<PageBuilderRuntimeContractResponse> {
    PageBuilderRuntimeContextInspector.contract(request)
}

pub fn export_page_builder_runtime_json_schema(
    request: PageBuilderRuntimeJsonSchemaRequest,
) -> FlyResult<PageBuilderRuntimeJsonSchemaResponse> {
    PageBuilderRuntimeContextInspector.json_schema(request)
}

pub fn generate_page_builder_runtime_example(
    request: PageBuilderRuntimeExampleRequest,
) -> FlyResult<PageBuilderRuntimeExampleResponse> {
    PageBuilderRuntimeContextInspector.example(request)
}

pub fn preflight_page_builder_runtime_context(
    request: PageBuilderRuntimePreflightRequest,
) -> FlyResult<PageBuilderRuntimePreflightResponse> {
    PageBuilderRuntimeContextInspector.preflight(request)
}

pub fn evaluate_page_builder_runtime_publish_gate(
    request: PageBuilderRuntimePublishGateRequest,
) -> FlyResult<PageBuilderRuntimePublishGateResponse> {
    PageBuilderRuntimeContextInspector.publish_gate(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{CurrentContextGateMode, LandingReadinessPolicy, ScenarioGateMode};
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
    fn consumer_can_export_json_schema_and_generated_example() {
        let schema = export_page_builder_runtime_json_schema(PageBuilderRuntimeJsonSchemaRequest {
            project_data: project_data(),
        })
        .expect("json schema response");
        assert_eq!(schema.schema.schema["type"], "object");
        assert!(!schema.schema.contract_hash.is_empty());

        let example = generate_page_builder_runtime_example(PageBuilderRuntimeExampleRequest {
            project_data: project_data(),
            policy: RuntimeContextExamplePolicy::default(),
        })
        .expect("example response");
        assert_eq!(example.example.input_context["shop"]["currency"], "EUR");
        assert_eq!(example.example.effective_context["page"]["label"], "EUR ");
    }

    #[test]
    fn strict_preflight_rejects_missing_required_context() {
        let response = preflight_page_builder_runtime_context(PageBuilderRuntimePreflightRequest {
            project_data: project_data(),
            context: json!({}),
            policy: RuntimeContextPreflightPolicy::default(),
        })
        .expect("preflight response");
        assert!(!response.preflight.accepted);
        assert_eq!(response.preflight.missing_required, 1);
        assert_eq!(response.preflight.defaults_applied, 1);
        assert_eq!(response.preflight.unresolved_computed, 1);
    }

    #[test]
    fn preflight_returns_effective_context_for_valid_input() {
        let response = preflight_page_builder_runtime_context(PageBuilderRuntimePreflightRequest {
            project_data: project_data(),
            context: json!({ "page": { "title": "Welcome" } }),
            policy: RuntimeContextPreflightPolicy::default(),
        })
        .expect("preflight response");
        assert!(response.preflight.accepted);
        assert_eq!(
            response.preflight.effective_context["shop"]["currency"],
            "EUR"
        );
        assert_eq!(
            response.preflight.effective_context["page"]["label"],
            "EUR Welcome"
        );
    }

    #[test]
    fn consumer_can_enable_publish_only_landing_readiness() {
        let response =
            evaluate_page_builder_runtime_publish_gate(PageBuilderRuntimePublishGateRequest {
                project_data: project_data(),
                context: None,
                scenarios: Vec::new(),
                policy: RuntimePublishGatePolicy {
                    readiness: Some(LandingReadinessPolicy::default()),
                    ..RuntimePublishGatePolicy::default()
                },
            })
            .expect("publish gate response");
        assert!(!response.evaluation.allowed);
        assert!(
            response
                .evaluation
                .readiness
                .as_ref()
                .is_some_and(|report| !report.ready)
        );
    }

    #[test]
    fn consumer_publish_gate_matches_admin_runtime_policy() {
        let response =
            evaluate_page_builder_runtime_publish_gate(PageBuilderRuntimePublishGateRequest {
                project_data: project_data(),
                context: Some(json!({ "page": { "title": "Welcome" } })),
                scenarios: vec![RuntimeContextScenario::new(
                    "populated",
                    "Populated",
                    json!({ "page": { "title": "Welcome" } }),
                )],
                policy: RuntimePublishGatePolicy {
                    current_context: CurrentContextGateMode::RequireValid,
                    scenarios: ScenarioGateMode::All,
                    preflight: RuntimeContextPreflightPolicy::default(),
                    readiness: None,
                },
            })
            .expect("publish gate response");
        assert!(response.evaluation.allowed);
    }
}
