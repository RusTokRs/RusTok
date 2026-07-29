use crate::domain::{FieldPath, FilterExpr, IndexValue, IndexValueType, OrderDirection, Pagination};

use super::{
    cursor::IndexCursor,
    planner::{ExecutableQueryPlan, PlannedField},
    postgres_compiler::{
        CompiledPostgresCount, CompiledPostgresQuery, CompiledQueryColumn, PostgresBindValue,
        PostgresQueryCompileError, quote_identifier,
    },
};

pub(super) fn compile_postgres_plan(
    plan: &ExecutableQueryPlan,
    cursor: Option<&IndexCursor>,
) -> Result<CompiledPostgresQuery, PostgresQueryCompileError> {
    let mut bindings = Bindings::default();
    let base = compile_base(plan, &mut bindings);
    let mut select = Vec::new();
    let mut columns = Vec::new();
    push_identity_column(
        &mut select,
        &mut columns,
        &plan.root_alias,
        &base.root_alias,
    );
    for join in &plan.joins {
        push_identity_column(
            &mut select,
            &mut columns,
            &join.alias,
            &quote_identifier(&join.alias),
        );
    }

    for (index, field) in plan.projection.iter().enumerate() {
        let field_sql = field_sql(field, &mut bindings);
        let output_alias = format!("f{index}");
        select.push(format!(
            "{} AS {}",
            field_sql.raw,
            quote_identifier(&output_alias),
        ));
        columns.push(CompiledQueryColumn::Field {
            output_alias,
            field: field.clone(),
        });
    }

    for (index, order) in plan.order_by.iter().enumerate() {
        let field_sql = field_sql(&order.field, &mut bindings);
        let output_alias = format!("__order_{index}");
        select.push(format!(
            "{} AS {}",
            field_sql.raw,
            quote_identifier(&output_alias),
        ));
        columns.push(CompiledQueryColumn::OrderValue {
            output_alias,
            field: order.field.clone(),
        });
    }

    let mut predicates = base.predicates;
    if let Some(filter) = &plan.filter {
        predicates.push(compile_filter(plan, filter, &mut bindings)?);
    }
    if let Some(cursor) = cursor {
        predicates.push(compile_keyset(plan, cursor, &mut bindings)?);
    }

    let order = compile_order(plan, &mut bindings);
    let pagination = compile_pagination(&plan.pagination, &mut bindings)?;
    let sql = format!(
        "SELECT {} {} WHERE {} {order} {pagination}",
        select.join(", "),
        base.from_sql,
        predicates.join(" AND "),
    );

    let exact_count = plan
        .include_exact_count
        .then(|| compile_exact_count(plan))
        .transpose()?;

    Ok(CompiledPostgresQuery {
        sql,
        binds: bindings.values,
        columns,
        exact_count,
        plan_fingerprint: plan.fingerprint()?,
    })
}

struct CompiledBase {
    root_alias: String,
    from_sql: String,
    predicates: Vec<String>,
}

fn compile_base(plan: &ExecutableQueryPlan, bindings: &mut Bindings) -> CompiledBase {
    let root_alias = quote_identifier(&plan.root_alias);
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
    let locale = bindings.push(PostgresBindValue::Text(
        plan.scope
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned()),
    ));

    let mut from_sql = format!("FROM index_entities AS {root_alias}");
    for (index, join) in plan.joins.iter().enumerate() {
        let source_alias = quote_identifier(&join.source_alias);
        let target_alias = quote_identifier(&join.alias);
        let link_alias = quote_identifier(&format!("l{}", index + 1));
        let link_name = bindings.push(PostgresBindValue::Text(join.link.as_str().to_owned()));
        let target_module = bindings.push(PostgresBindValue::Text(
            join.target_schema.module.as_str().to_owned(),
        ));
        let target_entity = bindings.push(PostgresBindValue::Text(
            join.target_schema.entity.as_str().to_owned(),
        ));
        let target_version = bindings.push(PostgresBindValue::Integer(i64::from(
            join.target_schema.version.get(),
        )));
        from_sql.push_str(&format!(
            " LEFT JOIN index_links AS {link_alias} ON {link_alias}.tenant_id = {source_alias}.tenant_id AND {link_alias}.source_module = {source_alias}.module_name AND {link_alias}.source_entity = {source_alias}.entity_name AND {link_alias}.source_schema_version = {source_alias}.schema_version AND {link_alias}.source_entity_id = {source_alias}.entity_id AND {link_alias}.source_locale_key = {source_alias}.locale_key AND {link_alias}.source_version = {source_alias}.source_version AND {link_alias}.link_name = {link_name} AND {link_alias}.target_module = {target_module} AND {link_alias}.target_entity = {target_entity} AND {link_alias}.target_schema_version = {target_version} LEFT JOIN index_entities AS {target_alias} ON {target_alias}.tenant_id = {link_alias}.tenant_id AND {target_alias}.module_name = {link_alias}.target_module AND {target_alias}.entity_name = {link_alias}.target_entity AND {target_alias}.schema_version = {link_alias}.target_schema_version AND {target_alias}.entity_id = {link_alias}.target_entity_id AND {target_alias}.locale_key = {link_alias}.target_locale_key AND {target_alias}.is_deleted = FALSE",
        ));
    }

    CompiledBase {
        root_alias: root_alias.clone(),
        from_sql,
        predicates: vec![format!(
            "{root_alias}.tenant_id = {tenant} AND {root_alias}.module_name = {module} AND {root_alias}.entity_name = {entity} AND {root_alias}.schema_version = {version} AND {root_alias}.locale_key = {locale} AND {root_alias}.is_deleted = FALSE"
        )],
    }
}

fn compile_filter(
    plan: &ExecutableQueryPlan,
    filter: &FilterExpr,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    match filter {
        FilterExpr::And(children) => compile_logical(plan, children, "AND", bindings),
        FilterExpr::Or(children) => compile_logical(plan, children, "OR", bindings),
        FilterExpr::Not(child) => Ok(format!(
            "NOT ({})",
            compile_filter(plan, child, bindings)?
        )),
        FilterExpr::Eq(path, value) => {
            let field = planned_field(plan, path)?;
            let sql = field_sql(field, bindings);
            let value = bindings.push(scalar_bind(path, value)?);
            Ok(format!("COALESCE({} = {value}, FALSE)", sql.scalar))
        }
        FilterExpr::Ne(path, value) => {
            let field = planned_field(plan, path)?;
            let sql = field_sql(field, bindings);
            let value = bindings.push(scalar_bind(path, value)?);
            Ok(format!(
                "({} IS NOT NULL AND {} <> {value})",
                sql.scalar, sql.scalar
            ))
        }
        FilterExpr::In(path, values) => {
            let field = planned_field(plan, path)?;
            let sql = field_sql(field, bindings);
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
        FilterExpr::Gt(path, value) => {
            compile_range(plan, path, value, ">", bindings)
        }
        FilterExpr::Gte(path, value) => {
            compile_range(plan, path, value, ">=", bindings)
        }
        FilterExpr::Lt(path, value) => {
            compile_range(plan, path, value, "<", bindings)
        }
        FilterExpr::Lte(path, value) => {
            compile_range(plan, path, value, "<=", bindings)
        }
        FilterExpr::Contains(path, value) => {
            let field = planned_field(plan, path)?;
            let sql = field_sql(field, bindings);
            let encoded = serde_json::to_value(vec![value.clone()])?;
            let value = bindings.push(PostgresBindValue::Json(encoded));
            Ok(format!(
                "COALESCE({} @> {value}::jsonb, FALSE)",
                sql.list_values
            ))
        }
        FilterExpr::IsNull(path, expected_null) => {
            let field = planned_field(plan, path)?;
            let sql = field_sql(field, bindings);
            if *expected_null {
                Ok(format!("({})", sql.null_predicate))
            } else {
                Ok(format!("NOT ({})", sql.null_predicate))
            }
        }
    }
}

fn compile_logical(
    plan: &ExecutableQueryPlan,
    children: &[FilterExpr],
    operator: &str,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let children = children
        .iter()
        .map(|child| compile_filter(plan, child, bindings))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("({})", children.join(&format!(" {operator} "))))
}

fn compile_range(
    plan: &ExecutableQueryPlan,
    path: &FieldPath,
    value: &IndexValue,
    operator: &str,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let field = planned_field(plan, path)?;
    let sql = field_sql(field, bindings);
    let value = bindings.push(scalar_bind(path, value)?);
    Ok(format!(
        "COALESCE({} {operator} {value}, FALSE)",
        sql.scalar
    ))
}

fn compile_keyset(
    plan: &ExecutableQueryPlan,
    cursor: &IndexCursor,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    let mut equalities = Vec::new();
    let mut disjuncts = Vec::new();
    for (order, cursor_value) in plan.order_by.iter().zip(&cursor.order_values) {
        let sql = field_sql(&order.field, bindings);
        let (equal, after) = cursor_field_predicates(
            &order.field.path,
            &sql,
            order.direction,
            cursor_value,
            bindings,
        )?;
        disjuncts.push(conjunction(&equalities, &after));
        equalities.push(equal);
    }

    let entity_id = bindings.push(PostgresBindValue::Uuid(cursor.entity_id));
    let root_after = format!(
        "{}.entity_id > {entity_id}",
        quote_identifier(&plan.root_alias)
    );
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
    };
    Ok((equal, after))
}

fn conjunction(prefix: &[String], final_predicate: &str) -> String {
    let mut predicates = prefix.to_vec();
    predicates.push(final_predicate.to_owned());
    format!("({})", predicates.join(" AND "))
}

fn compile_order(plan: &ExecutableQueryPlan, bindings: &mut Bindings) -> String {
    let mut terms = plan
        .order_by
        .iter()
        .map(|order| {
            let sql = field_sql(&order.field, bindings);
            match order.direction {
                OrderDirection::Asc => format!("{} ASC NULLS LAST", sql.scalar),
                OrderDirection::Desc => format!("{} DESC NULLS FIRST", sql.scalar),
            }
        })
        .collect::<Vec<_>>();
    terms.push(format!(
        "{}.entity_id ASC",
        quote_identifier(&plan.root_alias)
    ));
    format!("ORDER BY {}", terms.join(", "))
}

fn compile_pagination(
    pagination: &Pagination,
    bindings: &mut Bindings,
) -> Result<String, PostgresQueryCompileError> {
    match pagination {
        Pagination::Cursor { first, .. } => {
            let limit = bindings.push(PostgresBindValue::Integer(i64::from(*first)));
            Ok(format!("LIMIT {limit}"))
        }
        Pagination::Offset { limit, offset } => {
            let limit = bindings.push(PostgresBindValue::Integer(i64::from(*limit)));
            let offset_value = i64::try_from(*offset)
                .map_err(|_| PostgresQueryCompileError::OffsetOutOfRange(*offset))?;
            let offset = bindings.push(PostgresBindValue::Integer(offset_value));
            Ok(format!("LIMIT {limit} OFFSET {offset}"))
        }
    }
}

fn compile_exact_count(
    plan: &ExecutableQueryPlan,
) -> Result<CompiledPostgresCount, PostgresQueryCompileError> {
    let mut bindings = Bindings::default();
    let base = compile_base(plan, &mut bindings);
    let mut predicates = base.predicates;
    if let Some(filter) = &plan.filter {
        predicates.push(compile_filter(plan, filter, &mut bindings)?);
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

fn planned_field<'a>(
    plan: &'a ExecutableQueryPlan,
    path: &FieldPath,
) -> Result<&'a PlannedField, PostgresQueryCompileError> {
    plan.field(path)
        .ok_or_else(|| PostgresQueryCompileError::MissingFieldPlan(path.clone()))
}

struct FieldSql {
    raw: String,
    scalar: String,
    list_values: String,
    null_predicate: String,
}

fn field_sql(field: &PlannedField, bindings: &mut Bindings) -> FieldSql {
    let relation_alias = quote_identifier(&field.relation_alias);
    let field_name = bindings.push(PostgresBindValue::Text(
        field.path.field().as_str().to_owned(),
    ));
    let raw = format!(
        "jsonb_extract_path({relation_alias}.payload, {field_name}::text)"
    );
    let scalar_text = format!(
        "jsonb_extract_path_text({relation_alias}.payload, {field_name}::text, 'value')"
    );
    let type_text = format!(
        "jsonb_extract_path_text({relation_alias}.payload, {field_name}::text, 'type')"
    );
    let scalar = match field.value_type {
        IndexValueType::Boolean => format!("({scalar_text})::boolean"),
        IndexValueType::Integer => format!("({scalar_text})::bigint"),
        IndexValueType::Decimal => format!("({scalar_text})::numeric"),
        IndexValueType::String => format!("({scalar_text} COLLATE \"C\")"),
        IndexValueType::Uuid => format!("({scalar_text})::uuid"),
        IndexValueType::Timestamp => format!("({scalar_text})::timestamptz"),
    };
    FieldSql {
        raw,
        scalar,
        list_values: format!(
            "jsonb_extract_path({relation_alias}.payload, {field_name}::text, 'value')"
        ),
        null_predicate: format!("{type_text} IS NULL OR {type_text} = 'null'"),
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
        IndexValue::Null | IndexValue::List(_) => Err(
            PostgresQueryCompileError::InvalidScalarValue(path.clone()),
        ),
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

fn push_identity_column(
    select: &mut Vec<String>,
    columns: &mut Vec<CompiledQueryColumn>,
    relation_alias: &str,
    relation_alias_q: &str,
) {
    let output_alias = format!("__{relation_alias}_entity_id");
    select.push(format!(
        "{relation_alias_q}.entity_id AS {}",
        quote_identifier(&output_alias)
    ));
    columns.push(CompiledQueryColumn::EntityId {
        output_alias,
        relation_alias: relation_alias.to_owned(),
    });
}
