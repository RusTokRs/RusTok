use crate::{
    context_expression_dependencies, export_runtime_context_json_schema, ComputedContextValue,
    ContextFieldDefinition, ContextSchemaCatalog, ContextValueKind, ProjectDocument,
    ValidationDiagnostic, ValidationSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_RUNTIME_CONTEXT_CONTRACT_V1: &str = "fly_runtime_context_contract_v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContractChangeImpact {
    NonBreaking,
    Behavioral,
    Breaking,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeContractCompatibility {
    Compatible,
    RequiresReview,
    Breaking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextContractSnapshot {
    pub format: String,
    pub contract_hash: String,
    pub fields: Vec<ContextFieldDefinition>,
    pub computed: Vec<ComputedContextValue>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl RuntimeContextContractSnapshot {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let catalog = ContextSchemaCatalog::from_document(document);
        let schema = export_runtime_context_json_schema(document);
        Self {
            format: FLY_RUNTIME_CONTEXT_CONTRACT_V1.to_string(),
            contract_hash: schema.contract_hash,
            fields: catalog.fields,
            computed: catalog.computed,
            diagnostics: schema.diagnostics,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum RuntimeContextContractChange {
    FieldAdded {
        impact: ContractChangeImpact,
        path: String,
        required: bool,
        has_default: bool,
    },
    FieldRemoved {
        impact: ContractChangeImpact,
        path: String,
    },
    FieldTypeChanged {
        impact: ContractChangeImpact,
        path: String,
        previous: ContextValueKind,
        next: ContextValueKind,
    },
    FieldItemTypeChanged {
        impact: ContractChangeImpact,
        path: String,
        previous: Option<ContextValueKind>,
        next: Option<ContextValueKind>,
    },
    FieldRequiredChanged {
        impact: ContractChangeImpact,
        path: String,
        previous: bool,
        next: bool,
        has_default: bool,
    },
    FieldDefaultChanged {
        impact: ContractChangeImpact,
        path: String,
        previous: Option<Value>,
        next: Option<Value>,
    },
    ComputedAdded {
        impact: ContractChangeImpact,
        path: String,
    },
    ComputedRemoved {
        impact: ContractChangeImpact,
        path: String,
    },
    ComputedExpressionChanged {
        impact: ContractChangeImpact,
        path: String,
    },
    ComputedDependenciesChanged {
        impact: ContractChangeImpact,
        path: String,
        previous: Vec<String>,
        next: Vec<String>,
    },
    ComputedFallbackChanged {
        impact: ContractChangeImpact,
        path: String,
        previous: Option<Value>,
        next: Option<Value>,
    },
}

impl RuntimeContextContractChange {
    pub const fn impact(&self) -> ContractChangeImpact {
        match self {
            Self::FieldAdded { impact, .. }
            | Self::FieldRemoved { impact, .. }
            | Self::FieldTypeChanged { impact, .. }
            | Self::FieldItemTypeChanged { impact, .. }
            | Self::FieldRequiredChanged { impact, .. }
            | Self::FieldDefaultChanged { impact, .. }
            | Self::ComputedAdded { impact, .. }
            | Self::ComputedRemoved { impact, .. }
            | Self::ComputedExpressionChanged { impact, .. }
            | Self::ComputedDependenciesChanged { impact, .. }
            | Self::ComputedFallbackChanged { impact, .. } => *impact,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::FieldAdded { path, .. }
            | Self::FieldRemoved { path, .. }
            | Self::FieldTypeChanged { path, .. }
            | Self::FieldItemTypeChanged { path, .. }
            | Self::FieldRequiredChanged { path, .. }
            | Self::FieldDefaultChanged { path, .. }
            | Self::ComputedAdded { path, .. }
            | Self::ComputedRemoved { path, .. }
            | Self::ComputedExpressionChanged { path, .. }
            | Self::ComputedDependenciesChanged { path, .. }
            | Self::ComputedFallbackChanged { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeContextMigrationHintKind {
    SupplyRequiredValue,
    ReviewTypeConversion,
    ReviewBehaviorChange,
    RemoveDeprecatedUsage,
    DefaultWillBeApplied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextMigrationHint {
    pub kind: RuntimeContextMigrationHintKind,
    pub path: String,
    pub message: String,
    pub automatic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextContractDiff {
    pub previous_hash: String,
    pub next_hash: String,
    pub compatibility: RuntimeContractCompatibility,
    pub changes: Vec<RuntimeContextContractChange>,
    pub migration_hints: Vec<RuntimeContextMigrationHint>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl RuntimeContextContractDiff {
    pub fn is_unchanged(&self) -> bool {
        self.previous_hash == self.next_hash && self.changes.is_empty()
    }

    pub fn breaking_changes(&self) -> impl Iterator<Item = &RuntimeContextContractChange> {
        self.changes
            .iter()
            .filter(|change| change.impact() == ContractChangeImpact::Breaking)
    }
}

pub fn diff_runtime_context_contracts(
    previous: &RuntimeContextContractSnapshot,
    next: &RuntimeContextContractSnapshot,
) -> RuntimeContextContractDiff {
    let mut changes = Vec::new();
    let previous_fields = fields_by_path(&previous.fields);
    let next_fields = fields_by_path(&next.fields);
    let previous_computed = computed_by_path(&previous.computed);
    let next_computed = computed_by_path(&next.computed);

    for (path, field) in &previous_fields {
        let Some(next_field) = next_fields.get(path) else {
            changes.push(RuntimeContextContractChange::FieldRemoved {
                impact: ContractChangeImpact::Breaking,
                path: path.clone(),
            });
            continue;
        };
        if field.kind != next_field.kind {
            changes.push(RuntimeContextContractChange::FieldTypeChanged {
                impact: classify_type_change(field.kind, next_field.kind),
                path: path.clone(),
                previous: field.kind,
                next: next_field.kind,
            });
        }
        if field.item_kind != next_field.item_kind {
            changes.push(RuntimeContextContractChange::FieldItemTypeChanged {
                impact: ContractChangeImpact::Breaking,
                path: path.clone(),
                previous: field.item_kind,
                next: next_field.item_kind,
            });
        }
        if field.required != next_field.required {
            changes.push(RuntimeContextContractChange::FieldRequiredChanged {
                impact: if next_field.required && next_field.default.is_none() {
                    ContractChangeImpact::Breaking
                } else {
                    ContractChangeImpact::Behavioral
                },
                path: path.clone(),
                previous: field.required,
                next: next_field.required,
                has_default: next_field.default.is_some(),
            });
        }
        if field.default != next_field.default {
            changes.push(RuntimeContextContractChange::FieldDefaultChanged {
                impact: ContractChangeImpact::Behavioral,
                path: path.clone(),
                previous: field.default.clone(),
                next: next_field.default.clone(),
            });
        }
    }

    for (path, field) in &next_fields {
        if previous_fields.contains_key(path) {
            continue;
        }
        let impact = if field.required && field.default.is_none() {
            ContractChangeImpact::Breaking
        } else if field.default.is_some() {
            ContractChangeImpact::Behavioral
        } else {
            ContractChangeImpact::NonBreaking
        };
        changes.push(RuntimeContextContractChange::FieldAdded {
            impact,
            path: path.clone(),
            required: field.required,
            has_default: field.default.is_some(),
        });
    }

    for (path, computed) in &previous_computed {
        let Some(next_computed_value) = next_computed.get(path) else {
            changes.push(RuntimeContextContractChange::ComputedRemoved {
                impact: ContractChangeImpact::Breaking,
                path: path.clone(),
            });
            continue;
        };
        if computed.expression != next_computed_value.expression {
            changes.push(RuntimeContextContractChange::ComputedExpressionChanged {
                impact: ContractChangeImpact::Behavioral,
                path: path.clone(),
            });
        }
        let previous_dependencies = context_expression_dependencies(&computed.expression);
        let next_dependencies = context_expression_dependencies(&next_computed_value.expression);
        if previous_dependencies != next_dependencies {
            changes.push(RuntimeContextContractChange::ComputedDependenciesChanged {
                impact: ContractChangeImpact::Behavioral,
                path: path.clone(),
                previous: previous_dependencies,
                next: next_dependencies,
            });
        }
        if computed.fallback != next_computed_value.fallback {
            changes.push(RuntimeContextContractChange::ComputedFallbackChanged {
                impact: ContractChangeImpact::Behavioral,
                path: path.clone(),
                previous: computed.fallback.clone(),
                next: next_computed_value.fallback.clone(),
            });
        }
    }

    for path in next_computed.keys() {
        if !previous_computed.contains_key(path) {
            changes.push(RuntimeContextContractChange::ComputedAdded {
                impact: ContractChangeImpact::NonBreaking,
                path: path.clone(),
            });
        }
    }

    changes.sort_by(|left, right| {
        left.path()
            .cmp(right.path())
            .then_with(|| left.impact().cmp(&right.impact()))
    });
    let compatibility = classify_compatibility(&changes);
    let migration_hints = migration_hints(&changes);
    let mut diagnostics = Vec::new();
    diagnostics.extend(previous.diagnostics.clone());
    diagnostics.extend(next.diagnostics.clone());
    deduplicate_diagnostics(&mut diagnostics);

    RuntimeContextContractDiff {
        previous_hash: previous.contract_hash.clone(),
        next_hash: next.contract_hash.clone(),
        compatibility,
        changes,
        migration_hints,
        diagnostics,
    }
}

pub fn diff_runtime_context_documents(
    previous: &ProjectDocument,
    next: &ProjectDocument,
) -> RuntimeContextContractDiff {
    diff_runtime_context_contracts(
        &RuntimeContextContractSnapshot::from_document(previous),
        &RuntimeContextContractSnapshot::from_document(next),
    )
}

fn fields_by_path(
    fields: &[ContextFieldDefinition],
) -> BTreeMap<String, ContextFieldDefinition> {
    fields
        .iter()
        .cloned()
        .map(|field| (field.path.clone(), field))
        .collect()
}

fn computed_by_path(
    computed: &[ComputedContextValue],
) -> BTreeMap<String, ComputedContextValue> {
    computed
        .iter()
        .cloned()
        .map(|value| (value.path.clone(), value))
        .collect()
}

fn classify_type_change(
    previous: ContextValueKind,
    next: ContextValueKind,
) -> ContractChangeImpact {
    if previous == next {
        ContractChangeImpact::NonBreaking
    } else if previous == ContextValueKind::Any {
        ContractChangeImpact::Breaking
    } else if next == ContextValueKind::Any {
        ContractChangeImpact::NonBreaking
    } else {
        ContractChangeImpact::Breaking
    }
}

fn classify_compatibility(
    changes: &[RuntimeContextContractChange],
) -> RuntimeContractCompatibility {
    if changes
        .iter()
        .any(|change| change.impact() == ContractChangeImpact::Breaking)
    {
        RuntimeContractCompatibility::Breaking
    } else if changes
        .iter()
        .any(|change| change.impact() == ContractChangeImpact::Behavioral)
    {
        RuntimeContractCompatibility::RequiresReview
    } else {
        RuntimeContractCompatibility::Compatible
    }
}

fn migration_hints(
    changes: &[RuntimeContextContractChange],
) -> Vec<RuntimeContextMigrationHint> {
    let mut hints = Vec::new();
    for change in changes {
        match change {
            RuntimeContextContractChange::FieldAdded {
                path,
                required: true,
                has_default: false,
                ..
            }
            | RuntimeContextContractChange::FieldRequiredChanged {
                path,
                next: true,
                has_default: false,
                ..
            } => hints.push(RuntimeContextMigrationHint {
                kind: RuntimeContextMigrationHintKind::SupplyRequiredValue,
                path: path.clone(),
                message: format!("supply a runtime value for newly required path `{path}`"),
                automatic: false,
            }),
            RuntimeContextContractChange::FieldAdded {
                path,
                has_default: true,
                ..
            }
            | RuntimeContextContractChange::FieldRequiredChanged {
                path,
                has_default: true,
                ..
            } => hints.push(RuntimeContextMigrationHint {
                kind: RuntimeContextMigrationHintKind::DefaultWillBeApplied,
                path: path.clone(),
                message: format!("the new default will populate `{path}` when missing"),
                automatic: true,
            }),
            RuntimeContextContractChange::FieldTypeChanged { path, .. }
            | RuntimeContextContractChange::FieldItemTypeChanged { path, .. } => {
                hints.push(RuntimeContextMigrationHint {
                    kind: RuntimeContextMigrationHintKind::ReviewTypeConversion,
                    path: path.clone(),
                    message: format!("review existing runtime values for type conversion at `{path}`"),
                    automatic: false,
                })
            }
            RuntimeContextContractChange::FieldRemoved { path, .. }
            | RuntimeContextContractChange::ComputedRemoved { path, .. } => {
                hints.push(RuntimeContextMigrationHint {
                    kind: RuntimeContextMigrationHintKind::RemoveDeprecatedUsage,
                    path: path.clone(),
                    message: format!("remove consumers that still depend on `{path}`"),
                    automatic: false,
                })
            }
            RuntimeContextContractChange::FieldDefaultChanged { path, .. }
            | RuntimeContextContractChange::ComputedExpressionChanged { path, .. }
            | RuntimeContextContractChange::ComputedDependenciesChanged { path, .. }
            | RuntimeContextContractChange::ComputedFallbackChanged { path, .. } => {
                hints.push(RuntimeContextMigrationHint {
                    kind: RuntimeContextMigrationHintKind::ReviewBehaviorChange,
                    path: path.clone(),
                    message: format!("review changed runtime behavior at `{path}`"),
                    automatic: false,
                })
            }
            RuntimeContextContractChange::FieldAdded { .. }
            | RuntimeContextContractChange::FieldRequiredChanged { .. }
            | RuntimeContextContractChange::ComputedAdded { .. } => {}
        }
    }
    hints
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    fn document(schema: Value, computed: Value) -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": schema,
            "flyRuntimeComputed": computed
        }))
        .expect("document")
    }

    #[test]
    fn required_field_without_default_is_breaking() {
        let previous = document(json!([]), json!([]));
        let next = document(
            json!([{
                "id": "title",
                "path": "page.title",
                "kind": "string",
                "required": true
            }]),
            json!([]),
        );
        let diff = diff_runtime_context_documents(&previous, &next);
        assert_eq!(diff.compatibility, RuntimeContractCompatibility::Breaking);
        assert!(matches!(
            diff.changes.as_slice(),
            [RuntimeContextContractChange::FieldAdded {
                impact: ContractChangeImpact::Breaking,
                ..
            }]
        ));
        assert!(diff.migration_hints.iter().any(|hint| {
            hint.kind == RuntimeContextMigrationHintKind::SupplyRequiredValue
        }));
    }

    #[test]
    fn optional_field_is_non_breaking_and_default_is_behavioral() {
        let previous = document(json!([]), json!([]));
        let optional = document(
            json!([{
                "id": "subtitle",
                "path": "page.subtitle",
                "kind": "string"
            }]),
            json!([]),
        );
        let with_default = document(
            json!([{
                "id": "subtitle",
                "path": "page.subtitle",
                "kind": "string",
                "default": "Default"
            }]),
            json!([]),
        );
        assert_eq!(
            diff_runtime_context_documents(&previous, &optional).compatibility,
            RuntimeContractCompatibility::Compatible
        );
        assert_eq!(
            diff_runtime_context_documents(&previous, &with_default).compatibility,
            RuntimeContractCompatibility::RequiresReview
        );
    }

    #[test]
    fn computed_expression_change_requires_review() {
        let previous = document(
            json!([]),
            json!([{
                "id": "label",
                "path": "page.label",
                "expression": { "op": "literal", "value": "One" }
            }]),
        );
        let next = document(
            json!([]),
            json!([{
                "id": "label",
                "path": "page.label",
                "expression": { "op": "literal", "value": "Two" }
            }]),
        );
        let diff = diff_runtime_context_documents(&previous, &next);
        assert_eq!(
            diff.compatibility,
            RuntimeContractCompatibility::RequiresReview
        );
        assert!(diff.changes.iter().any(|change| matches!(
            change,
            RuntimeContextContractChange::ComputedExpressionChanged { .. }
        )));
    }

    #[test]
    fn identical_snapshots_have_no_changes() {
        let document = document(
            json!([{
                "id": "title",
                "path": "page.title",
                "kind": "string"
            }]),
            json!([]),
        );
        let snapshot = RuntimeContextContractSnapshot::from_document(&document);
        let diff = diff_runtime_context_contracts(&snapshot, &snapshot);
        assert!(diff.is_unchanged());
        assert_eq!(diff.compatibility, RuntimeContractCompatibility::Compatible);
    }
}
