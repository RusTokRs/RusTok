use sea_orm::{
    Condition, ConnectionTrait, DatabaseConnection, DbBackend, FromQueryResult, Statement,
    sea_query::Expr,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};
use crate::services::catalog_attribute_terms::{
    ProductAttributeFilterValue, parse_product_attribute_filter_value,
};
use crate::services::catalog_schema::AttributeValueType;

use super::ProductAttributeFilter;
use super::types::validate_product_attribute_filters;

#[derive(Debug, FromQueryResult)]
struct CatalogAttributeFilterDefinitionRow {
    id: Uuid,
    code: String,
    value_type: String,
    is_localized: bool,
}

pub(super) async fn load_catalog_attribute_filter_conditions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    fallback_locale: &str,
    filters: &[ProductAttributeFilter],
) -> CommerceResult<Vec<Condition>> {
    validate_product_attribute_filters(filters)?;
    if filters.is_empty() {
        return Ok(Vec::new());
    }

    let backend = db.get_database_backend();
    let mut values = vec![tenant_id.into()];
    let placeholders = filters
        .iter()
        .enumerate()
        .map(|(index, filter)| {
            values.push(filter.code.to_ascii_lowercase().into());
            sql_placeholder(backend, index + 2)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let tenant_placeholder = sql_placeholder(backend, 1);
    let definitions =
        CatalogAttributeFilterDefinitionRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            format!(
                r#"
                SELECT id, code, value_type, is_localized
                FROM product_attributes
                WHERE tenant_id = {tenant_placeholder}
                  AND archived_at IS NULL
                  AND is_filterable = TRUE
                  AND scope IN ('product', 'both')
                  AND LOWER(code) IN ({placeholders})
                "#
            ),
            values,
        ))
        .all(db)
        .await?
        .into_iter()
        .map(|definition| (definition.code.to_ascii_lowercase(), definition))
        .collect::<HashMap<_, _>>();

    let mut conditions = Vec::with_capacity(filters.len());
    for filter in filters {
        let definition = definitions
            .get(&filter.code.to_ascii_lowercase())
            .ok_or_else(|| {
                CommerceError::Validation(format!(
                    "attribute {} is not available as a product filter",
                    filter.code
                ))
            })?;
        let value_type =
            AttributeValueType::from_storage(definition.value_type.as_str()).map_err(|_| {
                CommerceError::Validation(format!(
                    "attribute {} has an unsupported stored value type",
                    definition.code
                ))
            })?;
        conditions.push(build_attribute_filter_condition(
            backend,
            tenant_id,
            definition,
            value_type,
            filter.value.as_str(),
            locale,
            fallback_locale,
        )?);
    }
    Ok(conditions)
}

fn build_attribute_filter_condition(
    backend: DbBackend,
    tenant_id: Uuid,
    definition: &CatalogAttributeFilterDefinitionRow,
    value_type: AttributeValueType,
    raw_value: &str,
    locale: &str,
    fallback_locale: &str,
) -> CommerceResult<Condition> {
    let value =
        parse_product_attribute_filter_value(definition.code.as_str(), value_type, raw_value)?;
    let condition = match value {
        ProductAttributeFilterValue::Text(value) if definition.is_localized => {
            localized_text_condition(
                backend,
                tenant_id,
                definition.id,
                locale.trim(),
                fallback_locale.trim(),
                value.as_str(),
            )
        }
        ProductAttributeFilterValue::Text(value) => custom_condition(
            backend,
            r#"
            EXISTS (
                SELECT 1
                FROM product_attribute_values pav
                WHERE pav.product_id = products.id
                  AND pav.tenant_id = {p1}
                  AND pav.attribute_id = {p2}
                  AND pav.detached_at IS NULL
                  AND pav.value_text = {p3}
            )
            "#,
            vec![tenant_id.into(), definition.id.into(), value.into()],
        ),
        ProductAttributeFilterValue::Integer(value) => scalar_condition(
            backend,
            tenant_id,
            definition.id,
            "value_integer",
            value.into(),
        ),
        ProductAttributeFilterValue::Decimal(value) => scalar_condition(
            backend,
            tenant_id,
            definition.id,
            "value_decimal",
            value.into(),
        ),
        ProductAttributeFilterValue::Boolean(value) => scalar_condition(
            backend,
            tenant_id,
            definition.id,
            "value_boolean",
            value.into(),
        ),
        ProductAttributeFilterValue::Date(value) => scalar_condition(
            backend,
            tenant_id,
            definition.id,
            "value_date",
            value.into(),
        ),
        ProductAttributeFilterValue::Datetime(value) => scalar_condition(
            backend,
            tenant_id,
            definition.id,
            "value_datetime",
            value.into(),
        ),
        ProductAttributeFilterValue::Option(value) => {
            option_condition(backend, tenant_id, definition.id, value.as_str())
        }
    };
    Ok(condition)
}

fn localized_text_condition(
    backend: DbBackend,
    tenant_id: Uuid,
    attribute_id: Uuid,
    locale: &str,
    fallback_locale: &str,
    raw_value: &str,
) -> Condition {
    if locale == fallback_locale {
        return custom_condition(
            backend,
            r#"
            EXISTS (
                SELECT 1
                FROM product_attribute_values pav
                JOIN product_attribute_value_translations pavt
                  ON pavt.value_id = pav.id
                WHERE pav.product_id = products.id
                  AND pav.tenant_id = {p1}
                  AND pav.attribute_id = {p2}
                  AND pav.detached_at IS NULL
                  AND pavt.locale = {p3}
                  AND pavt.value_text = {p4}
            )
            "#,
            vec![
                tenant_id.into(),
                attribute_id.into(),
                locale.into(),
                raw_value.into(),
            ],
        );
    }

    custom_condition(
        backend,
        r#"
        EXISTS (
            SELECT 1
            FROM product_attribute_values pav
            WHERE pav.product_id = products.id
              AND pav.tenant_id = {p1}
              AND pav.attribute_id = {p2}
              AND pav.detached_at IS NULL
              AND (
                  EXISTS (
                      SELECT 1
                      FROM product_attribute_value_translations requested
                      WHERE requested.value_id = pav.id
                        AND requested.locale = {p3}
                        AND requested.value_text = {p4}
                  )
                  OR (
                      NOT EXISTS (
                          SELECT 1
                          FROM product_attribute_value_translations requested_any
                          WHERE requested_any.value_id = pav.id
                            AND requested_any.locale = {p3}
                      )
                      AND EXISTS (
                          SELECT 1
                          FROM product_attribute_value_translations fallback_value
                          WHERE fallback_value.value_id = pav.id
                            AND fallback_value.locale = {p5}
                            AND fallback_value.value_text = {p4}
                      )
                  )
              )
        )
        "#,
        vec![
            tenant_id.into(),
            attribute_id.into(),
            locale.into(),
            raw_value.into(),
            fallback_locale.into(),
        ],
    )
}

fn scalar_condition(
    backend: DbBackend,
    tenant_id: Uuid,
    attribute_id: Uuid,
    column: &str,
    value: sea_orm::Value,
) -> Condition {
    custom_condition(
        backend,
        format!(
            r#"
            EXISTS (
                SELECT 1
                FROM product_attribute_values pav
                WHERE pav.product_id = products.id
                  AND pav.tenant_id = {{p1}}
                  AND pav.attribute_id = {{p2}}
                  AND pav.detached_at IS NULL
                  AND pav.{column} = {{p3}}
            )
            "#
        )
        .as_str(),
        vec![tenant_id.into(), attribute_id.into(), value],
    )
}

fn option_condition(
    backend: DbBackend,
    tenant_id: Uuid,
    attribute_id: Uuid,
    raw_value: &str,
) -> Condition {
    if let Ok(option_id) = Uuid::parse_str(raw_value) {
        return custom_condition(
            backend,
            r#"
            EXISTS (
                SELECT 1
                FROM product_attribute_values pav
                JOIN product_attribute_value_options pavo ON pavo.value_id = pav.id
                WHERE pav.product_id = products.id
                  AND pav.tenant_id = {p1}
                  AND pav.attribute_id = {p2}
                  AND pav.detached_at IS NULL
                  AND pavo.option_id = {p3}
            )
            "#,
            vec![tenant_id.into(), attribute_id.into(), option_id.into()],
        );
    }
    custom_condition(
        backend,
        r#"
        EXISTS (
            SELECT 1
            FROM product_attribute_values pav
            JOIN product_attribute_value_options pavo ON pavo.value_id = pav.id
            JOIN product_attribute_options pao
              ON pao.id = pavo.option_id
             AND pao.tenant_id = pav.tenant_id
            WHERE pav.product_id = products.id
              AND pav.tenant_id = {p1}
              AND pav.attribute_id = {p2}
              AND pav.detached_at IS NULL
              AND pao.archived_at IS NULL
              AND pao.code = {p3}
        )
        "#,
        vec![tenant_id.into(), attribute_id.into(), raw_value.into()],
    )
}

fn custom_condition(
    backend: DbBackend,
    sql_template: &str,
    values: Vec<sea_orm::Value>,
) -> Condition {
    let sql = values
        .iter()
        .enumerate()
        .fold(sql_template.to_string(), |sql, (index, _)| {
            sql.replace(
                format!("{{p{}}}", index + 1).as_str(),
                sql_placeholder(backend, index + 1).as_str(),
            )
        });
    Condition::all().add(Expr::cust_with_values(sql, values))
}

fn sql_placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Sqlite => "?".to_string(),
        _ => format!("${index}"),
    }
}
