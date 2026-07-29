use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    Condition, DatabaseConnection, DbBackend, FromQueryResult, Statement,
    sea_query::Expr,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};
use crate::services::catalog_schema::AttributeValueType;

use super::ProductAttributeFilter;

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
    if filters.is_empty() {
        return Ok(Vec::new());
    }

    let mut values = vec![tenant_id.into()];
    let placeholders = filters
        .iter()
        .enumerate()
        .map(|(index, filter)| {
            values.push(filter.code.to_ascii_lowercase().into());
            format!("${}", index + 2)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let definitions = CatalogAttributeFilterDefinitionRow::find_by_statement(
        Statement::from_sql_and_values(
            db.get_database_backend(),
            format!(
                r#"
                SELECT id, code, value_type, is_localized
                FROM product_attributes
                WHERE tenant_id = $1
                  AND archived_at IS NULL
                  AND is_filterable = TRUE
                  AND scope IN ('product', 'both')
                  AND LOWER(code) IN ({placeholders})
                "#
            ),
            values,
        ),
    )
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
        let value_type = AttributeValueType::from_storage(definition.value_type.as_str())
            .map_err(|_| {
                CommerceError::Validation(format!(
                    "attribute {} has an unsupported stored value type",
                    definition.code
                ))
            })?;
        conditions.push(build_attribute_filter_condition(
            db.get_database_backend(),
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
    let condition = match value_type {
        AttributeValueType::Text
        | AttributeValueType::Textarea
        | AttributeValueType::Richtext
            if definition.is_localized =>
        {
            custom_condition(
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
                      AND pavt.locale IN ({p3}, {p4})
                      AND pavt.value_text = {p5}
                )
                "#,
                vec![
                    tenant_id.into(),
                    definition.id.into(),
                    locale.trim().into(),
                    fallback_locale.trim().into(),
                    raw_value.into(),
                ],
            )
        }
        AttributeValueType::Text
        | AttributeValueType::Textarea
        | AttributeValueType::Richtext => custom_condition(
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
            vec![tenant_id.into(), definition.id.into(), raw_value.into()],
        ),
        AttributeValueType::Integer => {
            let value = raw_value.parse::<i64>().map_err(|_| {
                invalid_typed_value(definition.code.as_str(), "integer", raw_value)
            })?;
            scalar_condition(backend, tenant_id, definition.id, "value_integer", value.into())
        }
        AttributeValueType::Decimal => {
            let value = raw_value.parse::<Decimal>().map_err(|_| {
                invalid_typed_value(definition.code.as_str(), "decimal", raw_value)
            })?;
            scalar_condition(backend, tenant_id, definition.id, "value_decimal", value.into())
        }
        AttributeValueType::Boolean => {
            let value = match raw_value.to_ascii_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => {
                    return Err(invalid_typed_value(
                        definition.code.as_str(),
                        "boolean",
                        raw_value,
                    ));
                }
            };
            scalar_condition(backend, tenant_id, definition.id, "value_boolean", value.into())
        }
        AttributeValueType::Date => {
            let value = NaiveDate::parse_from_str(raw_value, "%Y-%m-%d").map_err(|_| {
                invalid_typed_value(definition.code.as_str(), "date (YYYY-MM-DD)", raw_value)
            })?;
            scalar_condition(backend, tenant_id, definition.id, "value_date", value.into())
        }
        AttributeValueType::Datetime => {
            let value = DateTime::parse_from_rfc3339(raw_value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|_| {
                    invalid_typed_value(definition.code.as_str(), "RFC3339 datetime", raw_value)
                })?;
            scalar_condition(
                backend,
                tenant_id,
                definition.id,
                "value_datetime",
                value.into(),
            )
        }
        AttributeValueType::Select | AttributeValueType::Multiselect => {
            option_condition(backend, tenant_id, definition.id, raw_value)
        }
        AttributeValueType::Json => {
            return Err(CommerceError::Validation(format!(
                "attribute {} uses json and cannot be used in attribute_filters",
                definition.code
            )));
        }
    };
    Ok(condition)
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
            let placeholder = match backend {
                DbBackend::Sqlite => "?".to_string(),
                _ => format!("${}", index + 1),
            };
            sql.replace(format!("{{p{}}}", index + 1).as_str(), placeholder.as_str())
        });
    Condition::all().add(Expr::cust_with_values(sql, values))
}

fn invalid_typed_value(code: &str, expected: &str, value: &str) -> CommerceError {
    CommerceError::Validation(format!(
        "attribute filter {code} expects {expected}, received `{value}`"
    ))
}
