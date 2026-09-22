use crate::ProjectDocument;
use crate::{
    LandingReadinessPolicy, LandingReadinessReport, RuntimeContextPreflight,
    RuntimeContextPreflightPolicy, RuntimeContextScenario, RuntimeContextScenarioSuiteResult,
    ValidationDiagnostic, ValidationSeverity, evaluate_landing_readiness_with_context,
    extract_runtime_context_contract, preflight_runtime_context,
    preflight_runtime_context_scenarios,
};
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
    Named {
        scenario_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePublishGatePolicy {
    #[serde(default)]
    pub current_context: CurrentContextGateMode,
    #[serde(default)]
    pub scenarios: ScenarioGateMode,
    #[serde(default)]
    pub preflight: RuntimeContextPreflightPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<LandingReadinessPolicy>,
}

impl Default for RuntimePublishGatePolicy {
    fn default() -> Self {
        Self {
            current_context: CurrentContextGateMode::Ignore,
            scenarios: ScenarioGateMode::Ignore,
            preflight: RuntimeContextPreflightPolicy::default(),
            readiness: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimePublishGateEvaluation {
    pub allowed: bool,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub current_context: Option<RuntimeContextPreflight>,
    pub scenarios: Option<RuntimeContextScenarioSuiteResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<LandingReadinessReport>,
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
        CurrentContextGateMode::ValidateIfProvided => current_context
            .map(|context| preflight_runtime_context(document, context, policy.preflight)),
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
                let suite =
                    preflight_runtime_context_scenarios(document, scenarios, policy.preflight);
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
                let suite =
                    preflight_runtime_context_scenarios(document, scenarios, policy.preflight);
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
                    let suite =
                        preflight_runtime_context_scenarios(document, &selected, policy.preflight);
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

    let readiness_result = policy.readiness.map(|readiness_policy| {
        let report = evaluate_landing_readiness_with_context(
            document,
            current_context,
            readiness_policy,
        );
        diagnostics.extend(report.diagnostics().cloned());
        if !report.ready {
            let blocking_count = report.blocking_issues().count();
            diagnostics.push(gate_diagnostic(
                "runtime_publish_readiness_rejected",
                "project.readiness",
                format!(
                    "landing readiness policy rejected publish with {blocking_count} blocking issue(s)"
                ),
            ));
        }
        report
    });

    deduplicate_diagnostics(&mut diagnostics);
    let allowed = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error);
    RuntimePublishGateEvaluation {
        allowed,
        diagnostics,
        current_context: current_context_result,
        scenarios: scenario_result,
        readiness: readiness_result,
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
    use crate::{GrapesJsCodec, RuntimeContextScenario};
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

    fn ready_document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "Landing description",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "tagName": "main",
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome"
                    }]
                }
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
        assert!(
            evaluation
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_publish_context_rejected")
        );
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
            evaluation
                .scenarios
                .as_ref()
                .map(|suite| suite.rejected_count),
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
        assert!(
            evaluation
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "runtime_publish_named_scenario_missing" })
        );
    }

    #[test]
    fn readiness_is_opt_in_and_does_not_block_existing_gate_policies() {
        let evaluation = evaluate_runtime_publish_gate(
            &document(),
            None,
            &[],
            &RuntimePublishGatePolicy::default(),
        );
        assert!(evaluation.allowed);
        assert!(evaluation.readiness.is_none());
    }

    #[test]
    fn enabled_readiness_blocks_publish_only_when_landing_is_not_ready() {
        let evaluation = evaluate_runtime_publish_gate(
            &document(),
            None,
            &[],
            &RuntimePublishGatePolicy {
                readiness: Some(LandingReadinessPolicy::default()),
                ..RuntimePublishGatePolicy::default()
            },
        );
        assert!(!evaluation.allowed);
        assert!(
            evaluation
                .readiness
                .as_ref()
                .is_some_and(|report| !report.ready)
        );
        assert!(
            evaluation
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_publish_readiness_rejected")
        );
    }

    #[test]
    fn enabled_readiness_allows_a_stable_landing() {
        let document = ready_document();
        let original = document.clone();
        let evaluation = evaluate_runtime_publish_gate(
            &document,
            None,
            &[],
            &RuntimePublishGatePolicy {
                readiness: Some(LandingReadinessPolicy::default()),
                ..RuntimePublishGatePolicy::default()
            },
        );
        assert!(evaluation.allowed, "{:?}", evaluation.diagnostics);
        assert!(
            evaluation
                .readiness
                .as_ref()
                .is_some_and(|report| report.ready)
        );
        assert_eq!(document, original);
    }

    #[test]
    fn readiness_audits_the_publish_context_after_runtime_bindings() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "Landing description",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "tagName": "main",
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "tagName": "h1",
                        "content": ""
                    }]
                }
            }],
            "flyRuntimeContextSchema": [{
                "id": "heading-title",
                "path": "page.title",
                "kind": "string",
                "required": true
            }],
            "flyRuntimeBindings": [{
                "id": "heading-content",
                "component_id": "heading",
                "path": "page.title",
                "target": "field",
                "name": "content"
            }]
        }))
        .expect("document");
        let context = json!({ "page": { "title": "Context heading" } });
        let evaluation = evaluate_runtime_publish_gate(
            &document,
            Some(&context),
            &[],
            &RuntimePublishGatePolicy {
                readiness: Some(LandingReadinessPolicy::default()),
                ..RuntimePublishGatePolicy::default()
            },
        );
        assert!(evaluation.allowed, "{:?}", evaluation.diagnostics);
        assert!(!evaluation.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "landing_empty_heading" || diagnostic.code == "landing_missing_h1"
        }));
    }
}
