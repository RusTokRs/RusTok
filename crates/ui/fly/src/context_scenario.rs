use crate::{
    ProjectDocument, RuntimeContextPreflight, RuntimeContextPreflightPolicy, ValidationDiagnostic,
    ValidationSeverity, preflight_runtime_context,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextScenario {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub context: Value,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl RuntimeContextScenario {
    pub fn new(id: impl Into<String>, label: impl Into<String>, context: Value) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            context,
            extensions: Map::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextScenarioResult {
    pub scenario_id: String,
    pub scenario_label: String,
    pub preflight: RuntimeContextPreflight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RuntimeContextScenarioSuiteResult {
    pub results: Vec<RuntimeContextScenarioResult>,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub accepted: bool,
    pub accepted_count: usize,
    pub rejected_count: usize,
}

pub fn validate_runtime_context_scenarios(
    scenarios: &[RuntimeContextScenario],
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut ids = BTreeSet::new();
    for (index, scenario) in scenarios.iter().enumerate() {
        let path = format!("runtime_scenarios[{index}]");
        if scenario.id.trim().is_empty() {
            diagnostics.push(scenario_diagnostic(
                ValidationSeverity::Error,
                "runtime_scenario_id_empty",
                format!("{path}.id"),
                "runtime context scenario id must not be empty",
            ));
        } else if !ids.insert(scenario.id.clone()) {
            diagnostics.push(scenario_diagnostic(
                ValidationSeverity::Error,
                "duplicate_runtime_scenario_id",
                format!("{path}.id"),
                format!(
                    "runtime context scenario id `{}` is duplicated",
                    scenario.id
                ),
            ));
        }
        if scenario.label.trim().is_empty() {
            diagnostics.push(scenario_diagnostic(
                ValidationSeverity::Warning,
                "runtime_scenario_label_empty",
                format!("{path}.label"),
                format!(
                    "runtime context scenario `{}` has an empty label",
                    scenario.id
                ),
            ));
        }
        if !scenario.context.is_object() {
            diagnostics.push(scenario_diagnostic(
                ValidationSeverity::Warning,
                "runtime_scenario_context_not_object",
                format!("{path}.context"),
                format!(
                    "runtime context scenario `{}` uses a non-object root context",
                    scenario.id
                ),
            ));
        }
    }
    diagnostics
}

pub fn preflight_runtime_context_scenarios(
    document: &ProjectDocument,
    scenarios: &[RuntimeContextScenario],
    policy: RuntimeContextPreflightPolicy,
) -> RuntimeContextScenarioSuiteResult {
    let diagnostics = validate_runtime_context_scenarios(scenarios);
    let definitions_valid = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error);
    let results = scenarios
        .iter()
        .map(|scenario| RuntimeContextScenarioResult {
            scenario_id: scenario.id.clone(),
            scenario_label: scenario.label.clone(),
            preflight: preflight_runtime_context(document, &scenario.context, policy),
        })
        .collect::<Vec<_>>();
    let accepted_count = results
        .iter()
        .filter(|result| result.preflight.accepted)
        .count();
    let rejected_count = results.len().saturating_sub(accepted_count);
    RuntimeContextScenarioSuiteResult {
        accepted: definitions_valid && rejected_count == 0,
        results,
        diagnostics,
        accepted_count,
        rejected_count,
    }
}

fn scenario_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [{
                "id": "title",
                "path": "page.title",
                "kind": "string",
                "required": true
            }]
        }))
        .expect("document")
    }

    #[test]
    fn scenario_suite_preflights_each_host_owned_context() {
        let scenarios = vec![
            RuntimeContextScenario::new(
                "populated",
                "Populated",
                json!({ "page": { "title": "Welcome" } }),
            ),
            RuntimeContextScenario::new("empty", "Empty", json!({})),
        ];
        let suite = preflight_runtime_context_scenarios(
            &document(),
            &scenarios,
            RuntimeContextPreflightPolicy::default(),
        );
        assert!(!suite.accepted);
        assert_eq!(suite.accepted_count, 1);
        assert_eq!(suite.rejected_count, 1);
        assert!(suite.results[0].preflight.accepted);
        assert!(!suite.results[1].preflight.accepted);
    }

    #[test]
    fn duplicate_scenario_ids_are_rejected() {
        let scenarios = vec![
            RuntimeContextScenario::new("same", "One", json!({})),
            RuntimeContextScenario::new("same", "Two", json!({})),
        ];
        let diagnostics = validate_runtime_context_scenarios(&scenarios);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate_runtime_scenario_id")
        );
    }
}
