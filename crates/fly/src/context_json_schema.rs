use crate::{
    ContextSchemaCatalog, ContextValueKind, ProjectDocument, ProjectHash, ValidationDiagnostic,
    ValidationSeverity, extract_runtime_context_contract, materialize_context, set_context_path,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextJsonSchema {
    pub schema: Value,
    pub contract_hash: String,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextExamplePolicy {
    pub include_optional_defaults: bool,
    pub synthesize_required_values: bool,
}

impl Default for RuntimeContextExamplePolicy {
    fn default() -> Self {
        Self {
            include_optional_defaults: true,
            synthesize_required_values: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextExample {
    pub input_context: Value,
    pub effective_context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

pub fn export_runtime_context_json_schema(document: &ProjectDocument) -> RuntimeContextJsonSchema {
    let contract = extract_runtime_context_contract(document);
    let catalog = ContextSchemaCatalog::from_document(document);
    let mut diagnostics = contract.definition_diagnostics.clone();
    let mut root = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Fly Runtime Context",
        "type": "object",
        "properties": {},
        "additionalProperties": true,
        "x-fly-contract": {
            "requiredPaths": contract.required_paths,
            "defaultedPaths": contract.defaulted_paths,
            "computedPaths": contract.computed_paths,
        }
    });

    for field in &catalog.fields {
        let Some(segments) = simple_object_path(&field.path) else {
            diagnostics.push(schema_diagnostic(
                ValidationSeverity::Warning,
                "runtime_context_json_schema_path_unsupported",
                &field.path,
                format!(
                    "context field `{}` uses an indexed or invalid path and is only exposed in x-fly-fields",
                    field.id
                ),
            ));
            continue;
        };
        insert_property_schema(
            &mut root,
            &segments,
            field_schema(field.kind, field.item_kind, field.default.as_ref()),
            field.required,
        );
    }

    for computed in &catalog.computed {
        if let Some(segments) = simple_object_path(&computed.path) {
            mark_computed_property(&mut root, &segments, &computed.id);
        }
    }

    if let Some(object) = root.as_object_mut() {
        object.insert(
            "x-fly-fields".to_string(),
            serde_json::to_value(&contract.fields).unwrap_or(Value::Array(Vec::new())),
        );
        object.insert(
            "x-fly-computed".to_string(),
            serde_json::to_value(&contract.computed).unwrap_or(Value::Array(Vec::new())),
        );
    }
    let bytes = serde_json::to_vec(&root).unwrap_or_default();
    RuntimeContextJsonSchema {
        schema: root,
        contract_hash: ProjectHash::from_bytes(&bytes).hex(),
        diagnostics,
    }
}

pub fn generate_runtime_context_example(
    document: &ProjectDocument,
    policy: RuntimeContextExamplePolicy,
) -> RuntimeContextExample {
    let catalog = ContextSchemaCatalog::from_document(document);
    let mut input_context = Value::Object(Map::new());
    let mut diagnostics = Vec::new();

    for field in &catalog.fields {
        let value = match field.default.as_ref() {
            Some(default) if field.required || policy.include_optional_defaults => {
                Some(default.clone())
            }
            Some(_) => None,
            None if field.required && policy.synthesize_required_values => {
                Some(example_value(field.kind, field.item_kind))
            }
            None => None,
        };
        let Some(value) = value else {
            continue;
        };
        if let Err(error) = set_context_path(&mut input_context, &field.path, value) {
            diagnostics.push(schema_diagnostic(
                ValidationSeverity::Warning,
                "runtime_context_example_write_failed",
                &field.path,
                format!(
                    "context example field `{}` could not be written: {error}",
                    field.id
                ),
            ));
        }
    }

    let materialized = materialize_context(document, &input_context);
    diagnostics.extend(materialized.diagnostics);
    RuntimeContextExample {
        input_context,
        effective_context: materialized.context,
        diagnostics,
    }
}

fn field_schema(
    kind: ContextValueKind,
    item_kind: Option<ContextValueKind>,
    default: Option<&Value>,
) -> Value {
    let mut schema = Map::new();
    if kind != ContextValueKind::Any {
        schema.insert(
            "type".to_string(),
            Value::String(json_schema_type(kind).to_string()),
        );
    }
    if kind == ContextValueKind::Array {
        if let Some(item_kind) = item_kind {
            let mut items = Map::new();
            if item_kind != ContextValueKind::Any {
                items.insert(
                    "type".to_string(),
                    Value::String(json_schema_type(item_kind).to_string()),
                );
            }
            schema.insert("items".to_string(), Value::Object(items));
        }
    }
    if let Some(default) = default {
        schema.insert("default".to_string(), default.clone());
    }
    Value::Object(schema)
}

fn insert_property_schema(root: &mut Value, segments: &[String], schema: Value, required: bool) {
    let Some((first, rest)) = segments.split_first() else {
        return;
    };

    if rest.is_empty() {
        {
            let object = root.as_object_mut().expect("root schema must be an object");
            let properties = object
                .entry("properties".to_string())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .expect("properties must be an object");
            properties.insert(first.clone(), schema);
        }
        if required {
            let object = root.as_object_mut().expect("root schema must be an object");
            push_required(object, first);
        }
        return;
    }

    let object = root.as_object_mut().expect("root schema must be an object");
    let properties = object
        .entry("properties".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("properties must be an object");
    let child = properties.entry(first.clone()).or_insert_with(|| {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    });
    insert_property_schema(child, rest, schema, required);
}

fn mark_computed_property(root: &mut Value, segments: &[String], computed_id: &str) {
    let Some((first, rest)) = segments.split_first() else {
        return;
    };
    let Some(object) = root.as_object_mut() else {
        return;
    };
    let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) else {
        return;
    };
    let child = properties.entry(first.clone()).or_insert_with(|| {
        if rest.is_empty() {
            Value::Object(Map::new())
        } else {
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": true
            })
        }
    });
    if rest.is_empty() {
        if let Some(schema) = child.as_object_mut() {
            schema.insert("readOnly".to_string(), Value::Bool(true));
            schema.insert(
                "x-fly-computed-id".to_string(),
                Value::String(computed_id.to_string()),
            );
        }
    } else {
        mark_computed_property(child, rest, computed_id);
    }
}

fn push_required(object: &mut Map<String, Value>, property: &str) {
    let required = object
        .entry("required".to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("required must be an array");
    if !required
        .iter()
        .any(|value| value.as_str() == Some(property))
    {
        required.push(Value::String(property.to_string()));
    }
}

fn simple_object_path(path: &str) -> Option<Vec<String>> {
    let path = path.trim();
    let path = path.strip_prefix('$').unwrap_or(path);
    let path = path.strip_prefix('.').unwrap_or(path);
    if path.is_empty()
        || path
            .chars()
            .any(|character| matches!(character, '[' | ']' | '{' | '}'))
    {
        return None;
    }
    let segments = path
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    (!segments.is_empty() && segments.join(".") == path).then_some(segments)
}

fn json_schema_type(kind: ContextValueKind) -> &'static str {
    match kind {
        ContextValueKind::Any => "object",
        ContextValueKind::Null => "null",
        ContextValueKind::Boolean => "boolean",
        ContextValueKind::Number => "number",
        ContextValueKind::String => "string",
        ContextValueKind::Object => "object",
        ContextValueKind::Array => "array",
    }
}

fn example_value(kind: ContextValueKind, item_kind: Option<ContextValueKind>) -> Value {
    match kind {
        ContextValueKind::Any | ContextValueKind::Null => Value::Null,
        ContextValueKind::Boolean => Value::Bool(false),
        ContextValueKind::Number => json!(0),
        ContextValueKind::String => Value::String(String::new()),
        ContextValueKind::Object => Value::Object(Map::new()),
        ContextValueKind::Array => item_kind
            .map(|kind| Value::Array(vec![example_value(kind, None)]))
            .unwrap_or_else(|| Value::Array(Vec::new())),
    }
}

fn schema_diagnostic(
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

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [{
                "id": "title",
                "path": "page.title",
                "kind": "string",
                "required": true,
                "default": "Welcome"
            }, {
                "id": "items",
                "path": "catalog.items",
                "kind": "array",
                "item_kind": "object"
            }],
            "flyRuntimeComputed": [{
                "id": "display-title",
                "path": "page.displayTitle",
                "expression": {
                    "op": "format",
                    "template": "{{page.title}}!"
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn exports_nested_json_schema_with_required_and_computed_metadata() {
        let exported = export_runtime_context_json_schema(&document());
        assert_eq!(exported.schema["type"], "object");
        assert_eq!(
            exported.schema["properties"]["page"]["properties"]["title"]["type"],
            "string"
        );
        assert_eq!(
            exported.schema["properties"]["page"]["required"][0],
            "title"
        );
        assert_eq!(
            exported.schema["properties"]["page"]["properties"]["displayTitle"]["readOnly"],
            true
        );
        assert!(!exported.contract_hash.is_empty());
    }

    #[test]
    fn generates_example_and_materializes_computed_values() {
        let example =
            generate_runtime_context_example(&document(), RuntimeContextExamplePolicy::default());
        assert_eq!(example.input_context["page"]["title"], "Welcome");
        assert_eq!(
            example.effective_context["page"]["displayTitle"],
            "Welcome!"
        );
    }
}
