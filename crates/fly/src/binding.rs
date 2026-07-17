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
    #[serde(flatten)]
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
    let mut ids = BTreeSet::new();

    for binding in &catalog.bindings {
        if binding.id.trim().is_empty() {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_id_empty",
                "runtime binding id must not be empty",
            ));
        } else if !ids.insert(binding.id.clone()) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "duplicate_runtime_binding_id",
                format!("runtime binding id `{}` is duplicated", binding.id),
            ));
        }
        if binding.path.trim().is_empty() {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_path_empty",
                format!("runtime binding `{}` has an empty context path", binding.id),
            ));
        }
        if !document.contains_component(&binding.component_id) {
            diagnostics.push(binding_diagnostic(
                ValidationSeverity::Error,
                "runtime_binding_component_missing",
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
    Ok(())
}

fn resolve_path<'a>(context: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = context;
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        current = current.get(segment)?;
    }
    Some(current)
}

fn apply_value(component: &mut ComponentObject, target: &BindingTarget, value: Value) {
    match target {
        BindingTarget::Attribute { name } => {
            component.attributes.insert(name.clone(), value);
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
                style.insert(name.clone(), value);
            }
        }
    }
}

fn set_component_field(component: &mut ComponentObject, name: &str, value: Value) {
    match name {
        "id" => component.id = value.as_str().map(ToString::to_string),
        "type" => component.component_type = value.as_str().map(ToString::to_string),
        "tagName" => component.tag_name = value.as_str().map(ToString::to_string),
        "provider" => component.provider = value.as_str().map(ToString::to_string),
        "style" => component.style = Some(value),
        "traits" => component.traits = value.as_array().cloned().unwrap_or_default(),
        "components" => {}
        other => {
            component.extensions.insert(other.to_string(), value);
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
        BindingTransform::Number => value.as_f64().and_then(Number::from_f64).map(Value::Number),
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
