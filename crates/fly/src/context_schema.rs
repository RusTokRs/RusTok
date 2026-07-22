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
                .project
                .extensions
                .get(FLY_RUNTIME_CONTEXT_SCHEMA_FIELD),
        );
        let (computed, unknown_computed_entries) =
            decode_entries(document.project.extensions.get(FLY_RUNTIME_COMPUTED_FIELD));
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
        if resolve_context_path(&context, &field.path).is_none() {
            if let Some(default) = field.default.clone() {
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
    }

    let mut pending = catalog.computed.iter().collect::<Vec<_>>();
    let mut computed_applied = 0usize;
    let mut computed_fallbacks = 0usize;
    let mut unresolved_computed = 0usize;
    let maximum_passes = pending.len().saturating_add(1);

    for _ in 0..maximum_passes {
        if pending.is_empty() {
            break;
        }
        let mut progress = false;
        let mut next = Vec::new();
        for computed in pending {
            match evaluate_expression(&computed.expression, &context) {
                ExpressionEvaluation::Value(value) => {
                    match set_context_path(&mut context, &computed.path, value) {
                        Ok(()) => {
                            computed_applied = computed_applied.saturating_add(1);
                            progress = true;
                        }
                        Err(error) => diagnostics.push(context_diagnostic(
                            ValidationSeverity::Warning,
                            "runtime_computed_write_failed",
                            &computed.path,
                            format!(
                                "computed value `{}` could not be written: {error}",
                                computed.id
                            ),
                        )),
                    }
                }
                ExpressionEvaluation::Pending => next.push(computed),
                ExpressionEvaluation::Error(error) => {
                    if let Some(fallback) = computed.fallback.clone() {
                        match set_context_path(&mut context, &computed.path, fallback) {
                            Ok(()) => {
                                computed_applied = computed_applied.saturating_add(1);
                                computed_fallbacks = computed_fallbacks.saturating_add(1);
                                progress = true;
                                diagnostics.push(context_diagnostic(
                                    ValidationSeverity::Info,
                                    "runtime_computed_fallback_used",
                                    &computed.path,
                                    format!(
                                        "computed value `{}` used fallback after evaluation error: {error}",
                                        computed.id
                                    ),
                                ));
                            }
                            Err(write_error) => diagnostics.push(context_diagnostic(
                                ValidationSeverity::Warning,
                                "runtime_computed_write_failed",
                                &computed.path,
                                format!(
                                    "computed value `{}` fallback could not be written: {write_error}",
                                    computed.id
                                ),
                            )),
                        }
                    } else {
                        unresolved_computed = unresolved_computed.saturating_add(1);
                        diagnostics.push(context_diagnostic(
                            ValidationSeverity::Warning,
                            "runtime_computed_evaluation_failed",
                            &computed.path,
                            format!("computed value `{}` failed: {error}", computed.id),
                        ));
                    }
                }
            }
        }
        if !progress {
            pending = next;
            break;
        }
        pending = next;
    }

    for computed in pending {
        if let Some(fallback) = computed.fallback.clone() {
            match set_context_path(&mut context, &computed.path, fallback) {
                Ok(()) => {
                    computed_applied = computed_applied.saturating_add(1);
                    computed_fallbacks = computed_fallbacks.saturating_add(1);
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Info,
                        "runtime_computed_fallback_used",
                        &computed.path,
                        format!(
                            "computed value `{}` used fallback because dependencies did not resolve",
                            computed.id
                        ),
                    ));
                }
                Err(error) => {
                    unresolved_computed = unresolved_computed.saturating_add(1);
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Warning,
                        "runtime_computed_write_failed",
                        &computed.path,
                        format!(
                            "computed value `{}` fallback could not be written: {error}",
                            computed.id
                        ),
                    ));
                }
            }
        } else {
            unresolved_computed = unresolved_computed.saturating_add(1);
            diagnostics.push(context_diagnostic(
                ValidationSeverity::Warning,
                "runtime_computed_unresolved",
                &computed.path,
                format!(
                    "computed value `{}` dependencies did not resolve; a dependency cycle may exist",
                    computed.id
                ),
            ));
        }
    }

    let mut type_mismatches = 0usize;
    for field in &catalog.fields {
        match resolve_context_path(&context, &field.path) {
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
        if !paths.insert(field.path.clone()) {
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
            {
                if items.iter().any(|item| !item_kind.accepts(item)) {
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
    }

    let mut computed_paths = BTreeMap::<String, String>::new();
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
        if let Some(previous) = computed_paths.insert(computed.path.clone(), computed.id.clone()) {
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
            if dependency == computed.path {
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
    if segments.is_empty() {
        *root = value;
        return Ok(());
    }
    set_path_segments(root, &segments, value)
}

fn normalize_context(value: Value) -> Value {
    match value {
        Value::Null => Value::Object(Map::new()),
        value => value,
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContextPathSegment {
    Key(String),
    Index(usize),
}

fn parse_context_path(path: &str) -> Option<Vec<ContextPathSegment>> {
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
                segments.push(ContextPathSegment::Key(std::mem::take(&mut token)));
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
                if !closed || index.is_empty() {
                    return None;
                }
                segments.push(ContextPathSegment::Index(index.parse().ok()?));
                if chars.peek() == Some(&'.') {
                    chars.next();
                }
            }
            ']' => return None,
            _ => token.push(character),
        }
    }
    if !token.is_empty() {
        segments.push(ContextPathSegment::Key(token));
    }
    (!segments.is_empty()).then_some(segments)
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
        {
            if items.iter().any(|item| !item_kind.accepts(item)) {
                return Err(FlyError::Decode(format!(
                    "runtime context field default contains items that are not {}",
                    item_kind.as_str()
                )));
            }
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
    if paths.contains(&computed.path) {
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
            if template.contains("{{") && !template.contains("}}") {
                diagnostics.push(context_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_computed_format_unclosed",
                    path,
                    format!("computed value `{id}` has an unclosed format placeholder"),
                ));
            }
            for dependency in format_dependencies(template) {
                if parse_context_path(&dependency).is_none() {
                    diagnostics.push(context_diagnostic(
                        ValidationSeverity::Error,
                        "runtime_computed_dependency_path_invalid",
                        path,
                        format!("computed value `{id}` dependency `{dependency}` is invalid"),
                    ));
                }
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
    let targets = computed
        .iter()
        .map(|value| (value.path.clone(), value.id.clone()))
        .collect::<BTreeMap<_, _>>();
    let graph = computed
        .iter()
        .map(|value| {
            let mut dependencies = BTreeSet::new();
            collect_expression_paths(&value.expression, &mut dependencies);
            let dependencies = dependencies
                .into_iter()
                .filter(|path| targets.contains_key(path))
                .collect::<Vec<_>>();
            (value.path.clone(), dependencies)
        })
        .collect::<BTreeMap<_, _>>();

    let mut diagnostics = Vec::new();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for path in graph.keys() {
        detect_cycle_from(
            path,
            &graph,
            &targets,
            &mut visiting,
            &mut visited,
            &mut Vec::new(),
            &mut diagnostics,
        );
    }
    diagnostics
}

fn detect_cycle_from(
    path: &str,
    graph: &BTreeMap<String, Vec<String>>,
    targets: &BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if visited.contains(path) {
        return;
    }
    if visiting.contains(path) {
        let start = stack.iter().position(|value| value == path).unwrap_or(0);
        let mut cycle = stack[start..].to_vec();
        cycle.push(path.to_string());
        diagnostics.push(context_diagnostic(
            ValidationSeverity::Error,
            "runtime_computed_dependency_cycle",
            path,
            format!(
                "computed context dependency cycle: {}",
                cycle
                    .iter()
                    .map(|value| targets.get(value).cloned().unwrap_or_else(|| value.clone()))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
        ));
        return;
    }

    visiting.insert(path.to_string());
    stack.push(path.to_string());
    if let Some(dependencies) = graph.get(path) {
        for dependency in dependencies {
            detect_cycle_from(
                dependency,
                graph,
                targets,
                visiting,
                visited,
                stack,
                diagnostics,
            );
        }
    }
    stack.pop();
    visiting.remove(path);
    visited.insert(path.to_string());
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
        document.project.extensions.remove(field);
    } else {
        document
            .project
            .extensions
            .insert(field.to_string(), Value::Array(entries));
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
    fn commands_preserve_opaque_entries() {
        let mut document = document();
        document.project.extensions.insert(
            FLY_RUNTIME_CONTEXT_SCHEMA_FIELD.to_string(),
            serde_json::json!([{ "providerSchema": true }]),
        );
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
        let entries = document.project.extensions[FLY_RUNTIME_CONTEXT_SCHEMA_FIELD]
            .as_array()
            .expect("schema entries");
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|entry| entry.get("providerSchema").is_some())
        );
    }
}
