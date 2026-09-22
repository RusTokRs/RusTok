use crate::{
    InteractionCapabilityPolicy, InteractionCapabilityRegistry, ProjectDocument,
    RuntimeContextScenario, RuntimePublishGateEvaluation, RuntimePublishGatePolicy,
    ValidationDiagnostic, ValidationSeverity, evaluate_runtime_publish_gate,
    validate_interaction_capabilities,
};
use serde_json::Value;
use std::collections::BTreeSet;

/// Evaluates the normal publish gate and then applies host-provided provider capabilities.
///
/// Capability registration is deliberately external to the project document. Draft editing and
/// lossless GrapesJS round-tripping remain permissive, while a production host can make missing
/// provider actions/forms blocking at publish time.
pub fn evaluate_runtime_publish_gate_with_capabilities(
    document: &ProjectDocument,
    current_context: Option<&Value>,
    scenarios: &[RuntimeContextScenario],
    gate_policy: &RuntimePublishGatePolicy,
    capability_registry: &InteractionCapabilityRegistry,
    capability_policy: InteractionCapabilityPolicy,
) -> RuntimePublishGateEvaluation {
    let mut evaluation =
        evaluate_runtime_publish_gate(document, current_context, scenarios, gate_policy);
    evaluation
        .diagnostics
        .extend(validate_interaction_capabilities(
            document,
            capability_registry,
            capability_policy,
        ));
    deduplicate_capability_gate_diagnostics(&mut evaluation.diagnostics);
    evaluation.allowed = !evaluation
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error);
    evaluation
}

fn deduplicate_capability_gate_diagnostics(diagnostics: &mut Vec<ValidationDiagnostic>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContextValueKind, GrapesJsCodec, InteractionCapabilityDefinition,
        InteractionCapabilityKind, InteractionRuntimeTarget, MissingInteractionCapabilityPolicy,
    };
    use serde_json::json;

    fn document() -> ProjectDocument {
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
                    }, {
                        "id": "lead",
                        "type": "button",
                        "flyAction": {
                            "kind": "provider_action",
                            "provider": "crm",
                            "action": "create_lead",
                            "input": { "source": "hero" }
                        }
                    }]
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn strict_capability_policy_blocks_publish_without_blocking_base_gate() {
        let document = document();
        let base = evaluate_runtime_publish_gate(
            &document,
            None,
            &[],
            &RuntimePublishGatePolicy::default(),
        );
        assert!(base.allowed);

        let strict = evaluate_runtime_publish_gate_with_capabilities(
            &document,
            None,
            &[],
            &RuntimePublishGatePolicy::default(),
            &InteractionCapabilityRegistry::default(),
            InteractionCapabilityPolicy {
                provider_actions: MissingInteractionCapabilityPolicy::Error,
                provider_forms: MissingInteractionCapabilityPolicy::Allow,
            },
        );
        assert!(!strict.allowed);
        assert!(strict.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "interaction_capability_missing"
                && diagnostic.severity == ValidationSeverity::Error
        }));
    }

    #[test]
    fn registered_capability_allows_publish() {
        let mut registry = InteractionCapabilityRegistry::default();
        registry
            .register(InteractionCapabilityDefinition {
                provider: "crm".to_string(),
                operation: "create_lead".to_string(),
                kind: InteractionCapabilityKind::Action,
                runtime: InteractionRuntimeTarget::Server,
                input_kind: ContextValueKind::Object,
                description: String::new(),
            })
            .unwrap();
        let evaluation = evaluate_runtime_publish_gate_with_capabilities(
            &document(),
            None,
            &[],
            &RuntimePublishGatePolicy::default(),
            &registry,
            InteractionCapabilityPolicy {
                provider_actions: MissingInteractionCapabilityPolicy::Error,
                provider_forms: MissingInteractionCapabilityPolicy::Error,
            },
        );
        assert!(evaluation.allowed);
    }
}
