use crate::{
    diff_runtime_context_contracts, preflight_runtime_context, resolve_context_path,
    set_context_path, ContextFieldDefinition, ContextSchemaCatalog, ContextValueKind,
    ProjectDocument, RuntimeContextContractDiff, RuntimeContextContractSnapshot,
    RuntimeContextPreflightPolicy, ValidationDiagnostic, ValidationSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextMigrationPolicy {
    pub apply_defaults: bool,
    pub coerce_scalars: bool,
    pub synthesize_required_values: bool,
    pub retain_removed_paths: bool,
    pub preflight: RuntimeContextPreflightPolicy,
}

impl Default for RuntimeContextMigrationPolicy {
    fn default() -> Self {
        Self {
            apply_defaults: true,
            coerce_scalars: false,
            synthesize_required_values: false,
            retain_removed_paths: true,
            preflight: RuntimeContextPreflightPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeContextMigrationOperationKind {
    Preserved,
    DefaultApplied,
    ScalarCoerced,
    RequiredValueSynthesized,
    RemovedPathRetained,
    MissingRequired,
    TypeMismatch,
    WriteFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextMigrationOperation {
    pub kind: RuntimeContextMigrationOperationKind,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Value>,
    pub automatic: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextMigrationResult {
    pub migrated_context: Value,
    pub effective_context: Value,
    pub accepted: bool,
    pub diff: RuntimeContextContractDiff,
    pub operations: Vec<RuntimeContextMigrationOperation>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

pub fn migrate_runtime_context(
    previous_contract: &RuntimeContextContractSnapshot,
    next_document: &ProjectDocument,
    input_context: &Value,
    policy: RuntimeContextMigrationPolicy,
) -> RuntimeContextMigrationResult {
    let next_contract = RuntimeContextContractSnapshot::from_document(next_document);
    let diff = diff_runtime_context_contracts(previous_contract, &next_contract);
    let next_catalog = ContextSchemaCatalog::from_document(next_document);
    let mut migrated_context = normalize_context(input_context.clone());
    let mut operations = Vec::new();
    let mut diagnostics = Vec::new();

    for field in &next_catalog.fields {
        migrate_field(
            field,
            &mut migrated_context,
            policy,
            &mut operations,
            &mut diagnostics,
        );
    }

    for previous_field in &previous_contract.fields {
        if next_catalog
            .fields
            .iter()
            .any(|field| field.path == previous_field.path)
        {
            continue;
        }
        if let Some(value) = resolve_context_path(&migrated_context, &previous_field.path).cloned()
        {
            if policy.retain_removed_paths {
                operations.push(RuntimeContextMigrationOperation {
                    kind: RuntimeContextMigrationOperationKind::RemovedPathRetained,
                    path: previous_field.path.clone(),
                    previous: Some(value.clone()),
                    next: Some(value),
                    automatic: true,
                    message: format!(
                        "removed contract path `{}` was retained as opaque runtime data",
                        previous_field.path
                    ),
                });
            } else {
                diagnostics.push(migration_diagnostic(
                    ValidationSeverity::Info,
                    "runtime_context_removed_path_not_pruned",
                    &previous_field.path,
                    "removed runtime paths are preserved because destructive pruning is not enabled in Fly v1",
                ));
            }
        }
    }

    let preflight = preflight_runtime_context(next_document, &migrated_context, policy.preflight);
    diagnostics.extend(preflight.diagnostics.clone());
    deduplicate_diagnostics(&mut diagnostics);
    RuntimeContextMigrationResult {
        migrated_context,
        effective_context: preflight.effective_context,
        accepted: preflight.accepted,
        diff,
        operations,
        diagnostics,
    }
}

fn migrate_field(
    field: &ContextFieldDefinition,
    context: &mut Value,
    policy: RuntimeContextMigrationPolicy,
    operations: &mut Vec<RuntimeContextMigrationOperation>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    match resolve_context_path(context, &field.path).cloned() {
        Some(value) if field_accepts(field, &value) => {
            operations.push(RuntimeContextMigrationOperation {
                kind: RuntimeContextMigrationOperationKind::Preserved,
                path: field.path.clone(),
                previous: Some(value.clone()),
                next: Some(value),
                automatic: true,
                message: format!(
                    "runtime value `{}` is compatible and was preserved",
                    field.path
                ),
            });
        }
        Some(value) => {
            if policy.coerce_scalars {
                if let Some(coerced) = coerce_value(&value, field.kind) {
                    match set_context_path(context, &field.path, coerced.clone()) {
                        Ok(()) => operations.push(RuntimeContextMigrationOperation {
                            kind: RuntimeContextMigrationOperationKind::ScalarCoerced,
                            path: field.path.clone(),
                            previous: Some(value),
                            next: Some(coerced),
                            automatic: true,
                            message: format!(
                                "runtime value `{}` was coerced to {}",
                                field.path,
                                field.kind.as_str()
                            ),
                        }),
                        Err(error) => {
                            record_write_failure(field, value, error, operations, diagnostics)
                        }
                    }
                    return;
                }
            }
            operations.push(RuntimeContextMigrationOperation {
                kind: RuntimeContextMigrationOperationKind::TypeMismatch,
                path: field.path.clone(),
                previous: Some(value.clone()),
                next: None,
                automatic: false,
                message: format!(
                    "runtime value `{}` is not compatible with {}",
                    field.path,
                    field.kind.as_str()
                ),
            });
            diagnostics.push(migration_diagnostic(
                ValidationSeverity::Warning,
                "runtime_context_migration_type_mismatch",
                &field.path,
                format!(
                    "runtime value `{}` requires manual conversion to {}",
                    field.path,
                    field.kind.as_str()
                ),
            ));
        }
        None if policy.apply_defaults && field.default.is_some() => {
            let default = field.default.clone().expect("default checked above");
            match set_context_path(context, &field.path, default.clone()) {
                Ok(()) => operations.push(RuntimeContextMigrationOperation {
                    kind: RuntimeContextMigrationOperationKind::DefaultApplied,
                    path: field.path.clone(),
                    previous: None,
                    next: Some(default),
                    automatic: true,
                    message: format!("default was applied to `{}`", field.path),
                }),
                Err(error) => {
                    record_write_failure(field, Value::Null, error, operations, diagnostics)
                }
            }
        }
        None if field.required && policy.synthesize_required_values => {
            let synthesized = placeholder_value(field.kind, field.item_kind);
            match set_context_path(context, &field.path, synthesized.clone()) {
                Ok(()) => operations.push(RuntimeContextMigrationOperation {
                    kind: RuntimeContextMigrationOperationKind::RequiredValueSynthesized,
                    path: field.path.clone(),
                    previous: None,
                    next: Some(synthesized),
                    automatic: true,
                    message: format!(
                        "placeholder value was synthesized for required path `{}`",
                        field.path
                    ),
                }),
                Err(error) => {
                    record_write_failure(field, Value::Null, error, operations, diagnostics)
                }
            }
        }
        None if field.required => {
            operations.push(RuntimeContextMigrationOperation {
                kind: RuntimeContextMigrationOperationKind::MissingRequired,
                path: field.path.clone(),
                previous: None,
                next: None,
                automatic: false,
                message: format!("required runtime path `{}` is missing", field.path),
            });
            diagnostics.push(migration_diagnostic(
                ValidationSeverity::Warning,
                "runtime_context_migration_required_missing",
                &field.path,
                format!("required runtime path `{}` needs a value", field.path),
            ));
        }
        None => {}
    }
}

fn field_accepts(field: &ContextFieldDefinition, value: &Value) -> bool {
    if !field.kind.accepts(value) {
        return false;
    }
    if let (ContextValueKind::Array, Some(item_kind), Value::Array(items)) =
        (field.kind, field.item_kind, value)
    {
        return items.iter().all(|item| item_kind.accepts(item));
    }
    true
}

fn coerce_value(value: &Value, kind: ContextValueKind) -> Option<Value> {
    match kind {
        ContextValueKind::Any => Some(value.clone()),
        ContextValueKind::Null => value.is_null().then_some(Value::Null),
        ContextValueKind::String => match value {
            Value::String(value) => Some(Value::String(value.clone())),
            Value::Bool(value) => Some(Value::String(value.to_string())),
            Value::Number(value) => Some(Value::String(value.to_string())),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        },
        ContextValueKind::Number => match value {
            Value::Number(value) => Some(Value::Number(value.clone())),
            Value::String(value) => value
                .trim()
                .parse::<f64>()
                .ok()
                .and_then(Number::from_f64)
                .map(Value::Number),
            Value::Bool(value) => Some(json!(if *value { 1 } else { 0 })),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        },
        ContextValueKind::Boolean => match value {
            Value::Bool(value) => Some(Value::Bool(*value)),
            Value::Number(value) => value.as_f64().map(|value| Value::Bool(value != 0.0)),
            Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(Value::Bool(true)),
                "false" | "0" | "no" | "off" => Some(Value::Bool(false)),
                _ => None,
            },
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        },
        ContextValueKind::Object => value.as_object().map(|value| Value::Object(value.clone())),
        ContextValueKind::Array => value.as_array().map(|value| Value::Array(value.clone())),
    }
}

fn placeholder_value(kind: ContextValueKind, item_kind: Option<ContextValueKind>) -> Value {
    match kind {
        ContextValueKind::Any | ContextValueKind::Null => Value::Null,
        ContextValueKind::Boolean => Value::Bool(false),
        ContextValueKind::Number => json!(0),
        ContextValueKind::String => Value::String(String::new()),
        ContextValueKind::Object => Value::Object(Map::new()),
        ContextValueKind::Array => item_kind
            .map(|kind| Value::Array(vec![placeholder_value(kind, None)]))
            .unwrap_or_else(|| Value::Array(Vec::new())),
    }
}

fn record_write_failure(
    field: &ContextFieldDefinition,
    previous: Value,
    error: String,
    operations: &mut Vec<RuntimeContextMigrationOperation>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    operations.push(RuntimeContextMigrationOperation {
        kind: RuntimeContextMigrationOperationKind::WriteFailed,
        path: field.path.clone(),
        previous: Some(previous),
        next: None,
        automatic: false,
        message: format!(
            "runtime path `{}` could not be written: {error}",
            field.path
        ),
    });
    diagnostics.push(migration_diagnostic(
        ValidationSeverity::Warning,
        "runtime_context_migration_write_failed",
        &field.path,
        error,
    ));
}

fn normalize_context(value: Value) -> Value {
    match value {
        Value::Null => Value::Object(Map::new()),
        value => value,
    }
}

fn deduplicate_diagnostics(diagnostics: &mut Vec<ValidationDiagnostic>) {
    let mut seen = std::collections::BTreeSet::new();
    diagnostics.retain(|diagnostic| {
        seen.insert((
            diagnostic.severity as u8,
            diagnostic.code.clone(),
            diagnostic.path.clone(),
            diagnostic.message.clone(),
        ))
    });
}

fn migration_diagnostic(
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
    use crate::GrapesJsV1Codec;

    fn document(schema: Value) -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": schema
        }))
        .expect("document")
    }

    #[test]
    fn migration_applies_defaults_and_coerces_scalars() {
        let previous_document = document(json!([{
            "id": "count",
            "path": "count",
            "kind": "string"
        }]));
        let next_document = document(json!([{
            "id": "count",
            "path": "count",
            "kind": "number",
            "required": true
        }, {
            "id": "currency",
            "path": "currency",
            "kind": "string",
            "default": "EUR"
        }]));
        let result = migrate_runtime_context(
            &RuntimeContextContractSnapshot::from_document(&previous_document),
            &next_document,
            &json!({ "count": "12.5" }),
            RuntimeContextMigrationPolicy {
                coerce_scalars: true,
                ..RuntimeContextMigrationPolicy::default()
            },
        );
        assert!(result.accepted);
        assert_eq!(result.migrated_context["count"], 12.5);
        assert_eq!(result.migrated_context["currency"], "EUR");
        assert!(result.operations.iter().any(|operation| {
            operation.kind == RuntimeContextMigrationOperationKind::ScalarCoerced
        }));
        assert!(result.operations.iter().any(|operation| {
            operation.kind == RuntimeContextMigrationOperationKind::DefaultApplied
        }));
    }

    #[test]
    fn migration_reports_missing_required_without_synthesis() {
        let previous_document = document(json!([]));
        let next_document = document(json!([{
            "id": "title",
            "path": "page.title",
            "kind": "string",
            "required": true
        }]));
        let result = migrate_runtime_context(
            &RuntimeContextContractSnapshot::from_document(&previous_document),
            &next_document,
            &json!({}),
            RuntimeContextMigrationPolicy::default(),
        );
        assert!(!result.accepted);
        assert!(result.operations.iter().any(|operation| {
            operation.kind == RuntimeContextMigrationOperationKind::MissingRequired
        }));
    }

    #[test]
    fn migration_can_synthesize_required_placeholders() {
        let previous_document = document(json!([]));
        let next_document = document(json!([{
            "id": "title",
            "path": "page.title",
            "kind": "string",
            "required": true
        }]));
        let result = migrate_runtime_context(
            &RuntimeContextContractSnapshot::from_document(&previous_document),
            &next_document,
            &json!({}),
            RuntimeContextMigrationPolicy {
                synthesize_required_values: true,
                ..RuntimeContextMigrationPolicy::default()
            },
        );
        assert!(result.accepted);
        assert_eq!(result.migrated_context["page"]["title"], "");
    }
}
