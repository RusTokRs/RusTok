//! Storefront product specifications: the storefront-safe slice of a product's attribute values.
//!
//! The storefront detail contract stops at translations, variants and prices, so a cataloguer fills
//! typed EAV values that no shopper ever sees. This projection resolves them for display:
//!
//! * only attributes safe for the storefront are returned — the attribute is not archived, its
//!   scope is a product scope (`product` or `both`), the storefront flag is on either on the
//!   definition or on the category binding of the product's effective form (the binding wins, the
//!   same merge rule the admin form uses), the binding is not disabled, and the attribute is not a
//!   JSON payload (JSON has no display projection, exactly like it has no filter projection);
//! * labels and option labels are resolved by the owner with the requested locale first and the
//!   fallback locale second, and the attribute code / option code is the last resort;
//! * every value is already formatted for display, including the localized text of localized
//!   attributes, so a storefront never joins `product_attribute*` tables, never resolves an option
//!   id and never guesses a locale.
//!
//! Values detached from the product are dropped; boolean values keep the `true`/`false`
//! vocabulary so each storefront renders its own localized yes/no copy. One row per attribute is a
//! storage invariant (`uq_product_attribute_values`), so the projection is bounded by the product's
//! own data and needs no pagination.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};
use uuid::Uuid;

use crate::dto::{StorefrontProductAttributeResponse, StorefrontProductAttributeValueResponse};
use crate::error::CommerceResult;
use crate::services::catalog_schema::AttributeValueType;
use crate::services::catalog_schema_service::ProductCatalogSchemaService;

use super::facets::locale_candidates;
use super::helpers::sql_placeholder;

#[derive(Debug, FromQueryResult)]
struct StorefrontAttributeValueRow {
    id: Uuid,
    attribute_id: Uuid,
    code: String,
    value_type: String,
    is_localized: bool,
    position: i32,
    show_on_storefront: bool,
    value_text: Option<String>,
    value_integer: Option<i64>,
    value_decimal: Option<Decimal>,
    value_boolean: Option<bool>,
    value_date: Option<NaiveDate>,
    value_datetime: Option<DateTime<Utc>>,
}

#[derive(Debug, FromQueryResult)]
struct StorefrontAttributeLabelRow {
    attribute_id: Uuid,
    locale: String,
    label: String,
}

#[derive(Debug, FromQueryResult)]
struct StorefrontAttributeValueTextRow {
    value_id: Uuid,
    locale: String,
    value_text: Option<String>,
}

#[derive(Debug, FromQueryResult)]
struct StorefrontAttributeValueOptionRow {
    value_id: Uuid,
    option_id: Uuid,
    option_code: String,
    option_position: i32,
}

#[derive(Debug, FromQueryResult)]
struct StorefrontAttributeOptionLabelRow {
    option_id: Uuid,
    locale: String,
    label: String,
}

/// One attribute of the product with the display order and visibility the owner resolved for it.
struct AttributeSlice {
    row: StorefrontAttributeValueRow,
    /// Effective storefront flag: the category binding override when present, else the definition.
    show_on_storefront: bool,
    /// Sort key: the category binding position when the attribute is part of the effective form,
    /// else the definition position; ties fall back to the attribute code.
    order: (i32, String),
}

/// Loads the storefront-safe attributes of one product.
///
/// The function is deliberately a plain read over the owner's tables: it needs the effective form
/// of the product (for the category-level storefront override and the display order) and the value
/// rows of the product itself, nothing else.
pub(super) async fn load_storefront_product_attributes(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &str,
    fallback_locale: &str,
) -> CommerceResult<Vec<StorefrontProductAttributeResponse>> {
    load_storefront_product_attributes_in(db, tenant_id, product_id, locale, fallback_locale).await
}

/// Connection-generic form: the storefront detail read already runs inside one transaction on the
/// separate-process runtime, so the projection must be usable with any connection.
pub(super) async fn load_storefront_product_attributes_in<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &str,
    fallback_locale: &str,
) -> CommerceResult<Vec<StorefrontProductAttributeResponse>>
where
    C: ConnectionTrait,
{
    let candidates = locale_candidates(locale.trim(), fallback_locale.trim());
    let rows = load_value_rows(db, tenant_id, product_id).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let form_overrides = load_form_overrides(db, tenant_id, product_id).await?;
    let mut slices = rows
        .into_iter()
        .map(|row| {
            let override_entry = form_overrides.get(&row.attribute_id);
            AttributeSlice {
                show_on_storefront: override_entry
                    .and_then(|entry| entry.show_on_storefront)
                    .unwrap_or(row.show_on_storefront),
                order: override_entry
                    .map(|entry| (entry.position, row.code.clone()))
                    .unwrap_or((row.position, row.code.clone())),
                row,
            }
        })
        .filter(|slice| slice.show_on_storefront)
        .collect::<Vec<_>>();
    slices.sort_by(|left, right| left.order.cmp(&right.order));
    if slices.is_empty() {
        return Ok(Vec::new());
    }

    let value_ids = slices.iter().map(|slice| slice.row.id).collect::<Vec<_>>();
    let attribute_ids = slices
        .iter()
        .map(|slice| slice.row.attribute_id)
        .collect::<Vec<_>>();
    let (labels, localized_texts, option_rows) = tokio::try_join!(
        load_attribute_labels(db, &attribute_ids, &candidates),
        load_localized_value_texts(db, &value_ids, &candidates),
        load_value_options(db, &value_ids),
    )?;
    let option_ids = option_rows
        .values()
        .flatten()
        .map(|option| option.option_id)
        .collect::<Vec<_>>();
    let option_labels = load_option_labels(db, &option_ids, &candidates).await?;

    let mut attributes = Vec::with_capacity(slices.len());
    for slice in slices {
        let row = slice.row;
        let value_type = match AttributeValueType::from_storage(&row.value_type) {
            Ok(value_type) => value_type,
            Err(error) => {
                tracing::warn!(
                    attribute_id = %row.attribute_id,
                    attribute_code = row.code.as_str(),
                    value_type = row.value_type.as_str(),
                    error = ?error,
                    "skipping storefront attribute with an unsupported stored value type"
                );
                continue;
            }
        };
        if matches!(value_type, AttributeValueType::Json) {
            continue;
        }

        let values = match value_type {
            AttributeValueType::Select | AttributeValueType::Multiselect => option_rows
                .get(&row.id)
                .map(|options| {
                    options
                        .iter()
                        .map(|option| {
                            let text = option_labels
                                .get(&option.option_id)
                                .cloned()
                                .unwrap_or_else(|| option.option_code.clone());
                            StorefrontProductAttributeValueResponse { text }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            _ => match format_scalar_value(&row, value_type, &localized_texts) {
                Some(text) => vec![StorefrontProductAttributeValueResponse { text }],
                None => Vec::new(),
            },
        };
        if values.is_empty() {
            continue;
        }

        attributes.push(StorefrontProductAttributeResponse {
            code: row.code.clone(),
            label: labels
                .get(&row.attribute_id)
                .cloned()
                .unwrap_or_else(|| row.code.clone()),
            value_type: row.value_type,
            is_localized: row.is_localized,
            values,
        });
    }

    Ok(attributes)
}

/// The product's own value rows, already narrowed to product-scope, non-archived attributes.
///
/// The storefront flag is *not* filtered here: the category binding of the effective form may
/// override it in either direction, and that merge happens once, in Rust, where both sides are
/// available.
async fn load_value_rows<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<Vec<StorefrontAttributeValueRow>>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let tenant = sql_placeholder(backend, 1);
    let product = sql_placeholder(backend, 2);
    StorefrontAttributeValueRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            r#"
            SELECT
                pav.id,
                pav.attribute_id,
                pa.code,
                pa.value_type,
                pa.is_localized,
                pa.position,
                pa.show_on_storefront,
                pav.value_text,
                pav.value_integer,
                pav.value_decimal,
                pav.value_boolean,
                pav.value_date,
                pav.value_datetime
            FROM product_attribute_values pav
            JOIN product_attributes pa
              ON pa.id = pav.attribute_id AND pa.tenant_id = pav.tenant_id
            WHERE pav.tenant_id = {tenant}
              AND pav.product_id = {product}
              AND pav.detached_at IS NULL
              AND pa.archived_at IS NULL
              AND pa.scope IN ('product', 'both')
            ORDER BY pa.position, pa.code
            "#
        ),
        vec![tenant_id.into(), product_id.into()],
    ))
    .all(db)
    .await
    .map_err(Into::into)
}

/// Category-level visibility of the product's effective form, keyed by attribute id.
struct FormOverride {
    /// `Some(value)` when the category binding overrides the definition's storefront flag.
    show_on_storefront: Option<bool>,
    position: i32,
}

async fn load_form_overrides<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<HashMap<Uuid, FormOverride>>
where
    C: ConnectionTrait,
{
    let Some(form) =
        ProductCatalogSchemaService::load_effective_form_for_product_in(db, tenant_id, product_id)
            .await?
    else {
        return Ok(HashMap::new());
    };

    let mut overrides = HashMap::with_capacity(form.attributes.len());
    for binding in form.attributes {
        if binding.is_disabled {
            // A disabled binding means "not part of this product's form": its value is detached
            // even when the row still exists.
            overrides.insert(
                binding.attribute_id,
                FormOverride {
                    show_on_storefront: Some(false),
                    position: binding.position,
                },
            );
            continue;
        }
        overrides.insert(
            binding.attribute_id,
            FormOverride {
                show_on_storefront: binding.visibility_overrides.show_on_storefront,
                position: binding.position,
            },
        );
    }
    Ok(overrides)
}

async fn load_attribute_labels<C>(
    db: &C,
    attribute_ids: &[Uuid],
    candidates: &[String],
) -> CommerceResult<HashMap<Uuid, String>>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let mut values = Vec::<sea_orm::Value>::new();
    let ids = attribute_ids
        .iter()
        .map(|attribute_id| {
            values.push((*attribute_id).into());
            sql_placeholder(backend, values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut label_values = values;
    let locales = candidates
        .iter()
        .map(|candidate| {
            label_values.push(candidate.clone().into());
            sql_placeholder(backend, label_values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut ranked = HashMap::<Uuid, BTreeMap<usize, String>>::new();
    if !candidates.is_empty() {
        let rows = StorefrontAttributeLabelRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            format!(
                r#"
                SELECT attribute_id, locale, label
                FROM product_attribute_translations
                WHERE attribute_id IN ({ids})
                  AND locale IN ({locales})
                "#
            ),
            label_values,
        ))
        .all(db)
        .await?;
        for row in rows {
            let Some(rank) = candidates.iter().position(|candidate| candidate == &row.locale) else {
                continue;
            };
            let label = row.label;
            if label.trim().is_empty() {
                continue;
            }
            ranked.entry(row.attribute_id).or_default().insert(rank, label);
        }
    }

    Ok(ranked
        .into_iter()
        .map(|(attribute_id, by_locale)| {
            let label = by_locale
                .into_values()
                .next()
                .expect("a ranked label map is never empty");
            (attribute_id, label)
        })
        .collect())
}

async fn load_localized_value_texts<C>(
    db: &C,
    value_ids: &[Uuid],
    candidates: &[String],
) -> CommerceResult<HashMap<Uuid, String>>
where
    C: ConnectionTrait,
{
    if candidates.is_empty() {
        return Ok(HashMap::new());
    }
    let backend = db.get_database_backend();
    let mut values = Vec::<sea_orm::Value>::new();
    let ids = value_ids
        .iter()
        .map(|value_id| {
            values.push((*value_id).into());
            sql_placeholder(backend, values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut label_values = values;
    let locales = candidates
        .iter()
        .map(|candidate| {
            label_values.push(candidate.clone().into());
            sql_placeholder(backend, label_values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut ranked = HashMap::<Uuid, BTreeMap<usize, String>>::new();
    let rows = StorefrontAttributeValueTextRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            r#"
            SELECT value_id, locale, value_text
            FROM product_attribute_value_translations
            WHERE value_id IN ({ids})
              AND locale IN ({locales})
            "#
        ),
        label_values,
    ))
    .all(db)
    .await?;
    for row in rows {
        let Some(rank) = candidates.iter().position(|candidate| candidate == &row.locale) else {
            continue;
        };
        let Some(text) = row.value_text.filter(|text| !text.trim().is_empty()) else {
            continue;
        };
        ranked.entry(row.value_id).or_default().insert(rank, text);
    }

    Ok(ranked
        .into_iter()
        .map(|(value_id, by_locale)| {
            let text = by_locale
                .into_values()
                .next()
                .expect("a ranked value map is never empty");
            (value_id, text)
        })
        .collect())
}

async fn load_value_options<C>(
    db: &C,
    value_ids: &[Uuid],
) -> CommerceResult<HashMap<Uuid, Vec<StorefrontAttributeValueOptionRow>>>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let mut values = Vec::<sea_orm::Value>::new();
    let ids = value_ids
        .iter()
        .map(|value_id| {
            values.push((*value_id).into());
            sql_placeholder(backend, values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");

    let rows = StorefrontAttributeValueOptionRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            r#"
            SELECT
                pavo.value_id,
                pavo.option_id,
                pao.code AS option_code,
                pao.position AS option_position
            FROM product_attribute_value_options pavo
            JOIN product_attribute_options pao ON pao.id = pavo.option_id
            WHERE pavo.value_id IN ({ids})
            ORDER BY pavo.value_id, pao.position, pao.code
            "#
        ),
        values,
    ))
    .all(db)
    .await?;

    let mut by_value = HashMap::<Uuid, Vec<StorefrontAttributeValueOptionRow>>::new();
    for row in rows {
        by_value.entry(row.value_id).or_default().push(row);
    }
    Ok(by_value)
}

async fn load_option_labels<C>(
    db: &C,
    option_ids: &[Uuid],
    candidates: &[String],
) -> CommerceResult<HashMap<Uuid, String>>
where
    C: ConnectionTrait,
{
    if option_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut unique = Vec::with_capacity(option_ids.len());
    let mut seen = HashSet::with_capacity(option_ids.len());
    for option_id in option_ids {
        if seen.insert(*option_id) {
            unique.push(*option_id);
        }
    }

    let backend = db.get_database_backend();
    let mut values = Vec::<sea_orm::Value>::new();
    let ids = unique
        .iter()
        .map(|option_id| {
            values.push((*option_id).into());
            sql_placeholder(backend, values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut label_values = values;
    let locales = candidates
        .iter()
        .map(|candidate| {
            label_values.push(candidate.clone().into());
            sql_placeholder(backend, label_values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut ranked = HashMap::<Uuid, BTreeMap<usize, String>>::new();
    if !candidates.is_empty() {
        let rows = StorefrontAttributeOptionLabelRow::find_by_statement(
            Statement::from_sql_and_values(
                backend,
                format!(
                    r#"
                    SELECT option_id, locale, label
                    FROM product_attribute_option_translations
                    WHERE option_id IN ({ids})
                      AND locale IN ({locales})
                    "#
                ),
                label_values,
            ),
        )
        .all(db)
        .await?;
        for row in rows {
            let Some(rank) = candidates.iter().position(|candidate| candidate == &row.locale) else {
                continue;
            };
            ranked.entry(row.option_id).or_default().insert(rank, row.label);
        }
    }

    Ok(ranked
        .into_iter()
        .map(|(option_id, by_locale)| {
            let label = by_locale
                .into_values()
                .next()
                .expect("a ranked option map is never empty");
            (option_id, label)
        })
        .collect())
}

/// Formats one scalar (non-dictionary) value for display.
///
/// Returns `None` for a stored value that does not match its declared type: a storefront read
/// must not fail because one row is inconsistent, so the value is dropped and logged instead.
fn format_scalar_value(
    row: &StorefrontAttributeValueRow,
    value_type: AttributeValueType,
    localized_texts: &HashMap<Uuid, String>,
) -> Option<String> {
    let missing = |expected: &str| {
        tracing::warn!(
            attribute_id = %row.attribute_id,
            attribute_code = row.code.as_str(),
            value_type = row.value_type.as_str(),
            expected = expected,
            "skipping storefront attribute value that does not match its stored type"
        );
        None
    };

    match value_type {
        AttributeValueType::Text
        | AttributeValueType::Textarea
        | AttributeValueType::Richtext
            if row.is_localized =>
        {
            localized_texts
                .get(&row.id)
                .cloned()
                .or_else(|| row.value_text.clone().filter(|text| !text.trim().is_empty()))
                .or_else(|| missing("localized text"))
        }
        AttributeValueType::Text
        | AttributeValueType::Textarea
        | AttributeValueType::Richtext => row
            .value_text
            .clone()
            .filter(|text| !text.trim().is_empty())
            .or_else(|| missing("text")),
        AttributeValueType::Integer => row
            .value_integer
            .map(|value| value.to_string())
            .or_else(|| missing("integer")),
        AttributeValueType::Decimal => row
            .value_decimal
            .map(|value| value.normalize().to_string())
            .or_else(|| missing("decimal")),
        AttributeValueType::Boolean => row
            .value_boolean
            .map(|value| if value { "true" } else { "false" }.to_string())
            .or_else(|| missing("boolean")),
        AttributeValueType::Date => row
            .value_date
            .map(|value| value.format("%Y-%m-%d").to_string())
            .or_else(|| missing("date")),
        AttributeValueType::Datetime => row
            .value_datetime
            .map(|value| value.to_rfc3339())
            .or_else(|| missing("datetime")),
        // Dictionary types are resolved through the option dictionary; JSON never reaches here.
        AttributeValueType::Select
        | AttributeValueType::Multiselect
        | AttributeValueType::Json => None,
    }
}
