use crate::{FlyError, FlyResult, ProjectDocument, ValidationDiagnostic, ValidationSeverity};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_RUNTIME_CONTEXT_SCHEMA_FIELD: &str = "flyRuntimeContextSchema";
pub const FLY_RUNTIME_COMPUTED_FIELD: &str = "flyRuntimeComputed";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContextValueKind {
    #[default]
    Any,
    Null,
    Boolean,
    Number,
    String,
    Object,
    Array,
}

impl ContextValueKind {
    pub fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Any => true,
            Self::Null => value.is_null(),
            Self::Boolean => value.is_boolean(),
            Self::Number => value.is_number(),
            Self::String => value.is_string(),
            Self::Object => value.is_object(),
            Self::Array => value.is_array(),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Number => "number",
            Self::String => "string",
            Self::Object => "object",
            Self::Array => "array",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextFieldDefinition {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub kind: ContextValueKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_kind: Option<ContextValueKind>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ContextExpression {
    Literal {
        value: Value,
    },
    Path {
        path: String,
    },
    Coalesce {
        values: Vec<ContextExpression>,
    },
    Concat {
        values: Vec<ContextExpression>,
        #[serde(default)]
        separator: String,
    },
    Add {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    Subtract {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    Multiply {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    Divide {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    Equals {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    NotEquals {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    GreaterThan {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    LessThan {
        left: Box<ContextExpression>,
        right: Box<ContextExpression>,
    },
    And {
        values: Vec<ContextExpression>,
    },
    Or {
        values: Vec<ContextExpression>,
    },
    Not {
        value: Box<ContextExpression>,
    },
    If {
        condition: Box<ContextExpression>,
        then_value: Box<ContextExpression>,
        else_value: Box<ContextExpression>,
    },
    Format {
        template: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputedContextValue {
    pub id: String,
    pub path: String,
    pub expression: ContextExpression,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Value>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ContextCommand {
    UpsertField { field: ContextFieldDefinition },
    RemoveField { field_id: String },
    UpsertComputed { computed: ComputedContextValue },
    RemoveComputed { computed_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContextSchemaCatalog {
    pub fields: Vec<ContextFieldDefinition>,
    pub computed: Vec<ComputedContextValue>,
    pub unknown_field_entries: Vec<Value>,
    pub unknown_computed_entries: Vec<Value>,
}

impl ContextSchemaCatalog {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let (fields, unknown_field_entries) = decode_entries(
            document
                .project()
                .extensions
                .get(FLY_RUNTIME_CONTEXT_SCHEMA_FIELD),
        );
        let (computed, unknown_computed_entries) = decode_entries(
            document
                .project()
                .extensions
                .get(FLY_RUNTIME_COMPUTED_FIELD),
        );
        Self {
            fields,
            computed,
            unknown_field_entries,
            unknown_computed_entries,
        }
    }

    pub fn field_for_path(&self, path: &str) -> Option<&ContextFieldDefinition> {
        self.fields.iter().find(|field| field.path == path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextMaterialization {
    pub context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub defaults_applied: usize,
    pub computed_applied: usize,
    pub computed_fallbacks: usize,
    pub unresolved_computed: usize,
    pub type_mismatches: usize,
}

pub fn apply_context_command(
    document: &mut ProjectDocument,
    command: &ContextCommand,
) -> FlyResult<()> {
    let mut catalog = ContextSchemaCatalog::from_document(document);
    match command {
        ContextCommand::UpsertField { field } => {
            validate_field_identity(field)?;
            upsert_by_id(&mut catalog.fields, field.clone(), |value| &value.id);
        }
        ContextCommand::RemoveField { field_id } => {
            remove_by_id(
                &mut catalog.fields,
                field_id,
                |value| &value.id,
                "context field",
            )?;
        }
        ContextCommand::UpsertComputed { computed } => {
            validate_computed_identity(computed)?;
            upsert_by_id(&mut catalog.computed, computed.clone(), |value| &value.id);
        }
        ContextCommand::RemoveComputed { computed_id } => {
            remove_by_id(
                &mut catalog.computed,
                computed_id,
                |value| &value.id,
                "computed context value",
            )?;
        }
    }
    write_catalog(document, catalog)
}

pub fn materialize_context(document: &ProjectDocument, input: &Value) -> ContextMaterialization {
    let catalog = ContextSchemaCatalog::from_document(document);
    let mut context = normalize_context(input.clone());
    let mut diagnostics = Vec::new();
    let mut defaults_applied = 0usize;

    for field in &catalog.fields {
        if resolve_context_path(&context, &field.path).is_none()
            && let Some(default) = field.default.clone()
        {
            match set_context_path(&mut context, &field.path, default) {
                Ok(()) => defaults_applied = defaults_applied.saturating_add(1),
                Err(error) => diagnostics.push(context_diagnostic(
                    ValidationSeverity::Warning,
                    "runtime_context_default_failed",
                    &field.path,
                    format!("field `{}` default could not be applied: {error}", field.id),
                )),
            }
        }
    }

    let (order, graph_diagnostics) = computed_evaluation_order(&catalog.computed);
    let mut computed_applied = 0usize;
    let mut computed_fallbacks = 0usize;
    let mut unresolved_computed = catalog.computed.len().saturating_sub(order.len());
    diagnostics.extend(graph_diagnostics);

    // Computed slots belong to producers, not host input or schema defaults.
    // Clear stale values even for failed or blocked producers without shifting
    // neighboring array slots.
    for computed in &catalog.computed {
        if let Some(segments) = parse_context_path(&computed.path) {
            remove_path_segments(&mut context, &segments, false);
        }
    }

    // All declared producers are settled before consumers, even when an input
    // or a schema default already populated the producer's target.
    for index in order {
        let computed = &catalog.computed[index];
        let (value, fallback_reason) = match evaluate_expression(&computed.expression, &context) {
            ExpressionEvaluation::Value(value) => (Some(value), None),
            ExpressionEvaluation::Pending => (
                computed.fallback.clone(),
                Some("dependencies did not resolve".to_string()),
            ),
            ExpressionEvaluation::Error(error) => (computed.fallback.clone(), Some(error)),
        };
        let Some(value) = value else {
            unresolved_computed += 1;
            let (code, reason) = match fallback_reason.as_deref() {
                Some("dependencies did not resolve") => (
                    "runtime_computed_unresolved",
                    "dependencies did not resolve",
                ),
                Some(reason) => ("runtime_computed_evaluation_failed", reason),
                None => ("runtime_computed_unresolved", "no value"),
            };
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Warning,
                code,
                &computed.path,
                format!("computed value `{}` failed: {reason}", computed.id),
            ));
            continue;
        };
        match set_context_path(&mut context, &computed.path, value) {
            Ok(()) => {
                computed_applied += 1;
                if let Some(reason) = fallback_reason {
                    computed_fallbacks += 1;
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Info,
                        "runtime_computed_fallback_used",
                        &computed.path,
                        format!("computed value `{}` used fallback: {reason}", computed.id),
                    ));
                }
            }
            Err(error) => {
                unresolved_computed += 1;
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Warning,
                    "runtime_computed_write_failed",
                    &computed.path,
                    format!(
                        "computed value `{}` could not be written: {error}",
                        computed.id
                    ),
                ));
            }
        }
    }

    let (type_mismatches, field_diagnostics) = validate_context_fields(&catalog, &context);
    diagnostics.extend(field_diagnostics);

    ContextMaterialization {
        context,
        diagnostics,
        defaults_applied,
        computed_applied,
        computed_fallbacks,
        unresolved_computed,
        type_mismatches,
    }
}

/// Validate the supplied values without applying defaults or evaluating computed producers.
/// Materialization and migration share this exact field contract.
pub(crate) fn validate_context_fields(
    catalog: &ContextSchemaCatalog,
    context: &Value,
) -> (usize, Vec<ValidationDiagnostic>) {
    let mut diagnostics = Vec::new();
    let mut type_mismatches = 0usize;
    for field in &catalog.fields {
        match resolve_context_path(context, &field.path) {
            Some(value) => {
                if !field.kind.accepts(value) {
                    type_mismatches = type_mismatches.saturating_add(1);
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Warning,
                        "runtime_context_type_mismatch",
                        &field.path,
                        format!(
                            "field `{}` expects {} but received {}",
                            field.id,
                            field.kind.as_str(),
                            value_kind(value)
                        ),
                    ));
                } else if let (ContextValueKind::Array, Some(item_kind), Value::Array(items)) =
                    (field.kind, field.item_kind, value)
                {
                    let mismatches = items.iter().filter(|item| !item_kind.accepts(item)).count();
                    if mismatches > 0 {
                        type_mismatches = type_mismatches.saturating_add(mismatches);
                        diagnostics.push(context_diagnostic(
                            ValidationSeverity::Warning,
                            "runtime_context_array_item_type_mismatch",
                            &field.path,
                            format!(
                                "field `{}` contains {mismatches} items that are not {}",
                                field.id,
                                item_kind.as_str()
                            ),
                        ));
                    }
                }
            }
            None if field.required => diagnostics.push(context_diagnostic(
                ValidationSeverity::Warning,
                "runtime_context_required_missing",
                &field.path,
                format!("required context field `{}` is missing", field.id),
            )),
            None => {}
        }
    }

    (type_mismatches, diagnostics)
}

pub fn validate_context_definitions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let catalog = ContextSchemaCatalog::from_document(document);
    let mut diagnostics = Vec::new();
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();

    for field in &catalog.fields {
        validate_definition_id(
            "context field",
            &field.id,
            &field.path,
            &mut ids,
            &mut diagnostics,
        );
        if parse_context_path(&field.path).is_some_and(|path| !paths.insert(path)) {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "duplicate_runtime_context_field_path",
                &field.path,
                format!("multiple context fields define path `{}`", field.path),
            ));
        }
        if parse_context_path(&field.path).is_none() {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_context_field_path_invalid",
                &field.path,
                format!("field `{}` path is invalid", field.id),
            ));
        }
        if field.item_kind.is_some() && field.kind != ContextValueKind::Array {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_context_item_kind_without_array",
                &field.path,
                format!(
                    "field `{}` declares item_kind but is not an array",
                    field.id
                ),
            ));
        }
        if let Some(default) = field.default.as_ref() {
            if !field.kind.accepts(default) {
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_context_default_type_mismatch",
                    &field.path,
                    format!(
                        "field `{}` default is not {}",
                        field.id,
                        field.kind.as_str()
                    ),
                ));
            }
            if let (ContextValueKind::Array, Some(item_kind), Value::Array(items)) =
                (field.kind, field.item_kind, default)
                && items.iter().any(|item| !item_kind.accepts(item))
            {
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_context_default_item_type_mismatch",
                    &field.path,
                    format!(
                        "field `{}` default contains items that are not {}",
                        field.id,
                        item_kind.as_str()
                    ),
                ));
            }
        }
    }

    // A field path that is a strict ancestor of another field's path.
    //
    // Only exact duplicates were rejected, so a schema could declare both `user` and `user.name`
    // and validate clean. The generated JSON Schema then depends on declaration order: inserting
    // `user` first yields `{"type":"string","properties":{"name":...}}` — a string carrying
    // properties, which every validator ignores — while inserting `user.name` first means the
    // later `user` simply overwrites it and the nested field disappears. Both outcomes are
    // silent, and neither is what the author wrote.
    diagnostics.extend(conflicting_field_paths(&catalog));

    let mut computed_paths = BTreeMap::<Vec<ContextPathSegment>, String>::new();
    for computed in &catalog.computed {
        validate_definition_id(
            "computed context value",
            &computed.id,
            &computed.path,
            &mut ids,
            &mut diagnostics,
        );
        if parse_context_path(&computed.path).is_none() {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_computed_path_invalid",
                &computed.path,
                format!("computed value `{}` path is invalid", computed.id),
            ));
        }
        if let Some(previous) = parse_context_path(&computed.path)
            .and_then(|path| computed_paths.insert(path, computed.id.clone()))
        {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "duplicate_runtime_computed_path",
                &computed.path,
                format!(
                    "computed values `{previous}` and `{}` write the same path",
                    computed.id
                ),
            ));
        }
        let mut dependencies = BTreeSet::new();
        collect_expression_paths(&computed.expression, &mut dependencies);
        for dependency in dependencies {
            if parse_context_path(&dependency).is_none() {
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_computed_dependency_path_invalid",
                    &computed.path,
                    format!(
                        "computed value `{}` contains invalid dependency path `{dependency}`",
                        computed.id
                    ),
                ));
            }
            if parse_context_path(&dependency)
                .is_some_and(|path| Some(path) == parse_context_path(&computed.path))
            {
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_computed_self_reference",
                    &computed.path,
                    format!("computed value `{}` references itself", computed.id),
                ));
            }
        }
        validate_expression_shape(
            &computed.expression,
            &computed.path,
            &computed.id,
            &mut diagnostics,
        );
    }

    // A computed producer owns its complete target subtree. A declared input
    // field must not occupy that path or any ancestor/descendant path, because
    // materialization clears producer-owned values before evaluation.
    let declared_fields = catalog
        .fields
        .iter()
        .filter_map(|field| parse_context_path(&field.path).map(|path| (field, path)))
        .collect::<Vec<_>>();
    for computed in &catalog.computed {
        let Some(computed_path) = parse_context_path(&computed.path) else {
            continue;
        };
        for (field, field_path) in &declared_fields {
            if field_path.starts_with(&computed_path) || computed_path.starts_with(field_path) {
                let (code, message) = if field_path == &computed_path {
                    (
                        "runtime_context_path_shadowed_by_computed",
                        format!(
                            "runtime path `{}` is declared as input and also written by a computed value",
                            field.path
                        ),
                    )
                } else {
                    (
                        "runtime_context_field_computed_target_overlap",
                        format!(
                            "computed value {} target {} overlaps declared input field {} path {}; input and computed paths must not overlap",
                            computed.id, computed.path, field.id, field.path
                        ),
                    )
                };
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Error,
                    code,
                    &computed.path,
                    message,
                ));
            }
        }
    }

    diagnostics.extend(detect_computed_cycles(&catalog.computed));

    if !catalog.unknown_field_entries.is_empty() {
        diagnostics.push(context_diagnostic(
            ValidationSeverity::Info,
            "opaque_runtime_context_fields",
            "project.runtime.context",
            format!(
                "{} context schema entries are opaque and preserved",
                catalog.unknown_field_entries.len()
            ),
        ));
    }
    if !catalog.unknown_computed_entries.is_empty() {
        diagnostics.push(context_diagnostic(
            ValidationSeverity::Info,
            "opaque_runtime_computed_values",
            "project.runtime.computed",
            format!(
                "{} computed context entries are opaque and preserved",
                catalog.unknown_computed_entries.len()
            ),
        ));
    }

    diagnostics
}

pub fn resolve_context_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for segment in parse_context_path(path)? {
        current = match segment {
            ContextPathSegment::Key(key) => current.as_object()?.get(&key)?,
            ContextPathSegment::Index(index) => current.as_array()?.get(index)?,
        };
    }
    Some(current)
}

pub fn set_context_path(root: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let segments = parse_context_path(path).ok_or_else(|| format!("invalid path `{path}`"))?;
    // Preflight the entire path before extending any arrays or creating objects.
    // This bounds total sparse growth even for nested indices and leaves the
    // original context intact when a write is rejected.
    validate_context_write(root, &segments)?;
    set_path_segments(root, &segments, value)
}

/// Remove the value stored at `path`, returning whether a value was removed.
///
/// A path that does not resolve, or that walks through a container of the wrong shape, is a
/// no-op returning `false`: migration tooling treats "there was nothing to drop" as a fact to
/// report, not as an error.
pub fn remove_context_path(root: &mut Value, path: &str) -> bool {
    let Some(segments) = parse_context_path(path) else {
        return false;
    };
    remove_path_segments(root, &segments, true)
}

pub fn is_valid_context_path(path: &str) -> bool {
    parse_context_path(path).is_some()
}

fn remove_path_segments(
    current: &mut Value,
    segments: &[ContextPathSegment],
    shift_indices: bool,
) -> bool {
    let Some((first, rest)) = segments.split_first() else {
        return false;
    };
    match first {
        ContextPathSegment::Key(key) => {
            let Some(object) = current.as_object_mut() else {
                return false;
            };
            if rest.is_empty() {
                return object.remove(key).is_some();
            }
            match object.get_mut(key) {
                Some(child) => remove_path_segments(child, rest, shift_indices),
                None => false,
            }
        }
        ContextPathSegment::Index(index) => {
            let Some(array) = current.as_array_mut() else {
                return false;
            };
            if rest.is_empty() {
                if *index < array.len() {
                    if shift_indices {
                        array.remove(*index);
                    } else {
                        array[*index] = Value::Null;
                    }
                    return true;
                }
                return false;
            }
            match array.get_mut(*index) {
                Some(child) => remove_path_segments(child, rest, shift_indices),
                None => false,
            }
        }
    }
}

fn normalize_context(value: Value) -> Value {
    match value {
        Value::Null => Value::Object(Map::new()),
        value => value,
    }
}

const MAX_CONTEXT_PATH_BYTES: usize = 2048;
const MAX_CONTEXT_PATH_SEGMENTS: usize = 64;
const MAX_CONTEXT_ARRAY_INDEX: usize = 9_999;
const MAX_CONTEXT_WRITE_NEW_SLOTS: usize = 10_000;

fn validate_context_write(root: &Value, segments: &[ContextPathSegment]) -> Result<(), String> {
    let mut current = Some(root);
    let mut new_slots = 0usize;
    for segment in segments {
        current = current.filter(|value| !value.is_null());
        match segment {
            ContextPathSegment::Key(key) => {
                current = match current {
                    Some(value) => value
                        .as_object()
                        .ok_or_else(|| format!("path segment `{key}` requires an object"))?
                        .get(key),
                    None => None,
                };
            }
            ContextPathSegment::Index(index) => {
                let length = match current {
                    Some(value) => value
                        .as_array()
                        .ok_or_else(|| format!("path index `{index}` requires an array"))?
                        .len(),
                    None => 0,
                };
                new_slots = new_slots.saturating_add((index + 1).saturating_sub(length));
                if new_slots > MAX_CONTEXT_WRITE_NEW_SLOTS {
                    return Err("context write exceeds sparse array growth limit".to_string());
                }
                current = current.and_then(|value| value.as_array()?.get(*index));
            }
        }
    }
    Ok(())
}

fn set_path_segments(
    current: &mut Value,
    segments: &[ContextPathSegment],
    value: Value,
) -> Result<(), String> {
    let Some((first, rest)) = segments.split_first() else {
        *current = value;
        return Ok(());
    };
    match first {
        ContextPathSegment::Key(key) => {
            if current.is_null() {
                *current = Value::Object(Map::new());
            }
            let object = current
                .as_object_mut()
                .ok_or_else(|| format!("path segment `{key}` requires an object"))?;
            if rest.is_empty() {
                object.insert(key.clone(), value);
                Ok(())
            } else {
                let child = object.entry(key.clone()).or_insert(Value::Null);
                set_path_segments(child, rest, value)
            }
        }
        ContextPathSegment::Index(index) => {
            if current.is_null() {
                *current = Value::Array(Vec::new());
            }
            let array = current
                .as_array_mut()
                .ok_or_else(|| format!("path index `{index}` requires an array"))?;
            if array.len() <= *index {
                array.resize(index.saturating_add(1), Value::Null);
            }
            if rest.is_empty() {
                array[*index] = value;
                Ok(())
            } else {
                set_path_segments(&mut array[*index], rest, value)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ContextPathSegment {
    Key(String),
    Index(usize),
}

/// Report field paths where one is a strict ancestor of another.
///
/// Comparison is by *segment*, never by string prefix: `user` and `username` share a textual
/// prefix but describe unrelated properties, and flagging them would be worse than the bug.
fn conflicting_field_paths(catalog: &ContextSchemaCatalog) -> Vec<ValidationDiagnostic> {
    // Segments are compared as `Vec<String>` tokens rather than a joined string, so a key that
    // happens to contain the delimiter cannot forge or hide a relationship.
    let mut declared = BTreeMap::<Vec<String>, String>::new();
    let mut parsed = Vec::new();
    for field in &catalog.fields {
        let Some(segments) = parse_context_path(&field.path) else {
            continue;
        };
        let tokens = segments
            .iter()
            .map(|segment| match segment {
                ContextPathSegment::Key(key) => format!("k{key}"),
                ContextPathSegment::Index(index) => format!("i{index}"),
            })
            .collect::<Vec<_>>();
        declared
            .entry(tokens.clone())
            .or_insert_with(|| field.path.clone());
        parsed.push((field, tokens));
    }

    let mut diagnostics = Vec::new();
    for (field, tokens) in parsed {
        for depth in 1..tokens.len() {
            let Some(ancestor) = declared.get(&tokens[..depth]) else {
                continue;
            };
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_context_field_path_conflict",
                &field.path,
                format!(
                    "field `{}` path `{}` is nested under field path `{}`, which is declared as a \
                     value; the generated schema would silently drop one of them",
                    field.id, field.path, ancestor
                ),
            ));
        }
    }
    diagnostics
}

pub(crate) fn context_paths_equal(left: &str, right: &str) -> bool {
    matches!(
        (parse_context_path(left), parse_context_path(right)),
        (Some(left), Some(right)) if left == right
    )
}

pub(crate) fn context_path_is_prefix(prefix: &str, path: &str) -> bool {
    let (Some(prefix), Some(path)) = (parse_context_path(prefix), parse_context_path(path)) else {
        return false;
    };
    path.starts_with(&prefix)
}

pub(crate) fn context_paths_overlap(left: &str, right: &str) -> bool {
    let (Some(left), Some(right)) = (parse_context_path(left), parse_context_path(right)) else {
        return false;
    };
    left.starts_with(&right) || right.starts_with(&left)
}

fn parse_context_path(path: &str) -> Option<Vec<ContextPathSegment>> {
    if path.len() > MAX_CONTEXT_PATH_BYTES {
        return None;
    }
    let path = path.trim();
    let path = path.strip_prefix('$').unwrap_or(path);
    let path = path.strip_prefix('.').unwrap_or(path);
    if path.is_empty() {
        return None;
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
                segments.push(ContextPathSegment::Key(std::mem::take(&mut token)));
                if matches!(
                    chars.peek().copied(),
                    None | Some('.') | Some('[') | Some(']')
                ) {
                    return None;
                }
            }
            '[' => {
                if !token.is_empty() {
                    segments.push(ContextPathSegment::Key(std::mem::take(&mut token)));
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
                if !closed || index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                let index = index.parse::<usize>().ok()?;
                if index > MAX_CONTEXT_ARRAY_INDEX {
                    return None;
                }
                segments.push(ContextPathSegment::Index(index));
                match chars.peek().copied() {
                    Some('.') => {
                        chars.next();
                        if matches!(
                            chars.peek().copied(),
                            None | Some('.') | Some('[') | Some(']')
                        ) {
                            return None;
                        }
                    }
                    Some('[') | None => {}
                    Some(_) => return None,
                }
            }
            ']' | '{' | '}' => return None,
            character if character.is_whitespace() || character.is_control() => return None,
            _ => token.push(character),
        }
    }
    if !token.is_empty() {
        segments.push(ContextPathSegment::Key(token));
    }
    (!segments.is_empty() && segments.len() <= MAX_CONTEXT_PATH_SEGMENTS).then_some(segments)
}

fn evaluate_expression(expression: &ContextExpression, context: &Value) -> ExpressionEvaluation {
    match expression {
        ContextExpression::Literal { value } => ExpressionEvaluation::Value(value.clone()),
        ContextExpression::Path { path } => resolve_context_path(context, path)
            .cloned()
            .map(ExpressionEvaluation::Value)
            .unwrap_or(ExpressionEvaluation::Pending),
        ContextExpression::Coalesce { values } => {
            let mut saw_pending = false;
            for value in values {
                match evaluate_expression(value, context) {
                    ExpressionEvaluation::Value(value) if !value.is_null() => {
                        return ExpressionEvaluation::Value(value);
                    }
                    ExpressionEvaluation::Value(_) => {}
                    ExpressionEvaluation::Pending => saw_pending = true,
                    ExpressionEvaluation::Error(_) => {}
                }
            }
            if saw_pending {
                ExpressionEvaluation::Pending
            } else {
                ExpressionEvaluation::Value(Value::Null)
            }
        }
        ContextExpression::Concat { values, separator } => {
            let mut rendered = Vec::new();
            for value in values {
                match evaluate_expression(value, context) {
                    ExpressionEvaluation::Value(value) => rendered.push(scalar_text(&value)),
                    ExpressionEvaluation::Pending => return ExpressionEvaluation::Pending,
                    ExpressionEvaluation::Error(error) => {
                        return ExpressionEvaluation::Error(error);
                    }
                }
            }
            ExpressionEvaluation::Value(Value::String(rendered.join(separator)))
        }
        ContextExpression::Add { left, right } => {
            evaluate_numeric_binary(left, right, context, |left, right| left + right, "add")
        }
        ContextExpression::Subtract { left, right } => {
            evaluate_numeric_binary(left, right, context, |left, right| left - right, "subtract")
        }
        ContextExpression::Multiply { left, right } => {
            evaluate_numeric_binary(left, right, context, |left, right| left * right, "multiply")
        }
        ContextExpression::Divide { left, right } => {
            let right_value = match evaluate_expression(right, context) {
                ExpressionEvaluation::Value(value) => value,
                state => return state,
            };
            let Some(divisor) = numeric_value(&right_value) else {
                return ExpressionEvaluation::Error(
                    "divide right operand is not numeric".to_string(),
                );
            };
            if divisor == 0.0 {
                return ExpressionEvaluation::Error("division by zero".to_string());
            }
            let left_value = match evaluate_expression(left, context) {
                ExpressionEvaluation::Value(value) => value,
                state => return state,
            };
            let Some(dividend) = numeric_value(&left_value) else {
                return ExpressionEvaluation::Error(
                    "divide left operand is not numeric".to_string(),
                );
            };
            number_value(dividend / divisor)
        }
        ContextExpression::Equals { left, right } => evaluate_equality(left, right, context, false),
        ContextExpression::NotEquals { left, right } => {
            evaluate_equality(left, right, context, true)
        }
        ContextExpression::GreaterThan { left, right } => evaluate_ordering(
            left,
            right,
            context,
            |ordering| ordering.is_gt(),
            "greater_than",
        ),
        ContextExpression::LessThan { left, right } => evaluate_ordering(
            left,
            right,
            context,
            |ordering| ordering.is_lt(),
            "less_than",
        ),
        ContextExpression::And { values } => {
            for value in values {
                match evaluate_expression(value, context) {
                    ExpressionEvaluation::Value(value) if !is_truthy(&value) => {
                        return ExpressionEvaluation::Value(Value::Bool(false));
                    }
                    ExpressionEvaluation::Value(_) => {}
                    ExpressionEvaluation::Pending => return ExpressionEvaluation::Pending,
                    ExpressionEvaluation::Error(error) => {
                        return ExpressionEvaluation::Error(error);
                    }
                }
            }
            ExpressionEvaluation::Value(Value::Bool(true))
        }
        ContextExpression::Or { values } => {
            let mut saw_pending = false;
            for value in values {
                match evaluate_expression(value, context) {
                    ExpressionEvaluation::Value(value) if is_truthy(&value) => {
                        return ExpressionEvaluation::Value(Value::Bool(true));
                    }
                    ExpressionEvaluation::Value(_) => {}
                    ExpressionEvaluation::Pending => saw_pending = true,
                    ExpressionEvaluation::Error(error) => {
                        return ExpressionEvaluation::Error(error);
                    }
                }
            }
            if saw_pending {
                ExpressionEvaluation::Pending
            } else {
                ExpressionEvaluation::Value(Value::Bool(false))
            }
        }
        ContextExpression::Not { value } => match evaluate_expression(value, context) {
            ExpressionEvaluation::Value(value) => {
                ExpressionEvaluation::Value(Value::Bool(!is_truthy(&value)))
            }
            state => state,
        },
        ContextExpression::If {
            condition,
            then_value,
            else_value,
        } => match evaluate_expression(condition, context) {
            ExpressionEvaluation::Value(value) if is_truthy(&value) => {
                evaluate_expression(then_value, context)
            }
            ExpressionEvaluation::Value(_) => evaluate_expression(else_value, context),
            state => state,
        },
        ContextExpression::Format { template } => format_expression(template, context),
    }
}

fn evaluate_numeric_binary(
    left: &ContextExpression,
    right: &ContextExpression,
    context: &Value,
    operation: impl FnOnce(f64, f64) -> f64,
    name: &str,
) -> ExpressionEvaluation {
    let left = match evaluate_expression(left, context) {
        ExpressionEvaluation::Value(value) => value,
        state => return state,
    };
    let right = match evaluate_expression(right, context) {
        ExpressionEvaluation::Value(value) => value,
        state => return state,
    };
    let Some(left) = numeric_value(&left) else {
        return ExpressionEvaluation::Error(format!("{name} left operand is not numeric"));
    };
    let Some(right) = numeric_value(&right) else {
        return ExpressionEvaluation::Error(format!("{name} right operand is not numeric"));
    };
    number_value(operation(left, right))
}

fn evaluate_equality(
    left: &ContextExpression,
    right: &ContextExpression,
    context: &Value,
    invert: bool,
) -> ExpressionEvaluation {
    let left = match evaluate_expression(left, context) {
        ExpressionEvaluation::Value(value) => value,
        state => return state,
    };
    let right = match evaluate_expression(right, context) {
        ExpressionEvaluation::Value(value) => value,
        state => return state,
    };
    ExpressionEvaluation::Value(Value::Bool((left == right) ^ invert))
}

fn evaluate_ordering(
    left: &ContextExpression,
    right: &ContextExpression,
    context: &Value,
    predicate: impl FnOnce(std::cmp::Ordering) -> bool,
    name: &str,
) -> ExpressionEvaluation {
    let left = match evaluate_expression(left, context) {
        ExpressionEvaluation::Value(value) => value,
        state => return state,
    };
    let right = match evaluate_expression(right, context) {
        ExpressionEvaluation::Value(value) => value,
        state => return state,
    };
    let ordering = match (numeric_value(&left), numeric_value(&right)) {
        (Some(left), Some(right)) => left.partial_cmp(&right),
        _ => match (left.as_str(), right.as_str()) {
            (Some(left), Some(right)) => Some(left.cmp(right)),
            _ => None,
        },
    };
    ordering
        .map(|ordering| ExpressionEvaluation::Value(Value::Bool(predicate(ordering))))
        .unwrap_or_else(|| {
            ExpressionEvaluation::Error(format!(
                "{name} operands must both be numbers or both be strings"
            ))
        })
}

fn format_expression(template: &str, context: &Value) -> ExpressionEvaluation {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 2..];
        let Some(end) = after.find("}}") else {
            return ExpressionEvaluation::Error(
                "format template contains an unclosed placeholder".to_string(),
            );
        };
        let path = after[..end].trim();
        if path.is_empty() {
            return ExpressionEvaluation::Error(
                "format template contains an empty placeholder".to_string(),
            );
        }
        let Some(value) = resolve_context_path(context, path) else {
            return ExpressionEvaluation::Pending;
        };
        output.push_str(&scalar_text(value));
        remaining = &after[end + 2..];
    }
    output.push_str(remaining);
    ExpressionEvaluation::Value(Value::String(output))
}

#[derive(Debug, Clone, PartialEq)]
enum ExpressionEvaluation {
    Value(Value),
    Pending,
    Error(String),
}

fn number_value(value: f64) -> ExpressionEvaluation {
    Number::from_f64(value)
        .map(Value::Number)
        .map(ExpressionEvaluation::Value)
        .unwrap_or_else(|| ExpressionEvaluation::Error("numeric result is not finite".to_string()))
}

fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse().ok(),
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
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

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn validate_field_identity(field: &ContextFieldDefinition) -> FlyResult<()> {
    if field.id.trim().is_empty() {
        return Err(FlyError::Decode(
            "runtime context field id must not be empty".to_string(),
        ));
    }
    if parse_context_path(&field.path).is_none() {
        return Err(FlyError::Decode(format!(
            "runtime context field path `{}` is invalid",
            field.path
        )));
    }
    if field.item_kind.is_some() && field.kind != ContextValueKind::Array {
        return Err(FlyError::Decode(
            "runtime context item_kind requires array kind".to_string(),
        ));
    }
    if let Some(default) = field.default.as_ref() {
        if !field.kind.accepts(default) {
            return Err(FlyError::Decode(format!(
                "runtime context field default must be {}",
                field.kind.as_str()
            )));
        }
        if let (ContextValueKind::Array, Some(item_kind), Value::Array(items)) =
            (field.kind, field.item_kind, default)
            && items.iter().any(|item| !item_kind.accepts(item))
        {
            return Err(FlyError::Decode(format!(
                "runtime context field default contains items that are not {}",
                item_kind.as_str()
            )));
        }
    }
    Ok(())
}

fn validate_computed_identity(computed: &ComputedContextValue) -> FlyResult<()> {
    if computed.id.trim().is_empty() {
        return Err(FlyError::Decode(
            "computed context value id must not be empty".to_string(),
        ));
    }
    if parse_context_path(&computed.path).is_none() {
        return Err(FlyError::Decode(format!(
            "computed context value path `{}` is invalid",
            computed.path
        )));
    }
    let mut paths = BTreeSet::new();
    collect_expression_paths(&computed.expression, &mut paths);
    if paths.iter().any(|path| {
        parse_context_path(path)
            .is_some_and(|path| Some(path) == parse_context_path(&computed.path))
    }) {
        return Err(FlyError::Decode(
            "computed context value cannot reference its own target path".to_string(),
        ));
    }
    let mut diagnostics = Vec::new();
    validate_expression_shape(
        &computed.expression,
        &computed.path,
        &computed.id,
        &mut diagnostics,
    );
    if let Some(error) = diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    {
        return Err(FlyError::Decode(error.message));
    }
    Ok(())
}

fn validate_definition_id(
    kind: &str,
    id: &str,
    path: &str,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if id.trim().is_empty() {
        diagnostics.push(context_diagnostic(
            ValidationSeverity::Error,
            "runtime_context_definition_id_empty",
            path,
            format!("{kind} id must not be empty"),
        ));
    } else if !ids.insert(id.to_string()) {
        diagnostics.push(context_diagnostic(
            ValidationSeverity::Error,
            "duplicate_runtime_context_definition_id",
            path,
            format!("runtime context definition id `{id}` is duplicated"),
        ));
    }
}

fn validate_expression_shape(
    expression: &ContextExpression,
    path: &str,
    id: &str,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    match expression {
        ContextExpression::Coalesce { values }
        | ContextExpression::Concat { values, .. }
        | ContextExpression::And { values }
        | ContextExpression::Or { values }
            if values.is_empty() =>
        {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_computed_expression_empty",
                path,
                format!("computed value `{id}` expression list must not be empty"),
            ));
        }
        ContextExpression::Path { path: dependency }
            if parse_context_path(dependency).is_none() =>
        {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_computed_dependency_path_invalid",
                path,
                format!("computed value `{id}` dependency `{dependency}` is invalid"),
            ));
        }
        ContextExpression::Format { template } => {
            let mut remaining = template.as_str();
            while let Some(start) = remaining.find("{{") {
                let after = &remaining[start + 2..];
                let Some(end) = after.find("}}") else {
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Error,
                        "runtime_computed_format_unclosed",
                        path,
                        format!("computed value `{id}` has an unclosed format placeholder"),
                    ));
                    break;
                };
                let dependency = after[..end].trim();
                if dependency.is_empty() {
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Error,
                        "runtime_computed_format_empty",
                        path,
                        format!("computed value `{id}` has an empty format placeholder"),
                    ));
                } else if parse_context_path(dependency).is_none() {
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Error,
                        "runtime_computed_dependency_path_invalid",
                        path,
                        format!("computed value `{id}` dependency `{dependency}` is invalid"),
                    ));
                }
                remaining = &after[end + 2..];
            }
        }
        _ => {}
    }

    match expression {
        ContextExpression::Coalesce { values }
        | ContextExpression::Concat { values, .. }
        | ContextExpression::And { values }
        | ContextExpression::Or { values } => {
            for value in values {
                validate_expression_shape(value, path, id, diagnostics);
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
            validate_expression_shape(left, path, id, diagnostics);
            validate_expression_shape(right, path, id, diagnostics);
        }
        ContextExpression::Not { value } => validate_expression_shape(value, path, id, diagnostics),
        ContextExpression::If {
            condition,
            then_value,
            else_value,
        } => {
            validate_expression_shape(condition, path, id, diagnostics);
            validate_expression_shape(then_value, path, id, diagnostics);
            validate_expression_shape(else_value, path, id, diagnostics);
        }
        ContextExpression::Literal { .. }
        | ContextExpression::Path { .. }
        | ContextExpression::Format { .. } => {}
    }
}

fn collect_expression_paths(expression: &ContextExpression, paths: &mut BTreeSet<String>) {
    match expression {
        ContextExpression::Path { path } => {
            paths.insert(path.clone());
        }
        ContextExpression::Format { template } => paths.extend(format_dependencies(template)),
        ContextExpression::Coalesce { values }
        | ContextExpression::Concat { values, .. }
        | ContextExpression::And { values }
        | ContextExpression::Or { values } => {
            for value in values {
                collect_expression_paths(value, paths);
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
            collect_expression_paths(left, paths);
            collect_expression_paths(right, paths);
        }
        ContextExpression::Not { value } => collect_expression_paths(value, paths),
        ContextExpression::If {
            condition,
            then_value,
            else_value,
        } => {
            collect_expression_paths(condition, paths);
            collect_expression_paths(then_value, paths);
            collect_expression_paths(else_value, paths);
        }
        ContextExpression::Literal { .. } => {}
    }
}

fn format_dependencies(template: &str) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut remaining = template;
    while let Some(start) = remaining.find("{{") {
        let after = &remaining[start + 2..];
        let Some(end) = after.find("}}") else {
            break;
        };
        let path = after[..end].trim();
        if !path.is_empty() {
            dependencies.push(path.to_string());
        }
        remaining = &after[end + 2..];
    }
    dependencies
}

fn detect_computed_cycles(computed: &[ComputedContextValue]) -> Vec<ValidationDiagnostic> {
    computed_evaluation_order(computed).1
}

/// One structural dependency graph for validation and evaluation. A dependency
/// on an object also depends on its computed descendants; a dependency on a
/// child depends on a computed ancestor. Overlapping writes are ambiguous.
/// Kahn's algorithm avoids recursion proportional to the catalog length.
pub(crate) fn computed_evaluation_order(
    computed: &[ComputedContextValue],
) -> (Vec<usize>, Vec<ValidationDiagnostic>) {
    let mut targets = BTreeMap::<Vec<ContextPathSegment>, usize>::new();
    let mut diagnostics = Vec::new();
    for (index, value) in computed.iter().enumerate() {
        let Some(path) = parse_context_path(&value.path) else {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_computed_path_invalid",
                &value.path,
                format!("computed value `{}` path is invalid", value.id),
            ));
            continue;
        };
        if targets.insert(path, index).is_some() {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "duplicate_runtime_computed_path",
                &value.path,
                format!("computed value `{}` has a duplicate target", value.id),
            ));
        }
    }
    for (path, &index) in &targets {
        if (1..path.len()).any(|length| targets.contains_key(&path[..length])) {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_computed_target_overlap",
                &computed[index].path,
                "computed targets must not overlap as ancestor and descendant",
            ));
        }
    }
    if !diagnostics.is_empty() {
        return (Vec::new(), diagnostics);
    }
    let mut indegree = vec![0usize; computed.len()];
    let mut dependents = vec![Vec::<usize>::new(); computed.len()];
    // Cap graph edges as well as path depth; aggregate reads can otherwise
    // create a dense dependency graph from a compact catalog.
    const MAX_DEPENDENCY_EDGES: usize = 100_000;
    let mut edges = 0usize;
    for (index, value) in computed.iter().enumerate() {
        let mut paths = BTreeSet::new();
        collect_expression_paths(&value.expression, &mut paths);
        let mut dependencies = BTreeSet::new();
        for path in paths.iter().filter_map(|path| parse_context_path(path)) {
            for length in 1..=path.len() {
                if let Some(&producer) = targets.get(&path[..length]) {
                    dependencies.insert(producer);
                }
            }
            for (target, &producer) in targets.range(path.clone()..) {
                if !target.starts_with(&path) {
                    break;
                }
                dependencies.insert(producer);
            }
            if edges.saturating_add(dependencies.len()) > MAX_DEPENDENCY_EDGES {
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_computed_dependency_budget_exceeded",
                    &value.path,
                    "computed dependency graph exceeds its edge budget",
                ));
                return (Vec::new(), diagnostics);
            }
        }
        indegree[index] = dependencies.len();
        edges += dependencies.len();
        for producer in dependencies {
            dependents[producer].push(index);
        }
    }
    let mut ready = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, &count)| (count == 0).then_some(index))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(computed.len());
    while let Some(index) = ready.pop_first() {
        order.push(index);
        for &consumer in &dependents[index] {
            indegree[consumer] -= 1;
            if indegree[consumer] == 0 {
                ready.insert(consumer);
            }
        }
    }
    for (index, count) in indegree.into_iter().enumerate() {
        if count != 0 {
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Error,
                "runtime_computed_dependency_cycle",
                &computed[index].path,
                format!(
                    "computed value `{}` is in or depends on a dependency cycle",
                    computed[index].id
                ),
            ));
        }
    }
    (order, diagnostics)
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

fn write_catalog(document: &mut ProjectDocument, catalog: ContextSchemaCatalog) -> FlyResult<()> {
    write_entries(
        document,
        FLY_RUNTIME_CONTEXT_SCHEMA_FIELD,
        catalog.fields,
        catalog.unknown_field_entries,
    )?;
    write_entries(
        document,
        FLY_RUNTIME_COMPUTED_FIELD,
        catalog.computed,
        catalog.unknown_computed_entries,
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
        document.project_mut().extensions.remove(field);
    } else {
        document
            .project_mut()
            .extensions
            .insert(field.to_string(), Value::Array(entries));
    }
    document.invalidate_caches();
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

fn remove_by_id<T>(
    values: &mut Vec<T>,
    target: &str,
    id: impl Fn(&T) -> &str,
    kind: &str,
) -> FlyResult<()> {
    let before = values.len();
    values.retain(|candidate| id(candidate) != target);
    if values.len() == before {
        return Err(FlyError::Decode(format!("{kind} `{target}` was not found")));
    }
    Ok(())
}

fn context_diagnostic(
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
            "pages": [{
                "component": { "id": "root", "type": "wrapper" }
            }],
            "flyRuntimeContextSchema": [{
                "id": "currency",
                "path": "shop.currency",
                "kind": "string",
                "required": true,
                "default": "EUR"
            }, {
                "id": "items",
                "path": "cart.items",
                "kind": "array",
                "item_kind": "object",
                "default": []
            }],
            "flyRuntimeComputed": [{
                "id": "subtotal",
                "path": "cart.subtotal",
                "expression": {
                    "op": "multiply",
                    "left": { "op": "path", "path": "cart.quantity" },
                    "right": { "op": "path", "path": "cart.unitPrice" }
                }
            }, {
                "id": "label",
                "path": "cart.label",
                "expression": {
                    "op": "format",
                    "template": "{{shop.currency}} {{cart.subtotal}}"
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn applies_defaults_and_resolves_forward_computed_dependencies() {
        let materialized = materialize_context(
            &document(),
            &json!({ "cart": { "quantity": 3, "unitPrice": 4.5 } }),
        );
        assert_eq!(
            resolve_context_path(&materialized.context, "shop.currency"),
            Some(&json!("EUR"))
        );
        assert_eq!(
            resolve_context_path(&materialized.context, "cart.subtotal"),
            Some(&json!(13.5))
        );
        assert_eq!(
            resolve_context_path(&materialized.context, "cart.label"),
            Some(&json!("EUR 13.5"))
        );
        assert_eq!(materialized.defaults_applied, 2);
        assert_eq!(materialized.computed_applied, 2);
        assert_eq!(materialized.unresolved_computed, 0);
    }

    #[test]
    fn computed_dependencies_use_fresh_values_and_propagate_fallbacks_in_either_order() {
        let definitions = vec![
            json!({"id": "label", "path": "label", "expression": {"op": "format", "template": "{{$.product.name}}!"}}),
            json!({"id": "product", "path": "product", "expression": {"op": "path", "path": "missing"}, "fallback": {"name": "Fresh"}}),
            json!({"id": "aggregate", "path": "aggregate", "expression": {"op": "path", "path": "cart"}}),
            json!({"id": "amount", "path": "cart.amount", "expression": {"op": "literal", "value": 7}}),
        ];
        for values in [definitions.clone(), definitions.into_iter().rev().collect()] {
            let document = GrapesJsCodec::decode_value(json!({
                "pages": [{"component": {"id": "root", "type": "wrapper"}}],
                "flyRuntimeComputed": values
            }))
            .expect("document");
            let result = materialize_context(
                &document,
                &json!({
                    "product": {"name": "Stale"}, "cart": {"amount": 0}
                }),
            );
            assert_eq!(result.context["label"], "Fresh!");
            assert_eq!(result.context["aggregate"]["amount"], 7);
            assert_eq!(result.computed_applied, 4);
            assert_eq!(result.computed_fallbacks, 1);
            assert_eq!(result.unresolved_computed, 0);
        }
    }

    #[test]
    fn failed_computed_producers_do_not_expose_stale_input_or_shift_array_slots() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper"}}],
            "flyRuntimeComputed": [
                {"id": "consumer", "path": "label", "expression": {"op": "path", "path": "total"}, "fallback": "Unavailable"},
                {"id": "producer", "path": "total", "expression": {"op": "path", "path": "missing"}},
                {"id": "array-producer", "path": "items[0]", "expression": {"op": "path", "path": "missing"}}
            ]
        })).expect("document");
        let input = json!({"total": 999, "label": "Stale", "items": [123, "keep"]});
        let result = materialize_context(&document, &input);
        assert!(result.context.get("total").is_none());
        assert_eq!(result.context["label"], "Unavailable");
        assert_eq!(result.context["items"], json!([null, "keep"]));
        assert_eq!(result.unresolved_computed, 2);
        assert_eq!(result.computed_fallbacks, 1);
        assert_eq!(input["total"], 999);
    }

    #[test]
    fn structural_cycles_block_fallback_and_downstream_evaluation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper"}}],
            "flyRuntimeComputed": [
                {"id": "a", "path": "a", "expression": {"op": "path", "path": "$.b"}, "fallback": 1},
                {"id": "b", "path": "b", "expression": {"op": "path", "path": "$.a"}, "fallback": 2},
                {"id": "c", "path": "c", "expression": {"op": "path", "path": "a"}}
            ]
        })).expect("document");
        let result = materialize_context(&document, &json!({}));
        assert_eq!(result.computed_applied, 0);
        assert_eq!(result.unresolved_computed, 3);
        assert_eq!(result.computed_fallbacks, 0);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "runtime_computed_dependency_cycle")
        );
        assert!(
            validate_context_definitions(&document)
                .iter()
                .any(|d| d.code == "runtime_computed_dependency_cycle")
        );
    }

    #[test]
    fn overlapping_computed_targets_are_rejected_and_failed_writes_are_counted() {
        let source = |values| {
            GrapesJsCodec::decode_value(json!({
                "pages": [{"component": {"id": "root", "type": "wrapper"}}],
                "flyRuntimeComputed": values
            }))
            .expect("document")
        };
        let overlap = source(json!([
            {"id": "one", "path": "obj", "expression": {"op": "literal", "value": {}}},
            {"id": "two", "path": "obj.child", "expression": {"op": "literal", "value": 1}}
        ]));
        assert!(
            validate_context_definitions(&overlap)
                .iter()
                .any(|d| d.code == "runtime_computed_target_overlap")
        );
        let bad_write = source(json!([
            {"id": "one", "path": "obj.child", "expression": {"op": "literal", "value": 1}}
        ]));
        let result = materialize_context(&bad_write, &json!({"obj": 2}));
        assert_eq!(result.unresolved_computed, 1);
        assert_eq!(result.computed_applied, 0);
        assert_eq!(result.context["obj"], 2);
    }

    #[test]
    fn declared_fields_cannot_overlap_computed_targets() {
        let make_document = |field_path: &str, computed_path: &str| {
            GrapesJsCodec::decode_value(json!({
                "pages": [{"component": {"id": "root", "type": "wrapper"}}],
                "flyRuntimeContextSchema": [{
                    "id": "input",
                    "path": field_path,
                    "kind": "object"
                }],
                "flyRuntimeComputed": [{
                    "id": "producer",
                    "path": computed_path,
                    "expression": {"op": "literal", "value": {}}
                }]
            }))
            .expect("document")
        };

        for (field_path, computed_path, expected_code) in [
            (
                "user.name",
                "user",
                "runtime_context_field_computed_target_overlap",
            ),
            (
                "user",
                "user.name",
                "runtime_context_field_computed_target_overlap",
            ),
            (
                "$.user.name",
                "user.name",
                "runtime_context_path_shadowed_by_computed",
            ),
            (
                "items[0]",
                "items[00]",
                "runtime_context_path_shadowed_by_computed",
            ),
        ] {
            let document = make_document(field_path, computed_path);
            assert!(
                validate_context_definitions(&document)
                    .iter()
                    .any(|diagnostic| diagnostic.code == expected_code),
                "expected {expected_code} for field {} and computed {}",
                field_path,
                computed_path
            );
            assert!(!crate::extract_runtime_context_contract(&document).is_valid());
        }

        let disjoint = make_document("user", "username");
        assert!(
            !validate_context_definitions(&disjoint).iter().any(
                |diagnostic| diagnostic.code == "runtime_context_field_computed_target_overlap"
            )
        );
    }

    #[test]
    fn validation_detects_dependency_cycles_and_default_type_errors() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": { "id": "root", "type": "wrapper" }
            }],
            "flyRuntimeContextSchema": [{
                "id": "count",
                "path": "count",
                "kind": "number",
                "default": "wrong"
            }],
            "flyRuntimeComputed": [{
                "id": "a",
                "path": "a",
                "expression": { "op": "path", "path": "b" }
            }, {
                "id": "b",
                "path": "b",
                "expression": { "op": "path", "path": "a" }
            }]
        }))
        .expect("document");
        let diagnostics = validate_context_definitions(&document);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_context_default_type_mismatch")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_computed_dependency_cycle")
        );
    }

    #[test]
    fn format_validation_rejects_later_unclosed_and_empty_placeholders() {
        let document_with_template = |template: &str| {
            GrapesJsCodec::decode_value(json!({
                "pages": [{"component": {"id": "root", "type": "wrapper"}}],
                "flyRuntimeComputed": [{
                    "id": "label",
                    "path": "label",
                    "expression": { "op": "format", "template": template }
                }]
            }))
            .expect("document")
        };

        let unclosed = document_with_template("{{name}} {{broken");
        assert!(
            validate_context_definitions(&unclosed)
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_computed_format_unclosed")
        );

        let empty = document_with_template("{{name}} {{}}");
        assert!(
            validate_context_definitions(&empty)
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_computed_format_empty")
        );
    }

    #[test]
    fn commands_preserve_opaque_entries() {
        let mut document = document();
        document.project_mut().extensions.insert(
            FLY_RUNTIME_CONTEXT_SCHEMA_FIELD.to_string(),
            serde_json::json!([{ "providerSchema": true }]),
        );
        let before_hash = document.hash();
        apply_context_command(
            &mut document,
            &ContextCommand::UpsertField {
                field: ContextFieldDefinition {
                    id: "name".to_string(),
                    path: "user.name".to_string(),
                    kind: ContextValueKind::String,
                    required: false,
                    default: None,
                    item_kind: None,
                    extensions: Map::new(),
                },
            },
        )
        .expect("upsert field");
        assert_ne!(document.hash(), before_hash);
        let entries = document.project().extensions[FLY_RUNTIME_CONTEXT_SCHEMA_FIELD]
            .as_array()
            .expect("schema entries");
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|entry| entry.get("providerSchema").is_some())
        );
    }

    #[test]
    fn context_path_limits_reject_sparse_growth_and_invalid_grammar_atomically() {
        let mut root = json!({"existing": 1});
        let before = root.clone();
        for path in [
            "items[10000]",
            "items[18446744073709551615]",
            "items.[0]",
            "items[+1]",
            "items[9999][9999]",
            "existing.child",
        ] {
            assert!(
                set_context_path(&mut root, path, json!(1)).is_err(),
                "{path}"
            );
            assert_eq!(root, before, "{path}");
        }
        assert!(!is_valid_context_path(&vec!["x"; 65].join(".")));
        assert!(!is_valid_context_path(&"x".repeat(2049)));
        set_context_path(&mut root, "items[2].name", json!("ok")).expect("bounded write");
        assert_eq!(
            resolve_context_path(&root, "$.items[02].name"),
            Some(&json!("ok"))
        );
    }

    #[test]
    fn equivalent_context_field_and_computed_paths_are_duplicate_targets() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper"}}],
            "flyRuntimeContextSchema": [
                {"id": "first", "path": "page.title", "kind": "string"},
                {"id": "second", "path": "$.page.title", "kind": "string"}
            ],
            "flyRuntimeComputed": [
                {"id": "one", "path": "items[0]", "expression": {"op": "literal", "value": 1}},
                {"id": "two", "path": "$.items[00]", "expression": {"op": "path", "path": "items[0]"}}
            ]
        })).expect("document");
        let diagnostics = validate_context_definitions(&document);
        for code in [
            "duplicate_runtime_context_field_path",
            "duplicate_runtime_computed_path",
            "runtime_computed_self_reference",
        ] {
            assert!(diagnostics.iter().any(|d| d.code == code), "{code}");
        }
    }

    #[test]
    fn invalid_context_paths_are_rejected_before_materialization() {
        let mut document = document();
        for path in [
            "",
            "items[0",
            "items[0]title",
            "items[0].[1]",
            "items.",
            "items. title",
        ] {
            let result = apply_context_command(
                &mut document,
                &ContextCommand::UpsertField {
                    field: ContextFieldDefinition {
                        id: format!("field-{path}"),
                        path: path.to_string(),
                        kind: ContextValueKind::String,
                        required: false,
                        default: None,
                        item_kind: None,
                        extensions: Map::new(),
                    },
                },
            );
            assert!(result.is_err(), "path {path:?} should be invalid");
        }
    }

    fn document_with_fields(fields: Value) -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": fields
        }))
        .expect("document")
    }

    fn conflict_codes(fields: Value) -> Vec<String> {
        validate_context_definitions(&document_with_fields(fields))
            .into_iter()
            .filter(|diagnostic| diagnostic.code == "runtime_context_field_path_conflict")
            .map(|diagnostic| diagnostic.path)
            .collect()
    }

    #[test]
    fn a_field_nested_under_a_value_field_is_rejected() {
        // Only exact duplicates were caught, so this validated clean and then produced a schema
        // that silently lost one of the two fields depending on declaration order.
        let conflicts = conflict_codes(json!([
            { "id": "user", "path": "user", "kind": "string" },
            { "id": "user-name", "path": "user.name", "kind": "string" }
        ]));
        assert_eq!(conflicts, vec!["user.name"]);
    }

    #[test]
    fn declaration_order_does_not_change_the_verdict() {
        // The corruption was order-dependent; the diagnostic must not be.
        let conflicts = conflict_codes(json!([
            { "id": "user-name", "path": "user.name", "kind": "string" },
            { "id": "user", "path": "user", "kind": "string" }
        ]));
        assert_eq!(conflicts, vec!["user.name"]);
    }

    #[test]
    fn a_shared_textual_prefix_is_not_a_conflict() {
        // `user` and `username` describe unrelated properties. Comparing strings rather than
        // segments here would be worse than the bug being fixed.
        assert!(
            conflict_codes(json!([
                { "id": "user", "path": "user", "kind": "string" },
                { "id": "username", "path": "username", "kind": "string" },
                { "id": "users", "path": "users", "kind": "array", "item_kind": "string" }
            ]))
            .is_empty()
        );
    }

    #[test]
    fn sibling_and_deeper_paths_without_a_declared_ancestor_are_fine() {
        assert!(
            conflict_codes(json!([
                { "id": "a", "path": "shop.currency", "kind": "string" },
                { "id": "b", "path": "shop.locale", "kind": "string" },
                { "id": "c", "path": "shop.address.city", "kind": "string" }
            ]))
            .is_empty()
        );
    }

    #[test]
    fn a_conflict_is_reported_for_every_declared_ancestor() {
        let conflicts = conflict_codes(json!([
            { "id": "a", "path": "a", "kind": "string" },
            { "id": "ab", "path": "a.b", "kind": "string" },
            { "id": "abc", "path": "a.b.c", "kind": "string" }
        ]));
        // `a.b` is nested under `a`; `a.b.c` is nested under both `a` and `a.b`.
        assert_eq!(conflicts, vec!["a.b", "a.b.c", "a.b.c"]);
    }
}
