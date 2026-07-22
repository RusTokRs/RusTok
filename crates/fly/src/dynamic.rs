use crate::{
    ComponentChildren, ComponentNode, ComponentObject, FLY_COMPONENT_RULE_FIELD, FLY_RULE_ID_FIELD,
    FlyError, FlyResult, ProjectDocument, StyleRuleDescriptor, ValidationDiagnostic,
    ValidationSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_RUNTIME_CONDITIONS_FIELD: &str = "flyRuntimeConditions";
pub const FLY_RUNTIME_REPEATERS_FIELD: &str = "flyRuntimeRepeaters";
pub const DEFAULT_REPEATER_LIMIT: usize = 100;
pub const MAX_REPEATER_LIMIT: usize = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Exists,
    Equals,
    NotEquals,
    Truthy,
    Falsy,
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCondition {
    pub id: String,
    pub component_id: String,
    pub path: String,
    pub operator: ConditionOperator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(default)]
    pub invert: bool,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmptyRepeaterBehavior {
    #[default]
    Hide,
    KeepTemplate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeRepeater {
    pub id: String,
    pub component_id: String,
    pub path: String,
    #[serde(default = "default_item_alias")]
    pub item_alias: String,
    #[serde(default = "default_index_alias")]
    pub index_alias: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default)]
    pub empty_behavior: EmptyRepeaterBehavior,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DynamicCommand {
    UpsertCondition { condition: RuntimeCondition },
    RemoveCondition { condition_id: String },
    UpsertRepeater { repeater: RuntimeRepeater },
    RemoveRepeater { repeater_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DynamicCatalog {
    pub conditions: Vec<RuntimeCondition>,
    pub repeaters: Vec<RuntimeRepeater>,
    pub unknown_condition_entries: Vec<Value>,
    pub unknown_repeater_entries: Vec<Value>,
}

impl DynamicCatalog {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let (conditions, unknown_condition_entries) = decode_entries(
            document
                .project
                .extensions
                .get(FLY_RUNTIME_CONDITIONS_FIELD),
        );
        let (repeaters, unknown_repeater_entries) =
            decode_entries(document.project.extensions.get(FLY_RUNTIME_REPEATERS_FIELD));
        Self {
            conditions,
            repeaters,
            unknown_condition_entries,
            unknown_repeater_entries,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeMaterialization {
    pub document: ProjectDocument,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub evaluated_conditions: usize,
    pub hidden_components: usize,
    pub repeated_nodes: usize,
}

pub fn apply_dynamic_command(
    document: &mut ProjectDocument,
    command: &DynamicCommand,
) -> FlyResult<()> {
    let mut catalog = DynamicCatalog::from_document(document);
    match command {
        DynamicCommand::UpsertCondition { condition } => {
            validate_definition_identity(
                document,
                &condition.id,
                &condition.component_id,
                &condition.path,
            )?;
            upsert_by_id(&mut catalog.conditions, condition.clone(), |value| {
                &value.id
            });
        }
        DynamicCommand::RemoveCondition { condition_id } => {
            let before = catalog.conditions.len();
            catalog
                .conditions
                .retain(|condition| condition.id != *condition_id);
            if catalog.conditions.len() == before {
                return Err(FlyError::Decode(format!(
                    "runtime condition `{condition_id}` was not found"
                )));
            }
        }
        DynamicCommand::UpsertRepeater { repeater } => {
            validate_definition_identity(
                document,
                &repeater.id,
                &repeater.component_id,
                &repeater.path,
            )?;
            if repeater
                .limit
                .is_some_and(|limit| limit > MAX_REPEATER_LIMIT)
            {
                return Err(FlyError::Decode(format!(
                    "runtime repeater limit must not exceed {MAX_REPEATER_LIMIT}"
                )));
            }
            upsert_by_id(&mut catalog.repeaters, repeater.clone(), |value| &value.id);
        }
        DynamicCommand::RemoveRepeater { repeater_id } => {
            let before = catalog.repeaters.len();
            catalog
                .repeaters
                .retain(|repeater| repeater.id != *repeater_id);
            if catalog.repeaters.len() == before {
                return Err(FlyError::Decode(format!(
                    "runtime repeater `{repeater_id}` was not found"
                )));
            }
        }
    }
    write_catalog(document, catalog)
}

pub fn materialize_runtime(document: &ProjectDocument, context: &Value) -> RuntimeMaterialization {
    let catalog = DynamicCatalog::from_document(document);
    let mut materialized = document.clone();
    let mut diagnostics = Vec::new();
    let mut hidden_components = 0usize;
    let original_styles = materialized.project.styles.clone();

    for condition in &catalog.conditions {
        let matched = evaluate_condition(condition, context);
        if !matched {
            match hide_runtime_component(&mut materialized, &condition.component_id) {
                Ok(()) => hidden_components = hidden_components.saturating_add(1),
                Err(error) => diagnostics.push(runtime_diagnostic(
                    ValidationSeverity::Warning,
                    "runtime_condition_target_missing",
                    Some(condition.component_id.clone()),
                    error.to_string(),
                )),
            }
        }
    }

    let mut repeated_nodes = 0usize;
    for repeater in &catalog.repeaters {
        if !materialized.contains_component(&repeater.component_id) {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Info,
                "runtime_repeater_target_hidden",
                Some(repeater.component_id.clone()),
                format!(
                    "repeater `{}` target is not present after condition evaluation",
                    repeater.id
                ),
            ));
            continue;
        }
        match expand_repeater(&mut materialized, repeater, context, &original_styles) {
            Ok(count) => repeated_nodes = repeated_nodes.saturating_add(count),
            Err(error) => diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_repeater_failed",
                Some(repeater.component_id.clone()),
                error.to_string(),
            )),
        }
    }

    RuntimeMaterialization {
        document: materialized,
        diagnostics,
        evaluated_conditions: catalog.conditions.len(),
        hidden_components,
        repeated_nodes,
    }
}

pub fn validate_dynamic_definitions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let catalog = DynamicCatalog::from_document(document);
    let mut diagnostics = Vec::new();
    let mut ids = BTreeSet::new();

    for condition in &catalog.conditions {
        validate_common_definition(
            document,
            "condition",
            &condition.id,
            &condition.component_id,
            &condition.path,
            &mut ids,
            &mut diagnostics,
        );
        if matches!(
            condition.operator,
            ConditionOperator::Equals | ConditionOperator::NotEquals | ConditionOperator::Contains
        ) && condition.expected.is_none()
        {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_condition_expected_missing",
                Some(condition.component_id.clone()),
                format!(
                    "condition `{}` uses an operator that normally requires `expected`",
                    condition.id
                ),
            ));
        }
    }

    for repeater in &catalog.repeaters {
        validate_common_definition(
            document,
            "repeater",
            &repeater.id,
            &repeater.component_id,
            &repeater.path,
            &mut ids,
            &mut diagnostics,
        );
        if document
            .component_location(&repeater.component_id)
            .is_some_and(|location| location.parent_component_id.is_none())
        {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_targets_page_root",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` cannot target a page root", repeater.id),
            ));
        }
        if repeater.item_alias.trim().is_empty() || repeater.index_alias.trim().is_empty() {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_alias_empty",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` aliases must not be empty", repeater.id),
            ));
        }
        if repeater.limit == Some(0) {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_repeater_zero_limit",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` has a zero item limit", repeater.id),
            ));
        }
        if repeater
            .limit
            .is_some_and(|limit| limit > MAX_REPEATER_LIMIT)
        {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_limit_exceeded",
                Some(repeater.component_id.clone()),
                format!(
                    "repeater `{}` exceeds maximum limit {MAX_REPEATER_LIMIT}",
                    repeater.id
                ),
            ));
        }
    }

    if !catalog.unknown_condition_entries.is_empty() {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Info,
            "opaque_runtime_conditions",
            None,
            format!(
                "{} runtime condition entries are opaque and preserved",
                catalog.unknown_condition_entries.len()
            ),
        ));
    }
    if !catalog.unknown_repeater_entries.is_empty() {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Info,
            "opaque_runtime_repeaters",
            None,
            format!(
                "{} runtime repeater entries are opaque and preserved",
                catalog.unknown_repeater_entries.len()
            ),
        ));
    }

    diagnostics
}

fn default_item_alias() -> String {
    "item".to_string()
}

fn default_index_alias() -> String {
    "index".to_string()
}

fn decode_entries<T>(value: Option<&Value>) -> (Vec<T>, Vec<Value>)
where
    T: for<'de> Deserialize<'de>,
{
    let mut known = Vec::new();
    let mut unknown = Vec::new();
    let Some(Value::Array(entries)) = value else {
        return (known, unknown);
    };
    for entry in entries {
        match serde_json::from_value::<T>(entry.clone()) {
            Ok(value) => known.push(value),
            Err(_) => unknown.push(entry.clone()),
        }
    }
    (known, unknown)
}

fn write_catalog(document: &mut ProjectDocument, catalog: DynamicCatalog) -> FlyResult<()> {
    write_entries(
        document,
        FLY_RUNTIME_CONDITIONS_FIELD,
        catalog.conditions,
        catalog.unknown_condition_entries,
    )?;
    write_entries(
        document,
        FLY_RUNTIME_REPEATERS_FIELD,
        catalog.repeaters,
        catalog.unknown_repeater_entries,
    )
}

fn write_entries<T>(
    document: &mut ProjectDocument,
    field: &str,
    known: Vec<T>,
    unknown: Vec<Value>,
) -> FlyResult<()>
where
    T: Serialize,
{
    let mut entries = known
        .into_iter()
        .map(|entry| {
            serde_json::to_value(entry).map_err(|error| FlyError::Encode(error.to_string()))
        })
        .collect::<FlyResult<Vec<_>>>()?;
    entries.extend(unknown);
    if entries.is_empty() {
        document.project.extensions.remove(field);
    } else {
        document
            .project
            .extensions
            .insert(field.to_string(), Value::Array(entries));
    }
    Ok(())
}

fn validate_definition_identity(
    document: &ProjectDocument,
    id: &str,
    component_id: &str,
    path: &str,
) -> FlyResult<()> {
    if id.trim().is_empty() {
        return Err(FlyError::Decode(
            "runtime definition id must not be empty".to_string(),
        ));
    }
    if path.trim().is_empty() {
        return Err(FlyError::Decode(
            "runtime definition path must not be empty".to_string(),
        ));
    }
    if !document.contains_component(component_id) {
        return Err(FlyError::ComponentNotFound(component_id.to_string()));
    }
    Ok(())
}

fn upsert_by_id<T>(values: &mut Vec<T>, value: T, id: impl Fn(&T) -> &str) {
    let target = id(&value).to_string();
    if let Some(index) = values.iter().position(|candidate| id(candidate) == target) {
        values[index] = value;
    } else {
        values.push(value);
    }
}

fn validate_common_definition(
    document: &ProjectDocument,
    kind: &str,
    id: &str,
    component_id: &str,
    path: &str,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if id.trim().is_empty() {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "runtime_definition_id_empty",
            Some(component_id.to_string()),
            format!("runtime {kind} id must not be empty"),
        ));
    } else if !ids.insert(id.to_string()) {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "duplicate_runtime_definition_id",
            Some(component_id.to_string()),
            format!("runtime definition id `{id}` is duplicated"),
        ));
    }
    if path.trim().is_empty() {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "runtime_definition_path_empty",
            Some(component_id.to_string()),
            format!("runtime {kind} `{id}` path must not be empty"),
        ));
    }
    if !document.contains_component(component_id) {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "runtime_definition_target_missing",
            Some(component_id.to_string()),
            format!("runtime {kind} `{id}` targets missing component `{component_id}`"),
        ));
    }
}

fn evaluate_condition(condition: &RuntimeCondition, context: &Value) -> bool {
    let resolved = resolve_path(context, &condition.path);
    let matched = match condition.operator {
        ConditionOperator::Exists => resolved.is_some_and(|value| !value.is_null()),
        ConditionOperator::Equals => resolved == condition.expected.as_ref(),
        ConditionOperator::NotEquals => resolved != condition.expected.as_ref(),
        ConditionOperator::Truthy => resolved.is_some_and(is_truthy),
        ConditionOperator::Falsy => resolved.is_none_or(|value| !is_truthy(value)),
        ConditionOperator::Contains => {
            resolved.is_some_and(|value| contains(value, condition.expected.as_ref()))
        }
    };
    if condition.invert { !matched } else { matched }
}

fn contains(value: &Value, expected: Option<&Value>) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    match value {
        Value::Array(values) => values.iter().any(|value| value == expected),
        Value::String(value) => expected
            .as_str()
            .is_some_and(|expected| value.contains(expected)),
        Value::Object(values) => expected
            .as_str()
            .is_some_and(|expected| values.contains_key(expected)),
        _ => false,
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

fn expand_repeater(
    document: &mut ProjectDocument,
    repeater: &RuntimeRepeater,
    context: &Value,
    original_styles: &[Value],
) -> FlyResult<usize> {
    let location = document
        .component_location(&repeater.component_id)
        .ok_or_else(|| FlyError::ComponentNotFound(repeater.component_id.clone()))?;
    let Some(parent_id) = location.parent_component_id.clone() else {
        return Err(FlyError::Decode(format!(
            "runtime repeater `{}` cannot target a page root",
            repeater.id
        )));
    };
    let template = ComponentNode::Object(Box::new(
        document
            .component(&repeater.component_id)
            .ok_or_else(|| FlyError::ComponentNotFound(repeater.component_id.clone()))?
            .clone(),
    ));
    let values = resolve_path(context, &repeater.path)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let limit = repeater
        .limit
        .unwrap_or(DEFAULT_REPEATER_LIMIT)
        .min(MAX_REPEATER_LIMIT);
    let values = values.into_iter().take(limit).collect::<Vec<_>>();

    if values.is_empty() {
        if repeater.empty_behavior == EmptyRepeaterBehavior::Hide {
            document.project.remove_component(&repeater.component_id)?;
        }
        return Ok(0);
    }

    let mut source_ids = Vec::new();
    template.collect_ids(&mut source_ids);
    let source_ids = source_ids.into_iter().collect::<BTreeSet<_>>();
    document.project.remove_component(&repeater.component_id)?;

    let mut generated_styles = Vec::new();
    for (index, item) in values.iter().enumerate() {
        let mapping = source_ids
            .iter()
            .map(|source| {
                (
                    source.clone(),
                    format!(
                        "{}--{}-{}",
                        source,
                        sanitize_identifier(&repeater.id),
                        index
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut clone = template.clone();
        clone.remap_ids(&mapping);
        let local_context = build_local_context(
            context,
            &repeater.item_alias,
            item.clone(),
            &repeater.index_alias,
            index,
        );
        interpolate_node(&mut clone, &local_context);
        document.project.insert_component(
            Some(parent_id.as_str()),
            location.index + index,
            clone,
        )?;
        generated_styles.extend(remap_styles(original_styles, &mapping, &repeater.id, index));
    }
    document.project.styles.extend(generated_styles);
    Ok(values.len())
}

fn hide_runtime_component(document: &mut ProjectDocument, component_id: &str) -> FlyResult<()> {
    let location = document
        .component_location(component_id)
        .ok_or_else(|| FlyError::ComponentNotFound(component_id.to_string()))?;
    if location.parent_component_id.is_some() {
        document.project.remove_component(component_id)?;
        return Ok(());
    }
    let page = document
        .project
        .pages
        .get_mut(location.page_index)
        .ok_or_else(|| FlyError::PageNotFound(location.page_index.to_string()))?;
    page.component = Some(ComponentNode::Object(Box::new(ComponentObject {
        id: Some(component_id.to_string()),
        component_type: Some("wrapper".to_string()),
        style: Some(Value::Object(Map::from_iter([(
            "display".to_string(),
            Value::String("none".to_string()),
        )]))),
        components: ComponentChildren::Nodes(Vec::new()),
        ..ComponentObject::default()
    })));
    Ok(())
}

fn remap_styles(
    styles: &[Value],
    mapping: &BTreeMap<String, String>,
    repeater_id: &str,
    index: usize,
) -> Vec<Value> {
    styles
        .iter()
        .filter_map(|raw| {
            let descriptor = StyleRuleDescriptor::from_value(raw.clone())?;
            let source_id = descriptor.component_id.as_ref()?;
            let target_id = mapping.get(source_id)?;
            let mut value = raw.clone();
            replace_exact_references(&mut value, mapping);
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    FLY_COMPONENT_RULE_FIELD.to_string(),
                    Value::String(target_id.clone()),
                );
                object.insert(
                    FLY_RULE_ID_FIELD.to_string(),
                    Value::String(format!(
                        "{}--{}-{}",
                        descriptor.id,
                        sanitize_identifier(repeater_id),
                        index
                    )),
                );
            }
            Some(value)
        })
        .collect()
}

fn build_local_context(
    root: &Value,
    item_alias: &str,
    item: Value,
    index_alias: &str,
    index: usize,
) -> Value {
    let mut object = root.as_object().cloned().unwrap_or_default();
    object.insert(item_alias.to_string(), item);
    object.insert(
        index_alias.to_string(),
        Value::Number(Number::from(index as u64)),
    );
    Value::Object(object)
}

fn interpolate_node(node: &mut ComponentNode, context: &Value) {
    match node {
        ComponentNode::Opaque(value) => interpolate_value(value, context),
        ComponentNode::Object(component) => {
            for value in component.attributes.values_mut() {
                interpolate_value(value, context);
            }
            if let Some(style) = component.style.as_mut() {
                interpolate_value(style, context);
            }
            for value in &mut component.traits {
                interpolate_value(value, context);
            }
            for value in component.extensions.values_mut() {
                interpolate_value(value, context);
            }
            if let Some(children) = component.children_mut() {
                for child in children {
                    interpolate_node(child, context);
                }
            }
        }
    }
}

fn interpolate_value(value: &mut Value, context: &Value) {
    match value {
        Value::String(text) => {
            if let Some(path) = exact_template_path(text) {
                if let Some(resolved) = resolve_path(context, path) {
                    *value = resolved.clone();
                }
                return;
            }
            *text = interpolate_text(text, context);
        }
        Value::Array(values) => {
            for value in values {
                interpolate_value(value, context);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                interpolate_value(value, context);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn exact_template_path(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn interpolate_text(value: &str, context: &Value) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 2..];
        let Some(end) = after.find("}}") else {
            output.push_str(&remaining[start..]);
            return output;
        };
        let path = after[..end].trim();
        if let Some(resolved) = resolve_path(context, path) {
            output.push_str(&scalar_text(resolved));
        }
        remaining = &after[end + 2..];
    }
    output.push_str(remaining);
    output
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
                for character in chars.by_ref() {
                    if character == ']' {
                        break;
                    }
                    index.push(character);
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

fn replace_exact_references(value: &mut Value, mapping: &BTreeMap<String, String>) {
    match value {
        Value::String(value) => {
            if let Some(replacement) = mapping.get(value) {
                *value = replacement.clone();
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_exact_references(value, mapping);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_exact_references(value, mapping);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn sanitize_identifier(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim_matches('-');
    if value.is_empty() {
        "repeat".to_string()
    } else {
        value.to_string()
    }
}

fn runtime_diagnostic(
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
            .unwrap_or_else(|| "project.runtime".to_string()),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsCodec, StyleRuleCatalog};
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "styles": [{
                "selectors": [{ "name": "card", "type": 2 }],
                "style": { "padding": "12px" },
                "flyComponentId": "card"
            }],
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "banner",
                        "type": "section",
                        "content": "Visible"
                    }, {
                        "id": "card",
                        "type": "section",
                        "components": [{
                            "id": "card-title",
                            "type": "heading",
                            "content": "{{item.title}} #{{index}}"
                        }]
                    }]
                }
            }],
            "flyRuntimeConditions": [{
                "id": "show-banner",
                "component_id": "banner",
                "path": "flags.banner",
                "operator": "truthy"
            }],
            "flyRuntimeRepeaters": [{
                "id": "cards",
                "component_id": "card",
                "path": "items",
                "item_alias": "item",
                "index_alias": "index"
            }]
        }))
        .expect("document")
    }

    #[test]
    fn conditions_hide_components_without_mutating_source() {
        let source = document();
        let materialized = materialize_runtime(
            &source,
            &json!({ "flags": { "banner": false }, "items": [] }),
        );
        assert!(source.contains_component("banner"));
        assert!(!materialized.document.contains_component("banner"));
        assert_eq!(materialized.hidden_components, 1);
    }

    #[test]
    fn repeaters_clone_interpolate_and_remap_style_rules() {
        let source = document();
        let materialized = materialize_runtime(
            &source,
            &json!({
                "flags": { "banner": true },
                "items": [{ "title": "One" }, { "title": "Two" }]
            }),
        );
        assert_eq!(materialized.repeated_nodes, 2);
        assert!(materialized.document.contains_component("card--cards-0"));
        assert!(
            materialized
                .document
                .contains_component("card-title--cards-1")
        );
        assert_eq!(
            materialized
                .document
                .component("card-title--cards-1")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Two #1")
        );
        assert!(
            StyleRuleCatalog::from_document(&materialized.document)
                .component_rules("card--cards-0")
                .next()
                .is_some()
        );
        assert!(source.contains_component("card"));
    }

    #[test]
    fn commands_preserve_unknown_entries() {
        let mut document = document();
        document.project.extensions.insert(
            FLY_RUNTIME_CONDITIONS_FIELD.to_string(),
            json!([{ "providerCondition": true }]),
        );
        apply_dynamic_command(
            &mut document,
            &DynamicCommand::UpsertCondition {
                condition: RuntimeCondition {
                    id: "show-card".to_string(),
                    component_id: "card".to_string(),
                    path: "flags.card".to_string(),
                    operator: ConditionOperator::Truthy,
                    expected: None,
                    invert: false,
                    extensions: Map::new(),
                },
            },
        )
        .expect("upsert condition");
        let entries = document.project.extensions[FLY_RUNTIME_CONDITIONS_FIELD]
            .as_array()
            .expect("condition array");
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|entry| entry.get("providerCondition").is_some())
        );
    }
}
