use crate::{
    ComponentAction, ComponentForm, ContextValueKind, FLY_ACTION_FIELD, FLY_FORM_FIELD, FlyError,
    FlyResult, ProjectDocument, ValidationDiagnostic, ValidationSeverity, visit_project_components,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InteractionCapabilityKind {
    Action,
    Form,
}

impl InteractionCapabilityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Form => "form",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionRuntimeTarget {
    Browser,
    Server,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractionCapabilityDefinition {
    pub provider: String,
    pub operation: String,
    pub kind: InteractionCapabilityKind,
    pub runtime: InteractionRuntimeTarget,
    #[serde(default)]
    pub input_kind: ContextValueKind,
    #[serde(default)]
    pub description: String,
}

impl InteractionCapabilityDefinition {
    pub fn normalized(mut self) -> FlyResult<Self> {
        self.provider = normalize_identifier(&self.provider, "provider")?;
        self.operation = normalize_identifier(&self.operation, "operation")?;
        self.description = self.description.trim().to_string();
        Ok(self)
    }

    pub fn id(&self) -> String {
        capability_id(self.kind, &self.provider, &self.operation)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct InteractionCapabilityRegistry {
    items: BTreeMap<String, InteractionCapabilityDefinition>,
}

impl InteractionCapabilityRegistry {
    pub fn register(&mut self, definition: InteractionCapabilityDefinition) -> FlyResult<()> {
        let definition = definition.normalized()?;
        let id = definition.id();
        if self.items.contains_key(&id) {
            return Err(FlyError::DuplicateRegistryItem(id));
        }
        self.items.insert(id, definition);
        Ok(())
    }

    pub fn get(
        &self,
        kind: InteractionCapabilityKind,
        provider: &str,
        operation: &str,
    ) -> Option<&InteractionCapabilityDefinition> {
        self.items
            .get(&capability_id(kind, provider.trim(), operation.trim()))
    }

    pub fn supports(
        &self,
        kind: InteractionCapabilityKind,
        provider: &str,
        operation: &str,
    ) -> bool {
        self.get(kind, provider, operation).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &InteractionCapabilityDefinition)> {
        self.items
            .iter()
            .map(|(id, definition)| (id.as_str(), definition))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MissingInteractionCapabilityPolicy {
    #[default]
    Allow,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InteractionCapabilityPolicy {
    #[serde(default)]
    pub provider_actions: MissingInteractionCapabilityPolicy,
    #[serde(default)]
    pub provider_forms: MissingInteractionCapabilityPolicy,
}

struct InteractionCapabilityUse<'a> {
    kind: InteractionCapabilityKind,
    provider: &'a str,
    operation: &'a str,
    input: &'a Value,
    missing_policy: MissingInteractionCapabilityPolicy,
    component_id: Option<&'a str>,
    canonical_path: &'a str,
}

pub fn validate_interaction_capabilities(
    document: &ProjectDocument,
    registry: &InteractionCapabilityRegistry,
    policy: InteractionCapabilityPolicy,
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();
    visit_project_components(&document.project, |component, visit| {
        if let Some(raw) = component.extensions.get(FLY_ACTION_FIELD).cloned() {
            if let Ok(ComponentAction::ProviderAction {
                provider,
                action,
                input,
            }) = serde_json::from_value::<ComponentAction>(raw)
            {
                validate_provider_interaction(
                    registry,
                    InteractionCapabilityUse {
                        kind: InteractionCapabilityKind::Action,
                        provider: &provider,
                        operation: &action,
                        input: &input,
                        missing_policy: policy.provider_actions,
                        component_id: component.id.as_deref(),
                        canonical_path: visit.path(),
                    },
                    &mut diagnostics,
                );
            }
        }

        if let Some(raw) = component.extensions.get(FLY_FORM_FIELD).cloned() {
            if let Ok(form) = serde_json::from_value::<ComponentForm>(raw) {
                if let (Some(provider), Some(action)) =
                    (form.provider.as_deref(), form.action.as_deref())
                {
                    validate_provider_interaction(
                        registry,
                        InteractionCapabilityUse {
                            kind: InteractionCapabilityKind::Form,
                            provider,
                            operation: action,
                            input: &form.input,
                            missing_policy: policy.provider_forms,
                            component_id: component.id.as_deref(),
                            canonical_path: visit.path(),
                        },
                        &mut diagnostics,
                    );
                }
            }
        }
    });
    diagnostics
}

pub fn validate_component_actions_with_capabilities(
    document: &ProjectDocument,
    registry: &InteractionCapabilityRegistry,
    policy: InteractionCapabilityPolicy,
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = crate::validate_component_actions(document);
    diagnostics.extend(validate_interaction_capabilities(
        document, registry, policy,
    ));
    diagnostics
}

fn validate_provider_interaction(
    registry: &InteractionCapabilityRegistry,
    interaction: InteractionCapabilityUse<'_>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    let Some(capability) = registry.get(
        interaction.kind,
        interaction.provider,
        interaction.operation,
    ) else {
        let severity = match interaction.missing_policy {
            MissingInteractionCapabilityPolicy::Allow => return,
            MissingInteractionCapabilityPolicy::Warn => ValidationSeverity::Warning,
            MissingInteractionCapabilityPolicy::Error => ValidationSeverity::Error,
        };
        diagnostics.push(capability_diagnostic(
            severity,
            "interaction_capability_missing",
            interaction.component_id,
            interaction.canonical_path,
            format!(
                "{} capability `{}.{}` is not registered",
                interaction.kind.as_str(),
                interaction.provider.trim(),
                interaction.operation.trim()
            ),
        ));
        return;
    };

    if !capability.input_kind.accepts(interaction.input) {
        diagnostics.push(capability_diagnostic(
            ValidationSeverity::Error,
            "interaction_capability_input_kind_mismatch",
            interaction.component_id,
            interaction.canonical_path,
            format!(
                "{} capability `{}` expects {} input, received {}",
                interaction.kind.as_str(),
                capability.id(),
                capability.input_kind.as_str(),
                value_kind_name(interaction.input)
            ),
        ));
    }
}

fn capability_id(kind: InteractionCapabilityKind, provider: &str, operation: &str) -> String {
    format!("{}:{provider}:{operation}", kind.as_str())
}

fn normalize_identifier(value: &str, label: &str) -> FlyResult<String> {
    let value = value.trim();
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
        })
    {
        return Err(FlyError::InvalidInteractionCapability(format!(
            "{label} `{value}` contains unsupported characters"
        )));
    }
    Ok(value.to_string())
}

fn value_kind_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn capability_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    component_id: Option<&str>,
    canonical_path: &str,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: component_id
            .map(|id| format!("component:{id}.interactionCapability"))
            .unwrap_or_else(|| format!("{canonical_path}.interactionCapability")),
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
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "lead-action",
                        "type": "button",
                        "flyAction": {
                            "kind": "provider_action",
                            "provider": "crm",
                            "action": "create_lead",
                            "input": { "source": "hero" }
                        }
                    }, {
                        "id": "lead-form",
                        "type": "form",
                        "flyForm": {
                            "id": "lead",
                            "method": "post",
                            "provider": "crm",
                            "action": "submit_lead",
                            "input": { "source": "form" }
                        }
                    }]
                }
            }]
        }))
        .expect("document")
    }

    fn registry() -> InteractionCapabilityRegistry {
        let mut registry = InteractionCapabilityRegistry::default();
        registry
            .register(InteractionCapabilityDefinition {
                provider: "crm".to_string(),
                operation: "create_lead".to_string(),
                kind: InteractionCapabilityKind::Action,
                runtime: InteractionRuntimeTarget::Hybrid,
                input_kind: ContextValueKind::Object,
                description: String::new(),
            })
            .unwrap();
        registry
    }

    #[test]
    fn strict_policy_rejects_unregistered_provider_forms() {
        let diagnostics = validate_interaction_capabilities(
            &document(),
            &registry(),
            InteractionCapabilityPolicy {
                provider_actions: MissingInteractionCapabilityPolicy::Error,
                provider_forms: MissingInteractionCapabilityPolicy::Error,
            },
        );
        assert!(!diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == ValidationSeverity::Error
                && diagnostic.path.starts_with("component:lead-action")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "interaction_capability_missing"
                && diagnostic.path.starts_with("component:lead-form")
        }));
    }

    #[test]
    fn capability_input_kind_is_validated() {
        let mut document = document();
        document.component_mut("lead-action").unwrap().extensions[FLY_ACTION_FIELD]["input"] =
            json!("not-an-object");
        let diagnostics = validate_interaction_capabilities(
            &document,
            &registry(),
            InteractionCapabilityPolicy {
                provider_actions: MissingInteractionCapabilityPolicy::Error,
                provider_forms: MissingInteractionCapabilityPolicy::Allow,
            },
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "interaction_capability_input_kind_mismatch"
                && diagnostic.severity == ValidationSeverity::Error
        }));
    }

    #[test]
    fn permissive_policy_preserves_unknown_provider_compatibility() {
        assert!(
            validate_interaction_capabilities(
                &document(),
                &InteractionCapabilityRegistry::default(),
                InteractionCapabilityPolicy::default(),
            )
            .is_empty()
        );
    }

    #[test]
    fn duplicate_capability_ids_are_rejected() {
        let mut registry = registry();
        let error = registry
            .register(InteractionCapabilityDefinition {
                provider: "crm".to_string(),
                operation: "create_lead".to_string(),
                kind: InteractionCapabilityKind::Action,
                runtime: InteractionRuntimeTarget::Server,
                input_kind: ContextValueKind::Any,
                description: String::new(),
            })
            .expect_err("duplicate");
        assert!(matches!(error, FlyError::DuplicateRegistryItem(_)));
    }

    #[test]
    fn invalid_capability_identifier_has_domain_error() {
        let mut registry = InteractionCapabilityRegistry::default();
        assert!(matches!(
            registry.register(InteractionCapabilityDefinition {
                provider: "bad provider".to_string(),
                operation: "run".to_string(),
                kind: InteractionCapabilityKind::Action,
                runtime: InteractionRuntimeTarget::Browser,
                input_kind: ContextValueKind::Any,
                description: String::new(),
            }),
            Err(FlyError::InvalidInteractionCapability(_))
        ));
    }
}
