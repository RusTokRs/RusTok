use crate::{
    ComputedContextValue, ContextExpression, ContextFieldDefinition, ContextSchemaCatalog,
    ContextValueKind, ProjectDocument, ValidationDiagnostic, ValidationSeverity,
    materialize_context, validate_context_definitions,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextFieldContract {
    pub id: String,
    pub path: String,
    pub kind: ContextValueKind,
    pub required: bool,
    pub has_default: bool,
    pub item_kind: Option<ContextValueKind>,
}

impl From<&ContextFieldDefinition> for RuntimeContextFieldContract {
    fn from(field: &ContextFieldDefinition) -> Self {
        Self {
            id: field.id.clone(),
            path: field.path.clone(),
            kind: field.kind,
            required: field.required,
            has_default: field.default.is_some(),
            item_kind: field.item_kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeComputedContract {
    pub id: String,
    pub path: String,
    pub dependencies: Vec<String>,
    pub has_fallback: bool,
}

impl From<&ComputedContextValue> for RuntimeComputedContract {
    fn from(computed: &ComputedContextValue) -> Self {
        Self {
            id: computed.id.clone(),
            path: computed.path.clone(),
            dependencies: context_expression_dependencies(&computed.expression),
            has_fallback: computed.fallback.is_some(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RuntimeContextContract {
    pub fields: Vec<RuntimeContextFieldContract>,
    pub computed: Vec<RuntimeComputedContract>,
    pub required_paths: Vec<String>,
    pub defaulted_paths: Vec<String>,
    pub computed_paths: Vec<String>,
    pub definition_diagnostics: Vec<ValidationDiagnostic>,
}

impl RuntimeContextContract {
    pub fn is_valid(&self) -> bool {
        !self
            .definition_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextPreflightPolicy {
    pub missing_required_is_error: bool,
    pub type_mismatch_is_error: bool,
    pub unresolved_computed_is_error: bool,
}

impl Default for RuntimeContextPreflightPolicy {
    fn default() -> Self {
        Self {
            missing_required_is_error: true,
            type_mismatch_is_error: true,
            unresolved_computed_is_error: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextPreflight {
    pub contract: RuntimeContextContract,
    pub effective_context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub accepted: bool,
    pub defaults_applied: usize,
    pub computed_applied: usize,
    pub computed_fallbacks: usize,
    pub unresolved_computed: usize,
    pub type_mismatches: usize,
    pub missing_required: usize,
}

pub fn extract_runtime_context_contract(document: &ProjectDocument) -> RuntimeContextContract {
    let catalog = ContextSchemaCatalog::from_document(document);
    let fields = catalog
        .fields
        .iter()
        .map(RuntimeContextFieldContract::from)
        .collect::<Vec<_>>();
    let computed = catalog
        .computed
        .iter()
        .map(RuntimeComputedContract::from)
        .collect::<Vec<_>>();
    let required_paths = catalog
        .fields
        .iter()
        .filter(|field| field.required)
        .map(|field| field.path.clone())
        .collect();
    let defaulted_paths = catalog
        .fields
        .iter()
        .filter(|field| field.default.is_some())
        .map(|field| field.path.clone())
        .collect();
    let computed_paths = catalog
        .computed
        .iter()
        .map(|computed| computed.path.clone())
        .collect();
    let mut definition_diagnostics = validate_context_definitions(document);
    definition_diagnostics.extend(validate_strict_context_paths(&catalog));
    deduplicate_diagnostics(&mut definition_diagnostics);

    RuntimeContextContract {
        fields,
        computed,
        required_paths,
        defaulted_paths,
        computed_paths,
        definition_diagnostics,
    }
}

pub fn preflight_runtime_context(
    document: &ProjectDocument,
    input_context: &Value,
    policy: RuntimeContextPreflightPolicy,
) -> RuntimeContextPreflight {
    let contract = extract_runtime_context_contract(document);
    let materialized = materialize_context(document, input_context);
    let mut diagnostics = contract.definition_diagnostics.clone();
    let missing_required = materialized
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "runtime_context_required_missing")
        .count();

    diagnostics.extend(materialized.diagnostics.into_iter().map(|mut diagnostic| {
        let promote = match diagnostic.code.as_str() {
            "runtime_context_required_missing" => policy.missing_required_is_error,
            "runtime_context_type_mismatch" | "runtime_context_array_item_type_mismatch" => {
                policy.type_mismatch_is_error
            }
            "runtime_computed_unresolved" | "runtime_computed_evaluation_failed" => {
                policy.unresolved_computed_is_error
            }
            _ => false,
        };
        if promote {
            diagnostic.severity = ValidationSeverity::Error;
        }
        diagnostic
    }));
    deduplicate_diagnostics(&mut diagnostics);
    let accepted = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error);

    RuntimeContextPreflight {
        contract,
        effective_context: materialized.context,
        diagnostics,
        accepted,
        defaults_applied: materialized.defaults_applied,
        computed_applied: materialized.computed_applied,
        computed_fallbacks: materialized.computed_fallbacks,
        unresolved_computed: materialized.unresolved_computed,
        type_mismatches: materialized.type_mismatches,
        missing_required,
    }
}

pub fn context_expression_dependencies(expression: &ContextExpression) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    collect_dependencies(expression, &mut dependencies);
    dependencies.into_iter().collect()
}

pub fn is_valid_runtime_context_path(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return false;
    }
    let path = path.strip_prefix('$').unwrap_or(path);
    let path = path.strip_prefix('.').unwrap_or(path);
    if path.is_empty() {
        return false;
    }

    let mut token = String::new();
    let mut chars = path.chars().peekable();
    let mut saw_segment = false;
    while let Some(character) = chars.next() {
        match character {
            '.' => {
                if token.is_empty() {
                    return false;
                }
                token.clear();
                saw_segment = true;
            }
            '[' => {
                if !token.is_empty() {
                    token.clear();
                }
                let mut index = String::new();
                let mut closed = false;
                for character in chars.by_ref() {
                    if character == ']' {
                        closed = true;
                        break;
                    }
                    index.push(character);
                }
                if !closed || index.is_empty() || index.parse::<usize>().is_err() {
                    return false;
                }
                saw_segment = true;
                if chars.peek() == Some(&'.') {
                    chars.next();
                    if chars.peek().is_none() {
                        return false;
                    }
                }
            }
            ']' | '{' | '}' => return false,
            character if character.is_whitespace() => return false,
            _ => token.push(character),
        }
    }
    saw_segment || !token.is_empty()
}

fn validate_strict_context_paths(catalog: &ContextSchemaCatalog) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();
    for field in &catalog.fields {
        if !is_valid_runtime_context_path(&field.path) {
            diagnostics.push(contract_diagnostic(
                ValidationSeverity::Error,
                "runtime_context_field_path_invalid",
                &field.path,
                format!("context field `{}` must use a non-empty path", field.id),
            ));
        }
    }
    for computed in &catalog.computed {
        if !is_valid_runtime_context_path(&computed.path) {
            diagnostics.push(contract_diagnostic(
                ValidationSeverity::Error,
                "runtime_computed_path_invalid",
                &computed.path,
                format!("computed value `{}` must use a non-empty path", computed.id),
            ));
        }
        for dependency in context_expression_dependencies(&computed.expression) {
            if !is_valid_runtime_context_path(&dependency) {
                diagnostics.push(contract_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_computed_dependency_path_invalid",
                    &computed.path,
                    format!(
                        "computed value `{}` dependency `{dependency}` is invalid",
                        computed.id
                    ),
                ));
            }
        }
    }
    diagnostics
}

fn collect_dependencies(expression: &ContextExpression, paths: &mut BTreeSet<String>) {
    match expression {
        ContextExpression::Path { path } => {
            paths.insert(path.clone());
        }
        ContextExpression::Format { template } => {
            let mut remaining = template.as_str();
            while let Some(start) = remaining.find("{{") {
                let after = &remaining[start + 2..];
                let Some(end) = after.find("}}") else {
                    break;
                };
                paths.insert(after[..end].trim().to_string());
                remaining = &after[end + 2..];
            }
        }
        ContextExpression::Coalesce { values }
        | ContextExpression::Concat { values, .. }
        | ContextExpression::And { values }
        | ContextExpression::Or { values } => {
            for value in values {
                collect_dependencies(value, paths);
            }
        }
        ContextExpression::Add { left, right }
        | ContextExpression::Subtract { left, right }
        | ContextExpression::Multiply { left, right }
        | ContextExpression::Divide { left, right }
        | ContextExpression::Equals { left, right }
        | ContextExpression::NotEquals { left, right }
        | ContextExpression::GreaterThan { left, right }
        | ContextExpression::LessThan { left, right } => {
            collect_dependencies(left, paths);
            collect_dependencies(right, paths);
        }
        ContextExpression::Not { value } => collect_dependencies(value, paths),
        ContextExpression::If {
            condition,
            then_value,
            else_value,
        } => {
            collect_dependencies(condition, paths);
            collect_dependencies(then_value, paths);
            collect_dependencies(else_value, paths);
        }
        ContextExpression::Literal { .. } => {}
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

fn contract_diagnostic(
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

    #[test]
    fn contract_exposes_required_defaults_and_dependencies() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [{
                "id": "currency",
                "path": "shop.currency",
                "kind": "string",
                "required": true,
                "default": "EUR"
            }],
            "flyRuntimeComputed": [{
                "id": "label",
                "path": "shop.label",
                "expression": {
                    "op": "format",
                    "template": "{{shop.currency}} {{shop.total}}"
                }
            }]
        }))
        .expect("document");
        let contract = extract_runtime_context_contract(&document);
        assert!(contract.is_valid());
        assert_eq!(contract.required_paths, vec!["shop.currency"]);
        assert_eq!(contract.defaulted_paths, vec!["shop.currency"]);
        assert_eq!(
            contract.computed[0].dependencies,
            vec!["shop.currency", "shop.total"]
        );
    }

    #[test]
    fn strict_preflight_promotes_missing_and_type_mismatch() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [{
                "id": "title",
                "path": "page.title",
                "kind": "string",
                "required": true
            }, {
                "id": "count",
                "path": "page.count",
                "kind": "number"
            }]
        }))
        .expect("document");
        let preflight = preflight_runtime_context(
            &document,
            &json!({ "page": { "count": "wrong" } }),
            RuntimeContextPreflightPolicy::default(),
        );
        assert!(!preflight.accepted);
        assert_eq!(preflight.missing_required, 1);
        assert_eq!(preflight.type_mismatches, 1);
        assert!(
            preflight
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
        );
    }

    #[test]
    fn empty_definition_paths_are_rejected_even_though_root_resolution_exists() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [{
                "id": "root-field",
                "path": "",
                "kind": "object"
            }]
        }))
        .expect("document");
        let contract = extract_runtime_context_contract(&document);
        assert!(!contract.is_valid());
        assert!(
            contract
                .definition_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_context_field_path_invalid")
        );
    }
}
