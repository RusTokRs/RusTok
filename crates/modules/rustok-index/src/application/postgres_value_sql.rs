use crate::domain::{
    FieldName, FieldPath, IndexValue, IndexValueType, SchemaFingerprint, SchemaRef,
};

use super::{
    PlannedField, PostgresBindValue, PostgresQueryCompileError, postgres_compiler::quote_identifier,
};

pub(crate) const MAX_MANY_PROJECTION_ITEMS: usize = 256;
pub(crate) const MAX_MANY_PROJECTION_BYTES: usize = 128 * 1024;
pub(crate) const PROJECTION_LIMIT_TAG: &str = "projection_budget_exceeded";
const MAX_BIND_COUNT: usize = 4096;
const MAX_BIND_BYTES: usize = 1024 * 1024;

/// Only validated owner identifiers, UUIDs and fixed-size fingerprints are literals. Keeping
/// these qualifiers literal makes the matching partial index usable even by a generic plan.
/// Tenant equality remains mandatory on every root and reached entity.
pub(crate) fn schema_scope_sql(
    column_prefix: &str,
    tenant_id: uuid::Uuid,
    schema: &SchemaRef,
    fingerprint: SchemaFingerprint,
) -> String {
    format!(
        "{column_prefix}tenant_id = '{tenant_id}'::uuid AND {column_prefix}module_name = '{}' \
         AND {column_prefix}entity_name = '{}' AND {column_prefix}schema_version = {} \
         AND {column_prefix}schema_fingerprint = '{fingerprint}' AND {column_prefix}is_deleted = FALSE",
        schema.module.as_str(),
        schema.entity.as_str(),
        schema.version.get(),
    )
}

pub(super) fn bound_projected_value(raw: &str) -> String {
    format!(
        "CASE WHEN octet_length(({raw})::text) > {MAX_MANY_PROJECTION_BYTES} \
         THEN jsonb_build_object('type', '{PROJECTION_LIMIT_TAG}') ELSE {raw} END"
    )
}

#[derive(Default)]
pub(super) struct Bindings {
    values: Vec<PostgresBindValue>,
}

impl Bindings {
    pub(super) fn push(&mut self, value: PostgresBindValue) -> String {
        self.values.push(value);
        format!("${}", self.values.len())
    }

    pub(super) fn finish(self) -> Result<Vec<PostgresBindValue>, PostgresQueryCompileError> {
        if self.values.len() > MAX_BIND_COUNT {
            return Err(PostgresQueryCompileError::BindBudgetExceeded);
        }
        let mut bytes = 0usize;
        for value in &self.values {
            let len = match value {
                PostgresBindValue::Text(text) => text.len(),
                PostgresBindValue::Json(json) => serde_json::to_vec(json)?.len(),
                _ => 64,
            };
            bytes = bytes.saturating_add(len);
            if bytes > MAX_BIND_BYTES {
                return Err(PostgresQueryCompileError::BindBudgetExceeded);
            }
        }
        Ok(self.values)
    }
}

pub(super) struct FieldSql {
    pub(super) raw: String,
    pub(super) scalar: String,
    pub(super) list_values: String,
    pub(super) type_text: String,
    pub(super) null_predicate: String,
}

impl FieldSql {
    pub(super) fn non_null_predicate(&self) -> String {
        format!(
            "({} IS NOT NULL AND {} IS NOT NULL AND {} <> 'null')",
            self.raw, self.type_text, self.type_text
        )
    }
}

pub(super) fn field_sql_for_alias(field: &PlannedField, relation_alias: &str) -> FieldSql {
    let relation_alias = quote_identifier(relation_alias);
    let payload = format!("{relation_alias}.payload");
    let raw = raw_expression(&payload, field.path.field());
    let scalar = scalar_expression(&payload, field.path.field(), field.value_type);
    let type_text = format!("({raw} ->> 'type')");
    let null_predicate = format!("{raw} IS NULL OR {type_text} IS NULL OR {type_text} = 'null'");
    FieldSql {
        raw,
        scalar,
        list_values: list_expression(&payload, field.path.field()),
        type_text,
        null_predicate,
    }
}

fn raw_expression(payload: &str, field: &FieldName) -> String {
    // FieldName's alphabet only allows lowercase ASCII letters, digits, '_', '-' and '.'.
    format!("({payload} -> '{}')", field.as_str())
}

pub(crate) fn list_expression(payload: &str, field: &FieldName) -> String {
    format!("({} -> 'value')", raw_expression(payload, field))
}

pub(crate) fn scalar_expression(
    payload: &str,
    field: &FieldName,
    value_type: IndexValueType,
) -> String {
    let text = format!("({} ->> 'value')", raw_expression(payload, field));
    match value_type {
        IndexValueType::Boolean => format!("({text}::boolean)"),
        IndexValueType::Integer => format!("({text}::bigint)"),
        IndexValueType::Decimal => format!("({text}::numeric)"),
        IndexValueType::String => format!("({text} COLLATE \"C\")"),
        IndexValueType::Uuid => format!("({text}::uuid)"),
        // Strip the trailing Z: the whole-second form then sorts before its fractional forms.
        IndexValueType::Timestamp => format!("(left({text}, -1) COLLATE \"C\")"),
    }
}

pub(super) fn contains_predicate(
    sql: &FieldSql,
    value: &IndexValue,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let list = &sql.list_values;
    if let IndexValue::Decimal(value) = value {
        // Decimal in the storage wire is a string. Compare SQL numeric keys so scale
        // differences do not affect membership, including rows already written.
        let value = bindings.push(PostgresBindValue::Decimal(*value));
        Ok(format!(
            "EXISTS (SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof({list}) = 'array' THEN {list} ELSE '[]'::jsonb END) AS member(value) WHERE (member.value->>'value')::numeric = {value})"
        ))
    } else {
        let encoded = serde_json::to_value([value])?;
        let value = bindings.push(PostgresBindValue::Json(encoded));
        Ok(format!("{list} @> {value}::jsonb"))
    }
}

pub(super) fn scalar_bind(
    path: &FieldPath,
    value: &IndexValue,
) -> Result<PostgresBindValue, PostgresQueryCompileError> {
    if !value.is_storable_scalar() {
        return Err(PostgresQueryCompileError::InvalidScalarValue(path.clone()));
    }
    match value {
        IndexValue::Boolean(value) => Ok(PostgresBindValue::Boolean(*value)),
        IndexValue::Integer(value) => Ok(PostgresBindValue::Integer(*value)),
        IndexValue::Decimal(value) => Ok(PostgresBindValue::Decimal(*value)),
        IndexValue::String(value) => Ok(PostgresBindValue::Text(value.clone())),
        IndexValue::Uuid(value) => Ok(PostgresBindValue::Uuid(*value)),
        IndexValue::Timestamp(value) => {
            let utc = value.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
            Ok(PostgresBindValue::Text(
                utc.trim_end_matches('Z').to_owned(),
            ))
        }
        IndexValue::Null | IndexValue::List(_) => {
            Err(PostgresQueryCompileError::InvalidScalarValue(path.clone()))
        }
    }
}
