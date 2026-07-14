use crate::{
    extract_runtime_context_contract, preflight_runtime_context,
    preflight_runtime_context_scenarios, RuntimeContextPreflight,
    RuntimeContextPreflightPolicy, RuntimeContextScenario, RuntimeContextScenarioSuiteResult,
    ValidationDiagnostic, ValidationSeverity,
};
use crate::ProjectDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CurrentContextGateMode {
    #[default]
    Ignore,
    ValidateIfProvided,
    RequireValid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ScenarioGateMode {
    #[default]
    Ignore,
    All,
    Any,
    Named { scenario_ids: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePublishGatePolicy {
    #[serde(default)]
    pub current_context: CurrentContextGateMode,
    #[serde(default)]
    pub scenarios: ScenarioGateMode,
    #[serde(default)]
    pub preflight: RuntimeContextPreflightPolicy,
}

impl Default for RuntimePublishGatePolicy {
    fn default() -> Self {
        Self {
            current_context: CurrentContextGateMode::Ignore,
            scenarios: ScenarioGateMode::Ignore,
            preflight: RuntimeContextPreflightPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimePublishGateEvaluation {
    pub allowed: bool,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub current_context: Option<RuntimeContextPreflight>,
    pub scenarios: Option<RuntimeContextScenarioSuiteResult>,
}

pub fn evaluate_runtime_publish_gate(
    document: &ProjectDocument,
    current_context: Option<&Value>,
    scenarios: &[RuntimeContextScenario],
    policy: &RuntimePublishGatePolicy,
) -> RuntimePublishGateEvaluation {
    let contract = extract_runtime_context_contract(document);
    let mut diagnostics = contract.definition_diagnostics.clone();

    let current_context_result = match policy.current_context {
        CurrentContextGateMode::Ignore => None,
        CurrentContextGateMode::ValidateIfProvided => current_context.map(|context| {
            preflight_runtime_context(document, context, policy.preflight)
        }),
        CurrentContextGateMode::RequireValid => Some(match current_context {
            Some(context) => preflight_runtime_context(document, context, policy.preflight),
            None => {
                diagnostics.push(gate_diagnostic(
                    "runtime_publish_context_required",
                    "project.runtime.context",
                    "publish requires a runtime context but none was provided",
                ));
                preflight_runtime_context(document, &Value::Null, policy.preflight)
            }
        }),
    };
    if let Some(preflight) = current_context_result.as_ref() {
        diagnostics.extend(preflight.diagnostics.clone());
        if !preflight.accepted {
            diagnostics.push(gate_diagnostic(
                "runtime_publish_context_rejected",
                "project.runtime.context",
                "current runtime context failed publish preflight",
            ));
        }
    }

    let scenario_result = match &policy.scenarios {
        ScenarioGateMode::Ignore => None,
        ScenarioGateMode::All => {
            if scenarios.is_empty() {
                diagnostics.push(gate_diagnostic(
                    "runtime_publish_scenarios_required",
                    "project.runtime.scenarios",
                    "publish requires runtime scenarios but none were provided",
                ));
                None
            } else {
                let suite = preflight_runtime_context_scenarios(
                    document,
                    scenarios,
                    policy.preflight,
                );
                if !suite.accepted {
                    diagnostics.push(gate_diagnostic(
                        "runtime_publish_scenarios_rejected",
                        "project.runtime.scenarios",
                        format!(
                            "{} of {} runtime scenarios failed publish preflight",
                            suite.rejected_count,
                            suite.results.len()
                        ),
                    ));
                }
                Some(suite)
            }
        }
        ScenarioGateMode::Any => {
            if scenarios.is_empty() {
                diagnostics.push(gate_diagnostic(
                    "runtime_publish_scenarios_required",
                    "project.runtime.scenarios",
                    "publish requires at least one runtime scenario",
                ));
                None
            } else {
                let suite = preflight_runtime_context_scenarios(
                    document,
                    scenarios,
                    policy.preflight,
                );
                if suite.accepted_count == 0 {
                    diagnostics.push(gate_diagnostic(
                        "runtime_publish_no_scenario_accepted",
                        "project.runtime.scenarios",
                        "no runtime scenario passed publish preflight",
                    ));
                }
                Some(suite)
            }
        }
        ScenarioGateMode::Named { scenario_ids } => {
            let requested = scenario_ids.iter().cloned().collect::<BTreeSet<_>>();
            if requested.is_empty() {
                diagnostics.push(gate_diagnostic(
                    "runtime_publish_named_scenarios_empty",
                    "project.runtime.scenarios",
                    "named scenario publish gate has no scenario ids",
                ));
                None
            } else {
                let selected = scenarios
                    .iter()
                    .filter(|scenario| requested.contains(&scenario.id))
                    .cloned()
                    .collect::<Vec<_>>();
                let available = selected
                    .iter()
                    .map(|scenario| scenario.id.clone())
                    .collect::<BTreeSet<_>>();
                for missing in requested.difference(&available) {
                    diagnostics.push(gate_diagnostic(
                        "runtime_publish_named_scenario_missing",
                        "project.runtime.scenarios",
                        format!("required runtime scenario `{missing}` was not provided"),
                    ));
                }
                if selected.is_empty() {
                    None
                } else {
                    let suite = preflight_runtime_context_scenarios(
                        document,
                        &selected,
                        policy.preflight,
                    );
                    if !suite.accepted {
                        diagnostics.push(gate_diagnostic(
                            "runtime_publish_named_scenarios_rejected",
                            "project.runtime.scenarios",
                            format!(
                                "{} required runtime scenarios failed publish preflight",
                                suite.rejected_count
                            ),
                        ));
                    }
                    Some(suite)
                }
            }
        }
    };

    deduplicate_diagnostics(&mut diagnostics);
    let allowed = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error);
    RuntimePublishGateEvaluation {
        allowed,
        diagnostics,
        current_context: current_context_result,
        scenarios: scenario_result,
    }
}

fn deduplicate_diagnostics(diagnostics: &mut Vec<ValidationDiagnostic>) {
    let mut seen = BTreeSet::new();
    diagnostics.retain(|diagnostic| {
        seen.insert((
            diagnostic.severity as u8,
            diagnostic.code.clone(),
            diagnostic.path.clone(),
            diagnostic.message.clone(),
        ))
    });
}

fn gate_diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity: ValidationSeverity::Error,
        code: code.into(),
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsV1Codec, RuntimeContextScenario};
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
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
    fn current_context_gate_rejects_missing_required_data() {
        let evaluation = evaluate_runtime_publish_gate(
            &document(),
            Some(&json!({})),
            &[],
            &RuntimePublishGatePolicy {
                current_context: CurrentContextGateMode::RequireValid,
                ..RuntimePublishGatePolicy::default()
            },
        );
        assert!(!evaluation.allowed);
        assert!(evaluation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_publish_context_rejected"));
    }

    #[test]
    fn all_scenario_gate_requires_every_scenario_to_pass() {
        let scenarios = vec![
            RuntimeContextScenario::new(
                "valid",
                "Valid",
                json!({ "page": { "title": "Welcome" } }),
            ),
            RuntimeContextScenario::new("invalid", "Invalid", json!({})),
        ];
        let evaluation = evaluate_runtime_publish_gate(
            &document(),
            None,
            &scenarios,
            &RuntimePublishGatePolicy {
                scenarios: ScenarioGateMode::All,
                ..RuntimePublishGatePolicy::default()
            },
        );
        assert!(!evaluation.allowed);
        assert_eq!(
            evaluation.scenarios.as_ref().map(|suite| suite.rejected_count),
            Some(1)
        );
    }

    #[test]
    fn any_scenario_gate_allows_one_valid_scenario() {
        let scenarios = vec![
            RuntimeContextScenario::new("invalid", "Invalid", json!({})),
            RuntimeContextScenario::new(
                "valid",
                "Valid",
                json!({ "page": { "title": "Welcome" } }),
            ),
        ];
        let evaluation = evaluate_runtime_publish_gate(
            &document(),
            None,
            &scenarios,
            &RuntimePublishGatePolicy {
                scenarios: ScenarioGateMode::Any,
                ..RuntimePublishGatePolicy::default()
            },
        );
        assert!(evaluation.allowed);
    }

    #[test]
    fn named_gate_rejects_missing_required_scenario() {
        let evaluation = evaluate_runtime_publish_gate(
            &document(),
            None,
            &[RuntimeContextScenario::new(
                "present",
                "Present",
                json!({ "page": { "title": "Welcome" } }),
            )],
            &RuntimePublishGatePolicy {
                scenarios: ScenarioGateMode::Named {
                    scenario_ids: vec!["missing".to_string()],
                },
                ..RuntimePublishGatePolicy::default()
            },
        );
        assert!(!evaluation.allowed);
        assert!(evaluation.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "runtime_publish_named_scenario_missing"
        }));
    }
}
