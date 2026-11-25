use crate::ComponentIndex;
use crate::{
    ComponentObject, FlyError, FlyResult, ProjectDocument, ValidationDiagnostic,
    ValidationSeverity, is_valid_runtime_context_path, resolve_context_path,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_RUNTIME_BINDINGS_FIELD: &str = "flyRuntimeBindings";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum BindingTarget {
    Attribute { name: String },
    Field { name: String },
    Style { name: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BindingTransform {
    #[default]
    Identity,
    String,
    Number,
    Boolean,
    Uppercase,
    Lowercase,
    Trim,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeBinding {
    pub id: String,
    pub component_id: String,
    pub path: String,
    #[serde(flatten)]
    pub target: BindingTarget,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_binding_fallback")]
    pub fallback: Option<Value>,
    #[serde(default)]
    pub transform: BindingTransform,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

// Missing fallback and an explicit JSON null fallback are different contracts.
fn deserialize_binding_fallback<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BindingCommand {
    Upsert { binding: Box<RuntimeBinding> },
    Remove { binding_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BindingCatalog {
    pub bindings: Vec<RuntimeBinding>,
    pub unknown_entries: Vec<Value>,
}

impl BindingCatalog {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let mut catalog = Self::default();
        let Some(Value::Array(entries)) =
            document.project.extensions.get(FLY_RUNTIME_BINDINGS_FIELD)
        else {
            return catalog;
        };
        for entry in entries {
            match serde_json::from_value::<RuntimeBinding>(entry.clone()) {
                Ok(binding) => catalog.bindings.push(binding),
                Err(_) => catalog.unknown_entries.push(entry.clone()),
            }
        }
        catalog
    }

    pub fn component_bindings<'a>(
        &'a self,
        component_id: &'a str,
    ) -> impl Iterator<Item = &'a RuntimeBinding> {
        self.bindings
            .iter()
            .filter(move |binding| binding.component_id == component_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BindingMaterialization {
    pub document: ProjectDocument,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub applied_bindings: usize,
    pub fallback_bindings: usize,
    pub unresolved_bindings: usize,
}

pub fn apply_binding_command(
    document: &mut ProjectDocument,
    command: &BindingCommand,
) -> FlyResult<()> {
    let mut catalog = BindingCatalog::from_document(document);
    match command {
        BindingCommand::Upsert { binding } => {
            validate_binding_identity(document, binding)?;
            if catalog.bindings.iter().filter(|candidate| candidate.id == binding.id).count() > 1 {
                return Err(FlyError::Decode(format!("runtime binding id `{}` is ambiguous", binding.id)));
            }
            if catalog.bindings.iter().any(|candidate| {
                candidate.id != binding.id && binding_slot(candidate) == binding_slot(binding)
            }) {
                return Err(FlyError::Decode(format!(
                    "runtime binding `{}` targets a slot owned by another binding", binding.id
                )));
            }
            if let Some(index) = catalog
                .bindings
                .iter()
                .position(|candidate| candidate.id == binding.id)
            {
                catalog.bindings[index] = binding.as_ref().clone();
            } else {
                catalog.bindings.push(binding.as_ref().clone());
            }
        }
        BindingCommand::Remove { binding_id } => {
            let before = catalog.bindings.len();
            catalog.bindings.retain(|binding| binding.id != *binding_id);
            if catalog.bindings.len() == before {
                return Err(FlyError::Decode(format!(
                    "runtime binding `{binding_id}` was not found"
                )));
            }
        }
    }
    write_catalog(document, catalog)
}

pub fn materialize_bindings(document: &ProjectDocument, context: &Value) -> BindingMaterialization {
    let catalog = BindingCatalog::from_document(document);
    let mut materialized = document.clone();
    let mut diagnostics = Vec::new();
    let mut applied_bindings = 0usize;
    let mut fallback_bindings = 0usize;
    let mut unresolved_bindings = 0usize;

    let (conflicts, conflict_diagnostics) = binding_conflicts(&catalog);
    diagnostics.extend(conflict_diagnostics);
    for (index, binding) in catalog.bindings.iter().enumerate() {
        if conflicts.contains(&index) {
            unresolved_bindings += 1;
            continue;
        }
        if !is_valid_runtime_context_path(&binding.path) {
            unresolved_bindings = unresolved_bindings.saturating_add(1);
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Warning,
                "runtime_binding_path_invalid",
                format!(
                    "binding `{}` has an invalid context path `{}`",
                    binding.id, binding.path
                ),
            ));
            continue;
        }
        if let Err(error) = validate_binding_target(&binding.target) {
            unresolved_bindings = unresolved_bindings.saturating_add(1);
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Warning,
                "runtime_binding_target_invalid",
                format!("binding `{}` has an invalid target: {error}", binding.id),
            ));
            continue;
        }
        let resolved = resolve_context_path(context, &binding.path).cloned();
        let (source, used_fallback) = match resolved {
            Some(value) => (Some(value), false),
            None => (binding.fallback.clone(), binding.fallback.is_some()),
        };
        let Some(source) = source else {
            unresolved_bindings = unresolved_bindings.saturating_add(1);
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Info,
                "runtime_binding_unresolved",
                format!(
                    "binding `{}` path `{}` did not resolve and has no fallback",
                    binding.id, binding.path
                ),
            ));
            continue;
        };
        let Some(value) = transform_value(source, binding.transform) else {
            unresolved_bindings = unresolved_bindings.saturating_add(1);
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Warning,
                "runtime_binding_transform_failed",
                format!(
                    "binding `{}` could not apply transform `{:?}`",
                    binding.id, binding.transform
                ),
            ));
            continue;
        };
        let Some(component) = materialized.component_mut(&binding.component_id) else {
            unresolved_bindings = unresolved_bindings.saturating_add(1);
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Warning,
                "runtime_binding_target_missing",
                format!(
                    "binding `{}` targets missing component `{}`",
                    binding.id, binding.component_id
                ),
            ));
            continue;
        };
        apply_value(component, &binding.target, value);
        applied_bindings = applied_bindings.saturating_add(1);
        if used_fallback {
            fallback_bindings = fallback_bindings.saturating_add(1);
        }
    }

    BindingMaterialization {
        document: materialized,
        diagnostics,
        applied_bindings,
        fallback_bindings,
        unresolved_bindings,
    }
}

pub fn validate_binding_definitions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let catalog = BindingCatalog::from_document(document);
    let mut diagnostics = Vec::new();
    // One traversal for the whole catalog. `contains_component` walks every page, and this loop
    // called it once per binding.
    let components = ComponentIndex::build(document);

    diagnostics.extend(binding_conflicts(&catalog).1);
    for binding in &catalog.bindings {
        if binding.id.trim().is_empty() {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_id_empty",
                "runtime binding id must not be empty",
            ));
        }
        if !is_valid_runtime_context_path(&binding.path) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_path_invalid",
                format!(
                    "runtime binding `{}` has an invalid context path `{}`",
                    binding.id, binding.path
                ),
            ));
        }
        if let Err(error) = validate_binding_target(&binding.target) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_target_invalid",
                format!(
                    "runtime binding `{}` has an invalid target: {error}",
                    binding.id
                ),
            ));
        }
        if !components.contains(&binding.component_id) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_target_missing",
                format!(
                    "runtime binding `{}` targets missing component `{}`",
                    binding.id, binding.component_id
                ),
            ));
        }
    }

    for entry in &catalog.unknown_entries {
        diagnostics.push(binding_diagnostic(
            ValidationSeverity::Warning,
            "runtime_binding_unknown_entry",
            format!("runtime binding entry is not understood and was preserved: {entry}"),
        ));
    }
    diagnostics
}

// HTML attribute and CSS property names are ASCII case-insensitive. Extension
// fields remain case-sensitive, and separate target namespaces never collide.
fn binding_slot(binding: &RuntimeBinding) -> (&str, u8, String) {
    let (kind, name) = match &binding.target {
        BindingTarget::Attribute { name } => (0, name.to_ascii_lowercase()),
        BindingTarget::Field { name } => (1, name.clone()),
        BindingTarget::Style { name } => (2, name.to_ascii_lowercase()),
    };
    (&binding.component_id, kind, name)
}

fn binding_conflicts(catalog: &BindingCatalog) -> (BTreeSet<usize>, Vec<ValidationDiagnostic>) {
    let mut ids = BTreeMap::<&str, Vec<usize>>::new();
    let mut slots = BTreeMap::<(&str, u8, String), Vec<usize>>::new();
    for (index, binding) in catalog.bindings.iter().enumerate() {
        ids.entry(binding.id.as_str()).or_default().push(index);
        slots.entry(binding_slot(binding)).or_default().push(index);
    }
    let mut conflicts = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for indices in ids.values().filter(|indices| indices.len() > 1) {
        conflicts.extend(indices.iter().copied());
        diagnostics.push(binding_diagnostic(
            ValidationSeverity::Error, "duplicate_runtime_binding_id",
            format!("runtime binding id `{}` is ambiguous", catalog.bindings[indices[0]].id),
        ));
    }
    for ((component, _, name), indices) in slots.iter().filter(|(_, indices)| indices.len() > 1) {
        conflicts.extend(indices.iter().copied());
        diagnostics.push(binding_diagnostic(
            ValidationSeverity::Error, "runtime_binding_target_conflict",
            format!("multiple bindings target component `{component}` slot `{name}`"),
        ));
    }
    (conflicts, diagnostics)
}

fn validate_binding_identity(
    document: &ProjectDocument,
    binding: &RuntimeBinding,
) -> FlyResult<()> {
    if binding.id.trim().is_empty() {
        return Err(FlyError::Decode(
            "runtime binding id must not be empty".to_string(),
        ));
    }
    if !is_valid_runtime_context_path(&binding.path) {
        return Err(FlyError::Decode(format!(
            "runtime binding path `{}` is invalid",
            binding.path
        )));
    }
    validate_binding_target(&binding.target).map_err(FlyError::Decode)?;
    if !document.contains_component(&binding.component_id) {
        return Err(FlyError::ComponentNotFound(binding.component_id.clone()));
    }
    Ok(())
}

fn write_catalog(document: &mut ProjectDocument, catalog: BindingCatalog) -> FlyResult<()> {
    let mut entries = catalog
        .bindings
        .into_iter()
        .map(|binding| {
            serde_json::to_value(binding).map_err(|error| FlyError::Encode(error.to_string()))
        })
        .collect::<FlyResult<Vec<_>>>()?;
    entries.extend(catalog.unknown_entries);
    document.project.extensions.insert(
        FLY_RUNTIME_BINDINGS_FIELD.to_string(),
        Value::Array(entries),
    );
    document.invalidate_caches();
    Ok(())
}

fn apply_value(component: &mut ComponentObject, target: &BindingTarget, value: Value) {
    match target {
        BindingTarget::Attribute { name } => {
            let name = name.to_ascii_lowercase();
            component.attributes.retain(|key, _| !key.eq_ignore_ascii_case(&name));
            component.attributes.insert(name, value);
        }
        BindingTarget::Field { name } => {
            set_component_field(component, name, value);
        }
        BindingTarget::Style { name } => {
            let style = component
                .style
                .get_or_insert_with(|| Value::Object(Map::new()));
            if !style.is_object() {
                *style = Value::Object(Map::new());
            }
            if let Some(style) = style.as_object_mut() {
                let name = name.to_ascii_lowercase();
                style.retain(|key, _| !key.eq_ignore_ascii_case(&name));
                style.insert(name, value);
            }
        }
    }
}

fn set_component_field(component: &mut ComponentObject, name: &str, value: Value) {
    match name {
        "tagName" => component.tag_name = value.as_str().map(ToString::to_string),
        other => {
            component.set_extension_field(other.to_string(), value);
        }
    }
}

fn validate_binding_target(target: &BindingTarget) -> Result<(), String> {
    match target {
        BindingTarget::Attribute { name } | BindingTarget::Style { name } => {
            if name.trim().is_empty() {
                Err("attribute and style targets require a non-empty name".to_string())
            } else {
                Ok(())
            }
        }
        BindingTarget::Field { name } => {
            if name.trim().is_empty() {
                return Err("field target requires a non-empty name".to_string());
            }
            if matches!(
                name.as_str(),
                "id" | "type" | "provider" | "symbolId" | "components" | "attributes" | "style" | "traits"
            ) {
                return Err(format!(
                    "field `{name}` is owned by the component structure and cannot be runtime-bound"
                ));
            }
            Ok(())
        }
    }
}

fn transform_value(value: Value, transform: BindingTransform) -> Option<Value> {
    match transform {
        BindingTransform::Identity => Some(value),
        BindingTransform::String => Some(Value::String(match value {
            Value::String(value) => value,
            other => other.to_string(),
        })),
        // The input already has a JSON numeric representation. Routing an
        // integer through f64 loses precision above 2^53 and changes its type.
        BindingTransform::Number => match value {
            Value::Number(_) => Some(value),
            _ => None,
        },
        BindingTransform::Boolean => match value {
            Value::Bool(value) => Some(Value::Bool(value)),
            Value::String(value) if value.eq_ignore_ascii_case("true") => Some(Value::Bool(true)),
            Value::String(value) if value.eq_ignore_ascii_case("false") => Some(Value::Bool(false)),
            _ => None,
        },
        BindingTransform::Uppercase => value
            .as_str()
            .map(|value| Value::String(value.to_uppercase())),
        BindingTransform::Lowercase => value
            .as_str()
            .map(|value| Value::String(value.to_lowercase())),
        BindingTransform::Trim => value
            .as_str()
            .map(|value| Value::String(value.trim().to_string())),
        BindingTransform::Json => Some(Value::String(value.to_string())),
    }
}

fn binding_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: format!("project.extensions.{FLY_RUNTIME_BINDINGS_FIELD}"),
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
                    "components": [{ "id": "title", "type": "text" }]
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn bindings_resolve_indexed_context_paths() {
        let mut document = document();
        let before_hash = document.hash();
        apply_binding_command(
            &mut document,
            &BindingCommand::Upsert {
                binding: Box::new(RuntimeBinding {
                    id: "title".to_string(),
                    component_id: "title".to_string(),
                    path: "items[0].title".to_string(),
                    target: BindingTarget::Field {
                        name: "content".to_string(),
                    },
                    fallback: None,
                    transform: BindingTransform::Identity,
                    extensions: Map::new(),
                }),
            },
        )
        .expect("binding");
        assert_ne!(document.hash(), before_hash);
        let materialized =
            materialize_bindings(&document, &json!({ "items": [{ "title": "First" }] }));
        assert_eq!(materialized.applied_bindings, 1);
        assert_eq!(
            materialized
                .document
                .component("title")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("First")
        );
    }

    #[test]
    fn number_transform_preserves_integer_identity_and_rejects_non_numbers() {
        for value in [json!(u64::MAX), json!(i64::MIN), json!(9007199254740993_u64), json!(1.25)] {
            assert_eq!(transform_value(value.clone(), BindingTransform::Number), Some(value));
        }
        assert!(transform_value(json!("42"), BindingTransform::Number).is_none());
    }

    #[test]
    fn explicit_null_fallback_round_trips_and_does_not_override_present_null() {
        let mut source = document();
        source.project_mut().extensions.insert(FLY_RUNTIME_BINDINGS_FIELD.to_string(), json!([
            {"id": "null", "component_id": "title", "path": "missing", "target": "field", "name": "content", "fallback": null}
        ]));
        let catalog = BindingCatalog::from_document(&source);
        assert_eq!(catalog.bindings[0].fallback, Some(Value::Null));
        assert!(serde_json::to_value(&catalog.bindings[0]).expect("serialize").get("fallback").is_some());
        let result = materialize_bindings(&source, &json!({}));
        assert_eq!(result.fallback_bindings, 1);
        assert_eq!(result.document.component("title").expect("title").extensions.get("content"), Some(&Value::Null));
        let result = materialize_bindings(&source, &json!({"missing": null}));
        assert_eq!(result.applied_bindings, 1);
        assert_eq!(result.fallback_bindings, 0);
    }

    #[test]
    fn conflicting_binding_slots_are_rejected_in_both_orders_and_commands() {
        let bindings = vec![
            json!({"id": "first", "component_id": "title", "path": "first", "target": "attribute", "name": "HREF"}),
            json!({"id": "second", "component_id": "title", "path": "second", "target": "attribute", "name": "href"}),
        ];
        for entries in [bindings.clone(), bindings.into_iter().rev().collect()] {
            let mut source = document();
            source.project_mut().extensions.insert(FLY_RUNTIME_BINDINGS_FIELD.to_string(), json!(entries));
            let result = materialize_bindings(&source, &json!({"first": "/one", "second": "/two"}));
            assert_eq!(result.document, source);
            assert_eq!(result.applied_bindings, 0);
            assert_eq!(result.unresolved_bindings, 2);
            assert!(result.diagnostics.iter().any(|d| d.code == "runtime_binding_target_conflict"));
            assert!(validate_binding_definitions(&source).iter().any(|d| d.code == "runtime_binding_target_conflict"));
            let binding = BindingCatalog::from_document(&source).bindings.remove(0);
            assert!(apply_binding_command(&mut source, &BindingCommand::Upsert { binding: Box::new(binding) }).is_err());
        }
    }

    #[test]
    fn binding_replaces_authored_case_variants_and_cannot_write_symbol_identity() {
        let mut source = document();
        let title = source.component_mut("title").expect("title");
        title.attributes.insert("TITLE".to_string(), json!("old"));
        title.style = Some(json!({"COLOR": "red"}));
        source.project_mut().extensions.insert(FLY_RUNTIME_BINDINGS_FIELD.to_string(), json!([
            {"id": "title", "component_id": "title", "path": "title", "target": "attribute", "name": "Title"},
            {"id": "color", "component_id": "title", "path": "color", "target": "style", "name": "Color"}
        ]));
        let result = materialize_bindings(&source, &json!({"title": "new", "color": "blue"}));
        let title = result.document.component("title").expect("title");
        assert_eq!(title.attributes, json!({"title": "new"}).as_object().unwrap().clone());
        assert_eq!(title.style, Some(json!({"color": "blue"})));
        assert!(validate_binding_target(&BindingTarget::Field {name: "symbolId".to_string()}).is_err());
    }

    #[test]
    fn structural_field_bindings_are_rejected() {
        let mut document = document();
        let error = apply_binding_command(
            &mut document,
            &BindingCommand::Upsert {
                binding: Box::new(RuntimeBinding {
                    id: "bad".to_string(),
                    component_id: "title".to_string(),
                    path: "component.id".to_string(),
                    target: BindingTarget::Field {
                        name: "id".to_string(),
                    },
                    fallback: None,
                    transform: BindingTransform::Identity,
                    extensions: Map::new(),
                }),
            },
        )
        .expect_err("structural binding");
        assert!(matches!(error, FlyError::Decode(_)));
    }
}
