use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{
    FieldPath, FilterExpr, IndexValue, IndexValueType, LocalizedEntityQuery, OrderDirection,
    Pagination,
};

use super::{
    CompiledPostgresCount, CompiledPostgresQuery, CompiledQueryColumn, ExecutableQueryPlan,
    LocalizedCursorCodec, LocalizedCursorValidationError, LocalizedEntityQueryValidationError,
    LocalizedIndexCursor, PlannedField, PostgresBindValue, PostgresQueryCompileError,
    QueryPlanError, QueryPlanFingerprint, SchemaRegistry, postgres_compiler::quote_identifier,
};

const ROOT_ALIAS: &str = "t0";
const REQUESTED_ALIAS: &str = "t1";
const FALLBACK_ALIAS: &str = "t2";
const ANY_LOCALE_ALIAS: &str = "t3";
const EARLIER_ANCHOR_ALIAS: &str = "t4";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalizedQueryPlanFingerprint([u8; 32]);

impl LocalizedQueryPlanFingerprint {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for LocalizedQueryPlanFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPostgresLocalizedPageQuery {
    compiled: CompiledPostgresQuery,
    requested_page_size: u32,
    localized_plan_fingerprint: LocalizedQueryPlanFingerprint,
}

impl CompiledPostgresLocalizedPageQuery {
    pub fn compiled(&self) -> &CompiledPostgresQuery {
        &self.compiled
    }

    pub fn compiled_mut(&mut self) -> &mut CompiledPostgresQuery {
        &mut self.compiled
    }

    pub fn requested_page_size(&self) -> u32 {
        self.requested_page_size
    }

    pub fn localized_plan_fingerprint(&self) -> LocalizedQueryPlanFingerprint {
        self.localized_plan_fingerprint
    }

    pub fn into_compiled(self) -> CompiledPostgresQuery {
        self.compiled
    }
}

#[derive(Debug, Error)]
pub enum PostgresLocalizedQueryBuildError {
    #[error(transparent)]
    Validation(#[from] LocalizedEntityQueryValidationError),
    #[error(transparent)]
    Plan(#[from] QueryPlanError),
    #[error(transparent)]
    Cursor(#[from] LocalizedCursorValidationError),
    #[error(transparent)]
    Compile(#[from] PostgresQueryCompileError),
}

#[derive(Serialize)]
struct LocalizedPlanIdentity<'a> {
    mode: &'static str,
    ordinary_plan_fingerprint: &'a [u8; 32],
    fallback_locale: Option<&'a crate::domain::LocaleKey>,
    any_locale_filter: &'a Option<FilterExpr>,
    localized_projection_fields: Vec<&'a FieldPath>,
    identity_order_direction: OrderDirection,
}

impl SchemaRegistry {
    pub fn compile_postgres_localized_page_query(
        &self,
        query: &LocalizedEntityQuery,
    ) -> Result<CompiledPostgresLocalizedPageQuery, PostgresLocalizedQueryBuildError> {
        self.validate_localized_entity_query(query)?;
        let plan = self.plan_query(&query.query)?;
        let any_locale_plan = match &query.any_locale_filter {
            Some(filter) => {
                let mut probe = query.query.clone();
                probe.filter = Some(filter.clone());
                Some(self.plan_query(&probe)?)
            }
            None => None,
        };
        let cursor = match &query.query.pagination {
            Pagination::Cursor {
                after: Some(encoded),
                ..
            } => Some(LocalizedCursorCodec::decode_scoped_for_query(
                encoded, query, self,
            )?),
            _ => None,
        };
        compile_localized_page(query, &plan, any_locale_plan.as_ref(), cursor.as_ref())
            .map_err(Into::into)
    }
}

fn compile_localized_page(
    query: &LocalizedEntityQuery,
    plan: &ExecutableQueryPlan,
    any_locale_plan: Option<&ExecutableQueryPlan>,
    cursor: Option<&LocalizedIndexCursor>,
) -> Result<CompiledPostgresLocalizedPageQuery, PostgresQueryCompileError> {
    let requested_page_size = page_size(&plan.pagination);
    let mut bindings = Bindings::default();
    let base = compile_base(query, plan, &mut bindings, true)?;
    let mut select = vec![format!(
        "{}.entity_id AS {}",
        quote_identifier(ROOT_ALIAS),
        quote_identifier("__t0_entity_id")
    )];
    let mut columns = vec![CompiledQueryColumn::EntityId {
        output_alias: "__t0_entity_id".to_owned(),
        relation_alias: ROOT_ALIAS.to_owned(),
    }];

    for (index, field) in plan.projection.iter().enumerate() {
        let raw = if query.is_localized_projection_path(&field.path) {
            localized_projection_sql(query, field, &mut bindings)
        } else {
            field_sql_for_alias(field, ROOT_ALIAS, &mut bindings).raw
        };
        let output_alias = format!("f{index}");
        select.push(format!("{raw} AS {}", quote_identifier(&output_alias)));
        columns.push(CompiledQueryColumn::Field {
            output_alias,
            field: field.clone(),
        });
    }

    for (index, order) in plan.order_by.iter().enumerate() {
        let sql = field_sql_for_alias(&order.field, ROOT_ALIAS, &mut bindings);
        let output_alias = format!("__order_{index}");
        select.push(format!(
            "{} AS {}",
            sql.raw,
            quote_identifier(&output_alias)
        ));
        columns.push(CompiledQueryColumn::OrderValue {
            output_alias,
            field: order.field.clone(),
        });
    }

    let mut predicates = base.predicates;
    if let Some(filter) = &plan.filter {
        predicates.push(compile_filter_for_alias(
            plan,
            filter,
            ROOT_ALIAS,
            &mut bindings,
        )?);
    }
    if let (Some(filter), Some(any_plan)) = (&query.any_locale_filter, any_locale_plan) {
        predicates.push(compile_any_locale_exists(
            plan,
            any_plan,
            filter,
            &mut bindings,
        )?);
    }
    if let Some(cursor) = cursor {
        predicates.push(compile_keyset(query, plan, cursor, &mut bindings)?);
    }

    let order = compile_order(query, plan, &mut bindings)?;
    let pagination = compile_pagination_with_lookahead(&plan.pagination, &mut bindings)?;
    let sql = format!(
        "SELECT {} {} WHERE {} {order} {pagination}",
        select.join(", "),
        base.from_sql,
        predicates.join(" AND "),
    );
    let exact_count = plan
        .include_exact_count
        .then(|| compile_exact_count(query, plan, any_locale_plan))
        .transpose()?;
    let ordinary_plan_fingerprint = plan.fingerprint()?;
    let localized_plan_fingerprint = localized_plan_fingerprint(query, ordinary_plan_fingerprint)?;

    Ok(CompiledPostgresLocalizedPageQuery {
        compiled: CompiledPostgresQuery {
            sql,
            binds: bindings.values,
            columns,
            many_relations: Vec::new(),
            exact_count,
            plan_fingerprint: ordinary_plan_fingerprint,
        },
        requested_page_size,
        localized_plan_fingerprint,
    })
}

pub(super) fn localized_plan_fingerprint(
    query: &LocalizedEntityQuery,
    ordinary_plan_fingerprint: QueryPlanFingerprint,
) -> Result<LocalizedQueryPlanFingerprint, PostgresQueryCompileError> {
    let mut localized_projection_fields =
        query.localized_projection_fields.iter().collect::<Vec<_>>();
    localized_projection_fields.sort();
    let identity = LocalizedPlanIdentity {
        mode: "localized_entity_fold_plan_v1",
        ordinary_plan_fingerprint: ordinary_plan_fingerprint.as_bytes(),
        fallback_locale: query.canonical_fallback_locale(),
        any_locale_filter: &query.any_locale_filter,
        localized_projection_fields,
        identity_order_direction: query.identity_order_direction,
    };
    let bytes = postcard::to_stdvec(&identity)?;
    let mut hasher = Sha256::new();
    hasher.update(b"rustok-index-localized-plan-v1");
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    Ok(LocalizedQueryPlanFingerprint(hasher.finalize().into()))
}

struct LocalizedBase {
    from_sql: String,
    predicates: Vec<String>,
}

fn compile_base(
    query: &LocalizedEntityQuery,
    plan: &ExecutableQueryPlan,
    bindings: &mut Bindings,
    include_projection_rows: bool,
) -> Result<LocalizedBase, PostgresQueryCompileError> {
    let root = quote_identifier(ROOT_ALIAS);
    let tenant = bindings.push(PostgresBindValue::Uuid(plan.scope.tenant_id));
    let module = bindings.push(PostgresBindValue::Text(
        plan.root_schema.module.as_str().to_owned(),
    ));
    let entity = bindings.push(PostgresBindValue::Text(
        plan.root_schema.entity.as_str().to_owned(),
    ));
    let version = bindings.push(PostgresBindValue::Integer(i64::from(
        plan.root_schema.version.get(),
    )));

    let mut from_sql = format!("FROM index_entities AS {root}");
    if include_projection_rows {
        let requested = quote_identifier(REQUESTED_ALIAS);
        let requested_locale = bindings.push(PostgresBindValue::Text(
            query
                .requested_locale()
                .expect("validated localized query carries requested locale")
                .as_str()
                .to_owned(),
        ));
        from_sql.push_str(&format!(
            " LEFT JOIN index_entities AS {requested} ON {} AND {requested}.locale_key = {requested_locale} AND {requested}.is_deleted = FALSE",
            same_identity_predicate(REQUESTED_ALIAS, ROOT_ALIAS),
        ));
        if let Some(fallback) = query.canonical_fallback_locale() {
            let fallback_alias = quote_identifier(FALLBACK_ALIAS);
            let fallback_locale =
                bindings.push(PostgresBindValue::Text(fallback.as_str().to_owned()));
            from_sql.push_str(&format!(
                " LEFT JOIN index_entities AS {fallback_alias} ON {} AND {fallback_alias}.locale_key = {fallback_locale} AND {fallback_alias}.is_deleted = FALSE",
                same_identity_predicate(FALLBACK_ALIAS, ROOT_ALIAS),
            ));
        }
    }

    let earlier = quote_identifier(EARLIER_ANCHOR_ALIAS);
    let root_predicate = format!(
        "{root}.tenant_id = {tenant} AND {root}.module_name = {module} AND {root}.entity_name = {entity} AND {root}.schema_version = {version} AND {root}.locale_key IS NOT NULL AND {root}.is_deleted = FALSE"
    );
    let anchor_predicate = format!(
        "NOT EXISTS (SELECT 1 FROM index_entities AS {earlier} WHERE {} AND {earlier}.locale_key IS NOT NULL AND {earlier}.locale_key < {root}.locale_key AND {earlier}.is_deleted = FALSE)",
        same_identity_predicate(EARLIER_ANCHOR_ALIAS, ROOT_ALIAS),
    );

    Ok(LocalizedBase {
        from_sql,
        predicates: vec![root_predicate, anchor_predicate],
    })
}

fn same_identity_predicate(candidate: &str, root: &str) -> String {
    let candidate = quote_identifier(candidate);
    let root = quote_identifier(root);
    format!(
        "{candidate}.tenant_id = {root}.tenant_id AND {candidate}.module_name = {root}.module_name AND {candidate}.entity_name = {root}.entity_name AND {candidate}.schema_version = {root}.schema_version AND {candidate}.entity_id = {root}.entity_id"
    )
}

fn localized_projection_sql(
    query: &LocalizedEntityQuery,
    field: &PlannedField,
    bindings: &mut Bindings,
) -> String {
    let requested = quote_identifier(REQUESTED_ALIAS);
    let requested_raw = field_sql_for_alias(field, REQUESTED_ALIAS, bindings).raw;
    match query.canonical_fallback_locale() {
        Some(_) => {
            let fallback = quote_identifier(FALLBACK_ALIAS);
            let fallback_raw = field_sql_for_alias(field, FALLBACK_ALIAS, bindings).raw;
            format!(
                "CASE WHEN {requested}.entity_id IS NOT NULL THEN {requested_raw} WHEN {fallback}.entity_id IS NOT NULL THEN {fallback_raw} ELSE NULL::jsonb END"
            )
        }
        None => format!(
            "CASE WHEN {requested}.entity_id IS NOT NULL THEN {requested_raw} ELSE NULL::jsonb END"
        ),
    }
}

fn compile_any_locale_exists(
    root_plan: &ExecutableQueryPlan,
    any_locale_plan: &ExecutableQueryPlan,
    filter: &FilterExpr,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let any_alias = quote_identifier(ANY_LOCALE_ALIAS);
    let filter = compile_filter_for_alias(any_locale_plan, filter, ANY_LOCALE_ALIAS, bindings)?;
    Ok(format!(
        "EXISTS (SELECT 1 FROM index_entities AS {any_alias} WHERE {} AND {any_alias}.locale_key IS NOT NULL AND {any_alias}.is_deleted = FALSE AND ({filter}))",
        same_identity_predicate(ANY_LOCALE_ALIAS, &root_plan.root_alias),
    ))
}

fn compile_filter_for_alias(
    plan: &ExecutableQueryPlan,
    filter: &FilterExpr,
    alias: &str,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    match filter {
        FilterExpr::And(children) => compile_logical(plan, children, alias, "AND", bindings),
        FilterExpr::Or(children) => compile_logical(plan, children, alias, "OR", bindings),
        FilterExpr::Not(child) => Ok(format!(
            "NOT ({})",
            compile_filter_for_alias(plan, child, alias, bindings)?
        )),
        FilterExpr::Eq(path, value) => {
            let sql = field_sql_for_path(plan, path, alias, bindings)?;
            let value = bindings.push(scalar_bind(path, value)?);
            Ok(format!("COALESCE({} = {value}, FALSE)", sql.scalar))
        }
        FilterExpr::Ne(path, value) => {
            let sql = field_sql_for_path(plan, path, alias, bindings)?;
            let value = bindings.push(scalar_bind(path, value)?);
            Ok(format!(
                "({} IS NOT NULL AND {} <> {value})",
                sql.scalar, sql.scalar
            ))
        }
        FilterExpr::In(path, values) => {
            let sql = field_sql_for_path(plan, path, alias, bindings)?;
            let values = values
                .iter()
                .map(|value| Ok(bindings.push(scalar_bind(path, value)?)))
                .collect::<Result<Vec<_>, PostgresQueryCompileError>>()?;
            Ok(format!(
                "COALESCE({} IN ({}), FALSE)",
                sql.scalar,
                values.join(", ")
            ))
        }
        FilterExpr::Gt(path, value) => compile_range(plan, path, value, alias, ">", bindings),
        FilterExpr::Gte(path, value) => compile_range(plan, path, value, alias, ">=", bindings),
        FilterExpr::Lt(path, value) => compile_range(plan, path, value, alias, "<", bindings),
        FilterExpr::Lte(path, value) => compile_range(plan, path, value, alias, "<=", bindings),
        FilterExpr::Contains(path, value) => {
            let sql = field_sql_for_path(plan, path, alias, bindings)?;
            let encoded = serde_json::to_value(vec![value.clone()])?;
            let value = bindings.push(PostgresBindValue::Json(encoded));
            Ok(format!(
                "COALESCE({} @> {value}::jsonb, FALSE)",
                sql.list_values
            ))
        }
        FilterExpr::IsNull(path, expected_null) => {
            let sql = field_sql_for_path(plan, path, alias, bindings)?;
            if *expected_null {
                Ok(format!("({})", sql.null_predicate))
            } else {
                Ok(format!("NOT ({})", sql.null_predicate))
            }
        }
        FilterExpr::TextLike(path, pattern) => {
            let sql = field_sql_for_path(plan, path, alias, bindings)?;
            let pattern = bindings.push(PostgresBindValue::Text(pattern.clone()));
            Ok(format!(
                "COALESCE({} LIKE {pattern} ESCAPE E'\\\\', FALSE)",
                sql.scalar
            ))
        }
    }
}

fn compile_logical(
    plan: &ExecutableQueryPlan,
    children: &[FilterExpr],
    alias: &str,
    operator: &str,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let children = children
        .iter()
        .map(|child| compile_filter_for_alias(plan, child, alias, bindings))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("({})", children.join(&format!(" {operator} "))))
}

fn compile_range(
    plan: &ExecutableQueryPlan,
    path: &FieldPath,
    value: &IndexValue,
    alias: &str,
    operator: &str,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let sql = field_sql_for_path(plan, path, alias, bindings)?;
    let value = bindings.push(scalar_bind(path, value)?);
    Ok(format!(
        "COALESCE({} {operator} {value}, FALSE)",
        sql.scalar
    ))
}

fn field_sql_for_path(
    plan: &ExecutableQueryPlan,
    path: &FieldPath,
    alias: &str,
    bindings: &mut Bindings,
) -> Result<FieldSql, PostgresQueryCompileError> {
    let field = plan
        .field(path)
        .ok_or_else(|| PostgresQueryCompileError::MissingFieldPlan(path.clone()))?;
    Ok(field_sql_for_alias(field, alias, bindings))
}

fn compile_keyset(
    query: &LocalizedEntityQuery,
    plan: &ExecutableQueryPlan,
    cursor: &LocalizedIndexCursor,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let mut equalities = Vec::new();
    let mut disjuncts = Vec::new();
    for (order, cursor_value) in plan.order_by.iter().zip(&cursor.order_values) {
        let sql = field_sql_for_alias(&order.field, ROOT_ALIAS, bindings);
        let (equal, after) = cursor_field_predicates(
            &order.field.path,
            &sql,
            order.direction.base_direction(),
            cursor_value,
            bindings,
        )?;
        disjuncts.push(conjunction(&equalities, &after));
        equalities.push(equal);
    }

    let entity_id = bindings.push(PostgresBindValue::Uuid(cursor.entity_id));
    let root_after = match query.identity_order_direction {
        OrderDirection::Asc => format!("{}.entity_id > {entity_id}", quote_identifier(ROOT_ALIAS)),
        OrderDirection::Desc => format!("{}.entity_id < {entity_id}", quote_identifier(ROOT_ALIAS)),
        _ => unreachable!("validated localized identity order is asc or desc"),
    };
    disjuncts.push(conjunction(&equalities, &root_after));
    Ok(format!("({})", disjuncts.join(" OR ")))
}

fn cursor_field_predicates(
    path: &FieldPath,
    sql: &FieldSql,
    direction: OrderDirection,
    cursor_value: &IndexValue,
    bindings: &mut Bindings,
) -> Result<(String, String), PostgresQueryCompileError> {
    let non_null = format!("NOT ({})", sql.null_predicate);
    if matches!(cursor_value, IndexValue::Null) {
        let after = match direction {
            OrderDirection::Asc => "FALSE".to_owned(),
            OrderDirection::Desc => non_null,
            _ => unreachable!("cursor predicates receive a normalized direction"),
        };
        return Ok((format!("({})", sql.null_predicate), after));
    }

    let value = bindings.push(scalar_bind(path, cursor_value)?);
    let equal = format!("({non_null} AND {} = {value})", sql.scalar);
    let after = match direction {
        OrderDirection::Asc => format!(
            "(({}) OR ({non_null} AND {} > {value}))",
            sql.null_predicate, sql.scalar
        ),
        OrderDirection::Desc => {
            format!("({non_null} AND {} < {value})", sql.scalar)
        }
        _ => unreachable!("cursor predicates receive a normalized direction"),
    };
    Ok((equal, after))
}

fn conjunction(prefix: &[String], final_predicate: &str) -> String {
    let mut predicates = prefix.to_vec();
    predicates.push(final_predicate.to_owned());
    format!("({})", predicates.join(" AND "))
}

fn compile_order(
    query: &LocalizedEntityQuery,
    plan: &ExecutableQueryPlan,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let mut terms = plan
        .order_by
        .iter()
        .map(|order| {
            let sql = field_sql_for_alias(&order.field, ROOT_ALIAS, bindings);
            Ok(match order.direction.base_direction() {
                OrderDirection::Asc => format!("{} ASC NULLS LAST", sql.scalar),
                OrderDirection::Desc => format!("{} DESC NULLS FIRST", sql.scalar),
                _ => unreachable!("validated root-only localized ordering has no aggregate mode"),
            })
        })
        .collect::<Result<Vec<_>, PostgresQueryCompileError>>()?;
    let identity_direction = match query.identity_order_direction {
        OrderDirection::Asc => "ASC",
        OrderDirection::Desc => "DESC",
        _ => unreachable!("validated localized identity order is asc or desc"),
    };
    terms.push(format!(
        "{}.entity_id {identity_direction}",
        quote_identifier(ROOT_ALIAS)
    ));
    Ok(format!("ORDER BY {}", terms.join(", ")))
}

fn compile_pagination_with_lookahead(
    pagination: &Pagination,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    match pagination {
        Pagination::Cursor { first, .. } => {
            let limit = bindings.push(PostgresBindValue::Integer(i64::from(*first) + 1));
            Ok(format!("LIMIT {limit}"))
        }
        Pagination::Offset { limit, offset } => {
            let limit = bindings.push(PostgresBindValue::Integer(i64::from(*limit) + 1));
            let offset_value = i64::try_from(*offset)
                .map_err(|_| PostgresQueryCompileError::OffsetOutOfRange(*offset))?;
            let offset = bindings.push(PostgresBindValue::Integer(offset_value));
            Ok(format!("LIMIT {limit} OFFSET {offset}"))
        }
    }
}

fn compile_exact_count(
    query: &LocalizedEntityQuery,
    plan: &ExecutableQueryPlan,
    any_locale_plan: Option<&ExecutableQueryPlan>,
) -> Result<CompiledPostgresCount, PostgresQueryCompileError> {
    let mut bindings = Bindings::default();
    let base = compile_base(query, plan, &mut bindings, false)?;
    let mut predicates = base.predicates;
    if let Some(filter) = &plan.filter {
        predicates.push(compile_filter_for_alias(
            plan,
            filter,
            ROOT_ALIAS,
            &mut bindings,
        )?);
    }
    if let (Some(filter), Some(any_plan)) = (&query.any_locale_filter, any_locale_plan) {
        predicates.push(compile_any_locale_exists(
            plan,
            any_plan,
            filter,
            &mut bindings,
        )?);
    }
    Ok(CompiledPostgresCount {
        sql: format!(
            "SELECT COUNT(*)::bigint AS \"__exact_count\" {} WHERE {}",
            base.from_sql,
            predicates.join(" AND "),
        ),
        binds: bindings.values,
    })
}

fn page_size(pagination: &Pagination) -> u32 {
    match pagination {
        Pagination::Cursor { first, .. } => *first,
        Pagination::Offset { limit, .. } => *limit,
    }
}

struct FieldSql {
    raw: String,
    scalar: String,
    list_values: String,
    null_predicate: String,
}

fn field_sql_for_alias(
    field: &PlannedField,
    relation_alias: &str,
    bindings: &mut Bindings,
) -> FieldSql {
    let relation_alias = quote_identifier(relation_alias);
    let field_name = bindings.push(PostgresBindValue::Text(
        field.path.field().as_str().to_owned(),
    ));
    let raw = format!("jsonb_extract_path({relation_alias}.payload, {field_name}::text)");
    let scalar_text =
        format!("jsonb_extract_path_text({relation_alias}.payload, {field_name}::text, 'value')");
    let type_text =
        format!("jsonb_extract_path_text({relation_alias}.payload, {field_name}::text, 'type')");
    let scalar = match field.value_type {
        IndexValueType::Boolean => format!("({scalar_text})::boolean"),
        IndexValueType::Integer => format!("({scalar_text})::bigint"),
        IndexValueType::Decimal => format!("({scalar_text})::numeric"),
        IndexValueType::String => format!("({scalar_text} COLLATE \"C\")"),
        IndexValueType::Uuid => format!("({scalar_text})::uuid"),
        IndexValueType::Timestamp => format!("({scalar_text})::timestamptz"),
    };
    let null_predicate = format!("{raw} IS NULL OR {type_text} IS NULL OR {type_text} = 'null'");
    FieldSql {
        raw,
        scalar,
        list_values: format!(
            "jsonb_extract_path({relation_alias}.payload, {field_name}::text, 'value')"
        ),
        null_predicate,
    }
}

fn scalar_bind(
    path: &FieldPath,
    value: &IndexValue,
) -> Result<PostgresBindValue, PostgresQueryCompileError> {
    match value {
        IndexValue::Boolean(value) => Ok(PostgresBindValue::Boolean(*value)),
        IndexValue::Integer(value) => Ok(PostgresBindValue::Integer(*value)),
        IndexValue::Decimal(value) => Ok(PostgresBindValue::Decimal(value.to_owned())),
        IndexValue::String(value) => Ok(PostgresBindValue::Text(value.clone())),
        IndexValue::Uuid(value) => Ok(PostgresBindValue::Uuid(*value)),
        IndexValue::Timestamp(value) => Ok(PostgresBindValue::Timestamp(value.to_owned())),
        IndexValue::Null | IndexValue::List(_) => {
            Err(PostgresQueryCompileError::InvalidScalarValue(path.clone()))
        }
    }
}

#[derive(Default)]
struct Bindings {
    values: Vec<PostgresBindValue>,
}

impl Bindings {
    fn push(&mut self, value: PostgresBindValue) -> String {
        self.values.push(value);
        format!("${}", self.values.len())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::PostgresQueryEntityAdmission;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexQuery, IndexQueryScope,
        IndexSchema, LocaleKey, LocaleMode, ModuleName, OrderExpr, SchemaRef, SchemaVersion,
    };

    fn schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("rustok-product").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::new(4),
            },
            locale_mode: LocaleMode::Required,
            fields: vec![
                IndexField {
                    name: FieldName::new("id").unwrap(),
                    value_type: IndexValueType::Uuid,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: true,
                },
                IndexField {
                    name: FieldName::new("title").unwrap(),
                    value_type: IndexValueType::String,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: false,
                },
                IndexField {
                    name: FieldName::new("status").unwrap(),
                    value_type: IndexValueType::String,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: false,
                },
            ],
            links: Vec::new(),
        }
    }

    fn query(schema: &IndexSchema) -> LocalizedEntityQuery {
        LocalizedEntityQuery::new(
            IndexQuery {
                scope: IndexQueryScope {
                    tenant_id: Uuid::new_v4(),
                    locale: Some(LocaleKey::new("fi").unwrap()),
                },
                schema: schema.reference.clone(),
                fields: vec![
                    FieldPath::new(FieldName::new("id").unwrap()),
                    FieldPath::new(FieldName::new("title").unwrap()),
                    FieldPath::new(FieldName::new("status").unwrap()),
                ],
                filter: Some(FilterExpr::Eq(
                    FieldPath::new(FieldName::new("status").unwrap()),
                    IndexValue::String("active".to_owned()),
                )),
                order_by: vec![OrderExpr {
                    field: FieldPath::new(FieldName::new("id").unwrap()),
                    direction: OrderDirection::Asc,
                }],
                pagination: Pagination::Cursor {
                    first: 20,
                    after: None,
                },
                include_exact_count: true,
            },
            Some(LocaleKey::new("en").unwrap()),
            Some(FilterExpr::TextLike(
                FieldPath::new(FieldName::new("title").unwrap()),
                "%needle%".to_owned(),
            )),
        )
        .with_localized_projection_fields([FieldPath::new(FieldName::new("title").unwrap())])
    }

    #[test]
    fn compiler_groups_by_one_admitted_anchor_before_page_and_count() {
        let schema = schema();
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let query = query(&schema);
        let compiled = registry
            .compile_postgres_localized_page_query(&query)
            .unwrap();
        let sql = &compiled.compiled().sql;
        assert!(sql.contains("index_entities AS \"t0\""));
        assert!(sql.contains("index_entities AS \"t1\""));
        assert!(sql.contains("index_entities AS \"t2\""));
        assert!(sql.contains("index_entities AS \"t3\""));
        assert!(sql.contains("index_entities AS \"t4\""));
        assert!(sql.contains("\"t4\".locale_key < \"t0\".locale_key"));
        assert!(sql.contains("CASE WHEN \"t1\".entity_id IS NOT NULL"));
        assert!(sql.contains("WHEN \"t2\".entity_id IS NOT NULL"));
        assert!(sql.contains(" LIKE "));
        assert!(sql.contains("ESCAPE E'\\\\'"));
        assert!(sql.contains("\"t0\".entity_id ASC"));
        assert_eq!(compiled.requested_page_size(), 20);
        let count = compiled.compiled().exact_count.as_ref().unwrap();
        assert!(count.sql.contains("index_entities AS \"t0\""));
        assert!(count.sql.contains("index_entities AS \"t3\""));
        assert!(count.sql.contains("index_entities AS \"t4\""));
        assert!(count.sql.contains(" LIKE "));
        assert!(!count.sql.contains("index_entities AS \"t1\""));
    }

    #[test]
    fn localized_identity_tie_break_can_follow_descending_owner_order() {
        let schema = schema();
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let query = query(&schema).with_identity_order_direction(OrderDirection::Desc);
        let compiled = registry
            .compile_postgres_localized_page_query(&query)
            .unwrap();
        assert!(compiled.compiled().sql.contains("\"t0\".entity_id DESC"));
    }

    #[test]
    fn canonical_entity_admission_can_cover_every_fold_physical_row_role() {
        let schema = schema();
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let query = query(&schema);
        let mut compiled = registry
            .compile_postgres_localized_page_query(&query)
            .unwrap();
        let admission = PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").unwrap();
        admission.apply(compiled.compiled_mut()).unwrap();
        for alias in ["t0", "t1", "t2", "t3", "t4"] {
            let marker =
                format!("\"{alias}\".is_deleted = FALSE AND (\"{alias}\".source_version > 0)");
            assert!(
                compiled.compiled().sql.contains(&marker),
                "missing {marker}"
            );
        }
        for alias in ["t0", "t3", "t4"] {
            let marker =
                format!("\"{alias}\".is_deleted = FALSE AND (\"{alias}\".source_version > 0)");
            assert!(
                compiled
                    .compiled()
                    .exact_count
                    .as_ref()
                    .unwrap()
                    .sql
                    .contains(&marker),
                "missing count {marker}"
            );
        }
    }
}
