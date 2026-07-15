use crate::{
    ComponentObject, FlyError, FlyResult, ProjectDocument, ValidationDiagnostic, ValidationSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;

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
    pub target: BindingTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Value>,
    #[serde(default)]
    pub transform: BindingTransform,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
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

    for binding in &catalog.bindings {
        let resolved = resolve_path(context, &binding.path).cloned();
        let (source, used_fallback) = match resolved {
            Some(value) => (Some(value), false),
            None => (binding.fallback.clone(), binding.fallback.is_some()),
        };
        let Some(source) = source else {
            unresolved_bindings = unresolved_bindings.saturating_add(1);
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Info,
                "runtime_binding_unresolved",
                Some(binding.component_id.clone()),
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
                Some(binding.component_id.clone()),
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
                Some(binding.component_id.clone()),
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
    let mut ids = BTreeSet::new();

    for binding in &catalog.bindings {
        if binding.id.trim().is_empty() {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_id_empty",
                Some(binding.component_id.clone()),
                "runtime binding id must not be empty",
            ));
        } else if !ids.insert(binding.id.clone()) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "duplicate_runtime_binding_id",
                Some(binding.component_id.clone()),
                format!("runtime binding id `{}` is duplicated", binding.id),
            ));
        }
        if binding.path.trim().is_empty() {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_path_empty",
                Some(binding.component_id.clone()),
                format!("binding `{}` path must not be empty", binding.id),
            ));
        }
        if !document.contains_component(&binding.component_id) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_target_missing",
                Some(binding.component_id.clone()),
                format!(
                    "binding `{}` targets missing component `{}`",
                    binding.id, binding.component_id
                ),
            ));
        }
        if let Err(message) = validate_target(&binding.target) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_target_invalid",
                Some(binding.component_id.clone()),
                format!("binding `{}` {message}", binding.id),
            ));
        }
    }

    if !catalog.unknown_entries.is_empty() {
        diagnostics.push(binding_diagnostic(
            ValidationSeverity::Info,
            "opaque_runtime_bindings",
            None,
            format!(
                "{} runtime binding entries are opaque and preserved",
                catalog.unknown_entries.len()
            ),
        ));
    }

    diagnostics
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
    if binding.path.trim().is_empty() {
        return Err(FlyError::Decode(
            "runtime binding path must not be empty".to_string(),
        ));
    }
    if !document.contains_component(&binding.component_id) {
        return Err(FlyError::ComponentNotFound(binding.component_id.clone()));
    }
    validate_target(&binding.target).map_err(FlyError::Decode)
}

fn validate_target(target: &BindingTarget) -> Result<(), String> {
    let (kind, name) = match target {
        BindingTarget::Attribute { name } => ("attribute", name),
        BindingTarget::Field { name } => ("field", name),
        BindingTarget::Style { name } => ("style property", name),
    };
    if name.trim().is_empty() {
        return Err(format!("{kind} name must not be empty"));
    }
    let valid = match target {
        BindingTarget::Style { .. } => name
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '-'),
        BindingTarget::Attribute { .. } | BindingTarget::Field { .. } => {
            name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
            })
        }
    };
    if !valid {
        return Err(format!(
            "{kind} name `{name}` contains unsupported characters"
        ));
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
    if entries.is_empty() {
        document
            .project
            .extensions
            .remove(FLY_RUNTIME_BINDINGS_FIELD);
    } else {
        document.project.extensions.insert(
            FLY_RUNTIME_BINDINGS_FIELD.to_string(),
            Value::Array(entries),
        );
    }
    Ok(())
}

fn apply_value(component: &mut ComponentObject, target: &BindingTarget, value: Value) {
    match target {
        BindingTarget::Attribute { name } => {
            if value.is_null() {
                component.attributes.remove(name);
            } else {
                component.attributes.insert(name.clone(), value);
            }
        }
        BindingTarget::Style { name } => {
            if !matches!(component.style, Some(Value::Object(_))) {
                component.style = Some(Value::Object(Map::new()));
            }
            if let Some(Value::Object(style)) = component.style.as_mut() {
                if value.is_null() {
                    style.remove(name);
                } else {
                    style.insert(name.clone(), value);
                }
            }
        }
        BindingTarget::Field { name } => match name.as_str() {
            "type" => component.component_type = value.as_str().map(ToString::to_string),
            "tagName" => component.tag_name = value.as_str().map(ToString::to_string),
            "provider" => component.provider = value.as_str().map(ToString::to_string),
            "schemaVersion" => component.schema_version = value.as_str().map(ToString::to_string),
            _ if value.is_null() => {
                component.extensions.remove(name);
            }
            _ => {
                component.extensions.insert(name.clone(), value);
            }
        },
    }
}

fn transform_value(value: Value, transform: BindingTransform) -> Option<Value> {
    match transform {
        BindingTransform::Identity => Some(value),
        BindingTransform::String => Some(Value::String(scalar_text(&value))),
        BindingTransform::Json => serde_json::to_string(&value).ok().map(Value::String),
        BindingTransform::Uppercase => Some(Value::String(scalar_text(&value).to_uppercase())),
        BindingTransform::Lowercase => Some(Value::String(scalar_text(&value).to_lowercase())),
        BindingTransform::Trim => Some(Value::String(scalar_text(&value).trim().to_string())),
        BindingTransform::Boolean => Some(Value::Bool(is_truthy(&value))),
        BindingTransform::Number => match value {
            Value::Number(value) => Some(Value::Number(value)),
            Value::String(value) => value
                .trim()
                .parse::<f64>()
                .ok()
                .and_then(Number::from_f64)
                .map(Value::Number),
            Value::Bool(value) => Some(Value::Number(Number::from(u64::from(value)))),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        },
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for segment in parse_path(path)? {
        current = match segment {
            PathSegment::Key(key) => current.as_object()?.get(&key)?,
            PathSegment::Index(index) => current.as_array()?.get(index)?,
        };
    }
    Some(current)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathSegment {
    Key(String),
    Index(usize),
}

fn parse_path(path: &str) -> Option<Vec<PathSegment>> {
    let path = path.trim().trim_start_matches('$').trim_start_matches('.');
    if path.is_empty() {
        return Some(Vec::new());
    }
    let mut segments = Vec::new();
    let mut token = String::new();
    let mut chars = path.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '.' => {
                if token.is_empty() {
                    return None;
                }
                segments.push(PathSegment::Key(std::mem::take(&mut token)));
            }
            '[' => {
                if !token.is_empty() {
                    segments.push(PathSegment::Key(std::mem::take(&mut token)));
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
                if !closed {
                    return None;
                }
                segments.push(PathSegment::Index(index.parse().ok()?));
                if chars.peek() == Some(&'.') {
                    chars.next();
                }
            }
            _ => token.push(character),
        }
    }
    if !token.is_empty() {
        segments.push(PathSegment::Key(token));
    }
    Some(segments)
}

fn binding_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    component_id: Option<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: component_id
            .as_deref()
            .map(|component_id| format!("component:{component_id}"))
            .unwrap_or_else(|| "project.runtime.bindings".to_string()),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "heading",
                        "content": "Static",
                        "attributes": { "title": "Static" },
                        "style": { "color": "black" }
                    }]
                }
            }],
            "flyRuntimeBindings": [{
                "id": "title-content",
                "component_id": "title",
                "path": "page.title",
                "target": "field",
                "name": "content",
                "transform": "uppercase"
            }, {
                "id": "title-attribute",
                "component_id": "title",
                "path": "page.tooltip",
                "target": "attribute",
                "name": "title",
                "fallback": "Fallback"
            }, {
                "id": "title-color",
                "component_id": "title",
                "path": "theme.color",
                "target": "style",
                "name": "color"
            }]
        }))
        .expect("document")
    }

    #[test]
    fn materialization_applies_fields_attributes_styles_and_fallbacks() {
        let source = document();
        let materialized = materialize_bindings(
            &source,
            &json!({ "page": { "title": "Hello" }, "theme": { "color": "red" } }),
        );
        let title = materialized.document.component("title").expect("title");
        assert_eq!(
            title.extensions.get("content").and_then(Value::as_str),
            Some("HELLO")
        );
        assert_eq!(
            title.attributes.get("title").and_then(Value::as_str),
            Some("Fallback")
        );
        assert_eq!(
            title
                .style
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|style| style.get("color"))
                .and_then(Value::as_str),
            Some("red")
        );
        assert_eq!(materialized.applied_bindings, 3);
        assert_eq!(materialized.fallback_bindings, 1);
        assert_eq!(
            source
                .component("title")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Static")
        );
    }

    #[test]
    fn commands_preserve_unknown_entries() {
        let mut document = document();
        document.project.extensions.insert(
            FLY_RUNTIME_BINDINGS_FIELD.to_string(),
            json!([{ "providerBinding": true }]),
        );
        apply_binding_command(
            &mut document,
            &BindingCommand::Upsert {
                binding: Box::new(RuntimeBinding {
                    id: "new-binding".to_string(),
                    component_id: "title".to_string(),
                    path: "page.title".to_string(),
                    target: BindingTarget::Field {
                        name: "content".to_string(),
                    },
                    fallback: None,
                    transform: BindingTransform::Identity,
                    extensions: Map::new(),
                }),
            },
        )
        .expect("upsert binding");
        let entries = document.project.extensions[FLY_RUNTIME_BINDINGS_FIELD]
            .as_array()
            .expect("bindings");
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.get("providerBinding").is_some()));
    }
}
