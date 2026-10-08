//! Storefront catalog facets: per-attribute bucket counts for the current filter set.
//!
//! Facets are computed by the owner so that a consumer never reads Product attribute tables and
//! never re-implements filter semantics. Every bucket count uses exactly the same product filter
//! set as the storefront list (tenant, active and published products, channel visibility,
//! category, search) plus every *other* attribute filter of the current selection. A facet's own
//! filter is excluded, which is the standard "OR within one attribute, AND between attributes"
//! drill-down behaviour.
//!
//! Bucket values are only enumerated for value types whose dictionary is bounded: `select` and
//! `multiselect` buckets are option ids (the same vocabulary `attribute_filters=code=<option id>`
//! accepts), `boolean` buckets are `true`/`false`. Free text and numeric types keep an empty
//! bucket list (`is_enumerable = false`): the UI keeps its value input instead of pretending to
//! enumerate an unbounded domain, and every facet still reports how many products carry a value.

use sea_orm::sea_query::{Alias, Expr, ExprTrait, JoinType, Order, Query};
use sea_orm::{Condition, DatabaseConnection, DbBackend, FromQueryResult, Statement, sea_query};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

use rustok_api::PLATFORM_FALLBACK_LOCALE;

use crate::error::{CommerceError, CommerceResult};
use crate::services::catalog_schema::AttributeValueType;

use super::ProductAttributeFilter;
use super::attribute_filters::load_catalog_attribute_filter_conditions;
use super::helpers::{product_channel_visibility_condition, sql_placeholder};
use super::queries::product_title_search_condition;
use super::types::MAX_ATTRIBUTE_FILTERS;

/// Maximum number of facets one request may ask for; mirrors the attribute-filter limit.
pub const MAX_CATALOG_FACETS: usize = MAX_ATTRIBUTE_FILTERS;
/// Maximum number of bucket values one enumerable facet returns.
pub const MAX_CATALOG_FACET_VALUES: usize = 20;

/// One bucket of an enumerable facet.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StorefrontCatalogFacetValue {
    /// `code=<value>` value for this bucket: an option id for dictionary types, `true`/`false`
    /// for booleans.
    pub value: String,
    pub label: String,
    pub count: u64,
}

/// One storefront facet: an attribute with its bucket counts under the current filter set.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StorefrontCatalogFacet {
    pub code: String,
    pub label: String,
    /// Stored attribute value type, e.g. `select`.
    pub value_type: String,
    pub is_localized: bool,
    /// False when the value domain is unbounded (text, numeric, date): `values` stays empty.
    pub is_enumerable: bool,
    /// True when the bucket list was cut at [`MAX_CATALOG_FACET_VALUES`].
    pub is_truncated: bool,
    pub values: Vec<StorefrontCatalogFacetValue>,
    /// Products matching every filter except this attribute's own filter that carry a value for
    /// this attribute.
    pub total_products: u64,
}

#[derive(Debug, FromQueryResult)]
struct FacetDefinitionRow {
    id: Uuid,
    code: String,
    value_type: String,
    is_localized: bool,
}

#[derive(Debug, FromQueryResult)]
struct FacetAttributeLabelRow {
    attribute_id: Uuid,
    locale: String,
    label: String,
}

#[derive(Debug, FromQueryResult)]
struct FacetOptionLabelRow {
    option_id: Uuid,
    locale: String,
    label: String,
}

#[derive(Debug, FromQueryResult)]
struct FacetOptionCodeRow {
    id: Uuid,
    code: String,
}

#[derive(Debug, FromQueryResult)]
struct FacetOptionBucketRow {
    option_id: Uuid,
    product_count: i64,
}

#[derive(Debug, FromQueryResult)]
struct FacetBooleanBucketRow {
    value_boolean: bool,
    product_count: i64,
}

#[derive(Debug, FromQueryResult)]
struct FacetCountRow {
    product_count: i64,
}

impl super::CatalogService {
    /// Computes storefront facet counts for the requested attribute codes.
    ///
    /// `list_query` carries the current storefront selection; its pagination fields are ignored
    /// because facets describe the whole filtered catalog, not one page.
    #[tracing::instrument(skip(self))]
    pub async fn storefront_catalog_facets(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
        list_query: &super::StorefrontProductListQuery,
        facet_codes: &[String],
    ) -> CommerceResult<Vec<StorefrontCatalogFacet>> {
        let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
        super::types::validate_storefront_product_search(list_query.search.as_deref())?;
        load_storefront_catalog_facets(
            &self.db,
            tenant_id,
            locale,
            fallback_locale,
            public_channel_slug,
            list_query,
            facet_codes,
        )
        .await
    }
}

pub(super) async fn load_storefront_catalog_facets(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    fallback_locale: &str,
    public_channel_slug: Option<&str>,
    list_query: &super::StorefrontProductListQuery,
    facet_codes: &[String],
) -> CommerceResult<Vec<StorefrontCatalogFacet>> {
    let codes = normalize_facet_codes(facet_codes)?;
    if codes.is_empty() {
        return Ok(Vec::new());
    }

    let definitions = load_facet_definitions(db, tenant_id, &codes).await?;
    let labels = load_facet_attribute_labels(db, &definitions, locale, fallback_locale).await?;

    let mut facets = Vec::with_capacity(definitions.len());
    let mut option_ids = Vec::<Uuid>::new();
    let mut option_bucket_slots = Vec::<(usize, Vec<Uuid>)>::new();

    for definition in &definitions {
        let value_type =
            AttributeValueType::from_storage(&definition.value_type).map_err(|_| {
                CommerceError::Validation(format!(
                    "attribute {} has an unsupported stored value type",
                    definition.code
                ))
            })?;
        if !value_type.is_attribute_filterable() {
            return Err(CommerceError::Validation(format!(
                "attribute {} uses {} and cannot be used in attribute_filters",
                definition.code,
                value_type.as_str()
            )));
        }

        let other_filters = list_query
            .attribute_filters
            .iter()
            .filter(|filter| !filter.code.eq_ignore_ascii_case(&definition.code))
            .cloned()
            .collect::<Vec<_>>();
        let other_conditions = load_catalog_attribute_filter_conditions(
            db,
            tenant_id,
            locale,
            fallback_locale,
            other_filters.as_slice(),
        )
        .await?;
        let products_condition = facet_products_condition(
            db.get_database_backend(),
            tenant_id,
            public_channel_slug,
            list_query,
            other_conditions,
        );

        let total_products = load_facet_total(db, &definition.id, &products_condition).await?;

        let is_enumerable = value_type.is_enumerable_facet();
        let mut values = Vec::new();
        let mut is_truncated = false;
        if is_enumerable {
            let buckets = load_facet_buckets(db, definition, &products_condition).await?;
            let mut collected = Vec::with_capacity(buckets.len());
            for (bucket, count) in buckets {
                if collected.len() == MAX_CATALOG_FACET_VALUES {
                    is_truncated = true;
                    break;
                }
                if let Ok(option_id) = Uuid::parse_str(&bucket) {
                    option_ids.push(option_id);
                }
                collected.push((bucket, count));
            }
            let slot = facets.len();
            option_bucket_slots.push((
                slot,
                collected
                    .iter()
                    .filter_map(|(bucket, _)| Uuid::parse_str(bucket).ok())
                    .collect(),
            ));
            values = collected
                .into_iter()
                .map(|(value, count)| StorefrontCatalogFacetValue {
                    // Dictionary buckets get their localized label below; a bucket without a
                    // dictionary entry (boolean) keeps its raw value as the label.
                    label: value.clone(),
                    value,
                    count,
                })
                .collect();
        }

        facets.push(StorefrontCatalogFacet {
            code: definition.code.clone(),
            label: labels
                .get(&definition.id)
                .cloned()
                .unwrap_or_else(|| definition.code.clone()),
            value_type: definition.value_type.clone(),
            is_localized: definition.is_localized,
            is_enumerable,
            is_truncated,
            values,
            total_products,
        });
    }

    if !option_ids.is_empty() {
        let option_labels =
            load_facet_option_labels(db, &option_ids, locale, fallback_locale).await?;
        for (slot, ids) in option_bucket_slots {
            if let Some(facet) = facets.get_mut(slot) {
                for (value, option_id) in facet.values.iter_mut().zip(ids) {
                    if let Some(label) = option_labels.get(&option_id) {
                        value.label = label.clone();
                    }
                }
            }
        }
    }

    Ok(facets)
}

/// Validates, trims and de-duplicates the requested facet codes, keeping the caller's order.
fn normalize_facet_codes(facet_codes: &[String]) -> CommerceResult<Vec<String>> {
    let mut seen = HashSet::with_capacity(facet_codes.len());
    let mut codes = Vec::with_capacity(facet_codes.len());
    for raw in facet_codes {
        let code = raw.trim();
        if code.is_empty() {
            return Err(CommerceError::Validation(
                "facet codes must not be empty".to_string(),
            ));
        }
        if code.chars().count() > 128 {
            return Err(CommerceError::Validation(
                "facet codes must be max 128 characters".to_string(),
            ));
        }
        let normalized = code.to_ascii_lowercase();
        if !seen.insert(normalized) {
            continue;
        }
        if codes.len() == MAX_CATALOG_FACETS {
            return Err(CommerceError::Validation(format!(
                "facets support at most {MAX_CATALOG_FACETS} attributes"
            )));
        }
        codes.push(code.to_string());
    }
    Ok(codes)
}

async fn load_facet_definitions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    codes: &[String],
) -> CommerceResult<Vec<FacetDefinitionRow>> {
    let backend = db.get_database_backend();
    let mut values = vec![tenant_id.into()];
    let placeholders = codes
        .iter()
        .enumerate()
        .map(|(index, code)| {
            values.push(code.to_ascii_lowercase().into());
            sql_placeholder(backend, index + 2)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let tenant_placeholder = sql_placeholder(backend, 1);
    let rows = FacetDefinitionRow::find_by_statement(Statement::from_sql_and_values(
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
    .await?;

    let resolved = rows
        .into_iter()
        .map(|row| (row.code.to_ascii_lowercase(), row))
        .collect::<HashMap<_, _>>();
    let mut definitions = Vec::with_capacity(codes.len());
    for code in codes {
        let definition = resolved.get(&code.to_ascii_lowercase()).ok_or_else(|| {
            CommerceError::Validation(format!(
                "attribute {code} is not available as a product filter"
            ))
        })?;
        definitions.push(FacetDefinitionRow {
            id: definition.id,
            code: definition.code.clone(),
            value_type: definition.value_type.clone(),
            is_localized: definition.is_localized,
        });
    }
    Ok(definitions)
}

/// Loads the facet label of every requested attribute, preferring `facet_label` over `label`,
/// then the requested locale over the fallback locale, then the attribute code.
async fn load_facet_attribute_labels(
    db: &DatabaseConnection,
    definitions: &[FacetDefinitionRow],
    locale: &str,
    fallback_locale: &str,
) -> CommerceResult<HashMap<Uuid, String>> {
    let locale = locale.trim();
    let fallback_locale = fallback_locale.trim();
    let candidates = locale_candidates(locale, fallback_locale);
    if definitions.is_empty() || candidates.is_empty() {
        return Ok(HashMap::new());
    }

    let backend = db.get_database_backend();
    let mut values = Vec::<sea_orm::Value>::new();
    let id_placeholders = definitions
        .iter()
        .map(|definition| {
            values.push(definition.id.into());
            sql_placeholder(backend, values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");
    let locale_placeholders = candidates
        .iter()
        .map(|candidate| {
            values.push(candidate.clone().into());
            sql_placeholder(backend, values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");

    let rows = FacetAttributeLabelRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            r#"
            SELECT attribute_id, locale, COALESCE(facet_label, label) AS label
            FROM product_attribute_translations
            WHERE attribute_id IN ({id_placeholders})
              AND locale IN ({locale_placeholders})
            "#
        ),
        values,
    ))
    .all(db)
    .await?;

    let mut by_attribute = HashMap::<Uuid, BTreeMap<usize, String>>::new();
    for row in rows {
        let Some(rank) = candidates
            .iter()
            .position(|candidate| candidate == &row.locale)
        else {
            continue;
        };
        by_attribute
            .entry(row.attribute_id)
            .or_default()
            .insert(rank, row.label);
    }

    Ok(by_attribute
        .into_iter()
        .filter_map(|(attribute_id, mut labels)| {
            labels
                .into_iter()
                .next()
                .map(|(_, label)| (attribute_id, label))
        })
        .collect())
}

async fn load_facet_option_labels(
    db: &DatabaseConnection,
    option_ids: &[Uuid],
    locale: &str,
    fallback_locale: &str,
) -> CommerceResult<HashMap<Uuid, String>> {
    let candidates = locale_candidates(locale.trim(), fallback_locale.trim());
    let backend = db.get_database_backend();
    let mut values = Vec::<sea_orm::Value>::new();
    let id_placeholders = option_ids
        .iter()
        .map(|option_id| {
            values.push((*option_id).into());
            sql_placeholder(backend, values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut labels = HashMap::<Uuid, BTreeMap<usize, String>>::new();
    if !candidates.is_empty() {
        let mut label_values = values.clone();
        let locale_placeholders = candidates
            .iter()
            .map(|candidate| {
                label_values.push(candidate.clone().into());
                sql_placeholder(backend, label_values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let rows = FacetOptionLabelRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            format!(
                r#"
                SELECT option_id, locale, label
                FROM product_attribute_option_translations
                WHERE option_id IN ({id_placeholders})
                  AND locale IN ({locale_placeholders})
                "#
            ),
            label_values,
        ))
        .all(db)
        .await?;
        for row in rows {
            let Some(rank) = candidates
                .iter()
                .position(|candidate| candidate == &row.locale)
            else {
                continue;
            };
            labels
                .entry(row.option_id)
                .or_default()
                .insert(rank, row.label);
        }
    }

    // Options without a translation row (or archived options still referenced by a product) fall
    // back to the dictionary code so a bucket is never rendered as a raw identifier when the
    // dictionary still knows the option.
    let missing = option_ids
        .iter()
        .filter(|option_id| !labels.contains_key(option_id))
        .copied()
        .collect::<Vec<_>>();
    let mut resolved = labels
        .into_iter()
        .filter_map(|(option_id, mut by_locale)| {
            by_locale
                .into_iter()
                .next()
                .map(|(_, label)| (option_id, label))
        })
        .collect::<HashMap<_, _>>();
    if !missing.is_empty() {
        let mut code_values = Vec::<sea_orm::Value>::new();
        let missing_placeholders = missing
            .iter()
            .map(|option_id| {
                code_values.push((*option_id).into());
                sql_placeholder(backend, code_values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let rows = FacetOptionCodeRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            format!(
                r#"
                SELECT id, code
                FROM product_attribute_options
                WHERE id IN ({missing_placeholders})
                "#
            ),
            code_values,
        ))
        .all(db)
        .await?;
        for row in rows {
            resolved.insert(row.id, row.code);
        }
    }

    Ok(resolved)
}

/// Locale candidates in resolution order: requested, then fallback (when different).
fn locale_candidates(locale: &str, fallback_locale: &str) -> Vec<String> {
    let mut candidates = Vec::with_capacity(2);
    if !locale.is_empty() {
        candidates.push(locale.to_string());
    }
    if !fallback_locale.is_empty() && fallback_locale != locale {
        candidates.push(fallback_locale.to_string());
    }
    candidates
}

/// Builds the product filter set shared by every facet query.
///
/// `other_conditions` are the already-resolved attribute-filter conditions of every *other*
/// selected attribute. The product status is compared with an inline literal because PostgreSQL
/// stores it as the `product_status_enum` type, which does not resolve against a text parameter.
fn facet_products_condition(
    backend: DbBackend,
    tenant_id: Uuid,
    public_channel_slug: Option<&str>,
    list_query: &super::StorefrontProductListQuery,
    other_conditions: Vec<Condition>,
) -> Condition {
    let mut condition = Condition::all()
        .add(Expr::cust_with_values(
            format!("products.tenant_id = {}", sql_placeholder(backend, 1)),
            vec![sea_orm::Value::from(tenant_id)],
        ))
        .add(Expr::cust("products.status = 'active'"))
        .add(Expr::cust("products.published_at IS NOT NULL"))
        .add(product_channel_visibility_condition(
            backend,
            public_channel_slug,
        ));
    if let Some(category_id) = list_query.category_id {
        condition = condition.add(Expr::cust_with_values(
            format!(
                "products.primary_category_id = {}",
                sql_placeholder(backend, 1)
            ),
            vec![sea_orm::Value::from(category_id)],
        ));
    }
    if let Some(search) = list_query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        condition = condition.add(product_title_search_condition(backend, search));
    }
    for other in other_conditions {
        condition = condition.add(other);
    }
    condition
}

/// Attributes whose bucket vocabulary is bounded and therefore enumerable for a storefront UI.
trait EnumerableFacet {
    fn is_enumerable_facet(self) -> bool;
}

impl EnumerableFacet for AttributeValueType {
    fn is_enumerable_facet(self) -> bool {
        matches!(self, Self::Select | Self::Multiselect | Self::Boolean)
    }
}

async fn load_facet_total(
    db: &DatabaseConnection,
    attribute_id: &Uuid,
    products_condition: &Condition,
) -> CommerceResult<u64> {
    let mut query = facet_base_query(db.get_database_backend(), attribute_id);
    query
        .expr_as(
            Expr::cust("COUNT(DISTINCT products.id)"),
            Alias::new("product_count"),
        )
        .cond_where(products_condition.clone());
    let statement = db.get_database_backend().build(&query);
    let row = FacetCountRow::find_by_statement(statement)
        .one(db)
        .await?;
    Ok(row.map(|row| std::cmp::Ord::max(row.product_count, 0) as u64).unwrap_or(0))
}

async fn load_facet_buckets(
    db: &DatabaseConnection,
    definition: &FacetDefinitionRow,
    products_condition: &Condition,
) -> CommerceResult<Vec<(String, u64)>> {
    let value_type = AttributeValueType::from_storage(&definition.value_type).map_err(|_| {
        CommerceError::Validation(format!(
            "attribute {} has an unsupported stored value type",
            definition.code
        ))
    })?;

    let backend = db.get_database_backend();
    let pav = Alias::new("facet_pav");
    let mut query = facet_base_query(backend, &definition.id);
    // Buckets are selected as their native column types and rendered to strings in Rust: casting
    // a uuid column to text would depend on the backend's storage encoding, and the same SQL runs
    // on PostgreSQL and the portable SQLite test schema.
    let buckets = match value_type {
        AttributeValueType::Select | AttributeValueType::Multiselect => {
            let pavo = Alias::new("facet_pavo");
            query
                .column((pavo.clone(), Alias::new("option_id")))
                .expr_as(
                    Expr::cust("COUNT(DISTINCT products.id)"),
                    Alias::new("product_count"),
                )
                .join_as(
                    JoinType::InnerJoin,
                    Alias::new("product_attribute_value_options"),
                    pavo.clone(),
                    Expr::col((pavo.clone(), Alias::new("value_id")))
                        .equals((pav.clone(), Alias::new("id"))),
                )
                .group_by_col((pavo.clone(), Alias::new("option_id")))
                .cond_where(products_condition.clone())
                .order_by_expr(Expr::cust("product_count"), Order::Desc)
                .order_by((pavo.clone(), Alias::new("option_id")), Order::Asc)
                // One extra row distinguishes "exactly the limit" from "truncated".
                .limit((MAX_CATALOG_FACET_VALUES as u64) + 1);

            let statement = backend.build(&query);
            FacetOptionBucketRow::find_by_statement(statement)
                .all(db)
                .await?
                .into_iter()
                .map(|row| (row.option_id.to_string(), std::cmp::Ord::max(row.product_count, 0) as u64))
                .collect::<Vec<_>>()
        }
        AttributeValueType::Boolean => {
            query
                .column((pav.clone(), Alias::new("value_boolean")))
                .expr_as(
                    Expr::cust("COUNT(DISTINCT products.id)"),
                    Alias::new("product_count"),
                )
                .group_by_col((pav.clone(), Alias::new("value_boolean")))
                .cond_where(products_condition.clone())
                .order_by_expr(Expr::cust("product_count"), Order::Desc)
                .order_by((pav.clone(), Alias::new("value_boolean")), Order::Asc)
                .limit((MAX_CATALOG_FACET_VALUES as u64) + 1);

            let statement = backend.build(&query);
            FacetBooleanBucketRow::find_by_statement(statement)
                .all(db)
                .await?
                .into_iter()
                .map(|row| {
                    (
                        row.value_boolean.to_string(),
                        std::cmp::Ord::max(row.product_count, 0) as u64,
                    )
                })
                .collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };
    Ok(buckets)
}

/// `FROM products JOIN product_attribute_values facet_pav ON ...` with the tenant, attribute and
/// detached-value guard every facet query needs.
fn facet_base_query(backend: DbBackend, attribute_id: &Uuid) -> sea_query::SelectStatement {
    let pav = Alias::new("facet_pav");
    let mut query = Query::select();
    query
        .from(Alias::new("products"))
        .join_as(
            JoinType::InnerJoin,
            Alias::new("product_attribute_values"),
            pav.clone(),
            Expr::col((pav.clone(), Alias::new("product_id")))
                .equals((Alias::new("products"), Alias::new("id"))),
        )
        .and_where(
            Expr::col((pav.clone(), Alias::new("tenant_id")))
                .eq(Expr::col((Alias::new("products"), Alias::new("tenant_id")))),
        )
        .and_where(Expr::cust_with_values(
            format!("facet_pav.attribute_id = {}", sql_placeholder(backend, 1)),
            vec![sea_orm::Value::from(*attribute_id)],
        ))
        .and_where(Expr::col((pav, Alias::new("detached_at"))).is_null());
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn facet_codes_are_trimmed_deduplicated_and_bounded() {
        let normalized =
            normalize_facet_codes(&codes(&[" color ", "COLOR", "size"])).expect("valid");
        assert_eq!(normalized, codes(&["color", "size"]));

        assert!(normalize_facet_codes(&codes(&["   "])).is_err());
        assert!(normalize_facet_codes(&codes(&["x".repeat(129).as_str()])).is_err());
        let too_many = (0..=MAX_CATALOG_FACETS)
            .map(|index| format!("code_{index}"))
            .collect::<Vec<_>>();
        let error = normalize_facet_codes(&too_many).expect_err("facet limit");
        assert!(error.to_string().contains("at most"));
    }

    #[test]
    fn only_bounded_dictionaries_are_enumerable() {
        assert!(AttributeValueType::Select.is_enumerable_facet());
        assert!(AttributeValueType::Multiselect.is_enumerable_facet());
        assert!(AttributeValueType::Boolean.is_enumerable_facet());
        assert!(!AttributeValueType::Text.is_enumerable_facet());
        assert!(!AttributeValueType::Integer.is_enumerable_facet());
        assert!(!AttributeValueType::Decimal.is_enumerable_facet());
        assert!(!AttributeValueType::Date.is_enumerable_facet());
        assert!(!AttributeValueType::Json.is_enumerable_facet());
    }

    #[test]
    fn locale_candidates_prefer_the_requested_locale() {
        assert_eq!(locale_candidates("de", "en"), vec!["de", "en"]);
        assert_eq!(locale_candidates("en", "en"), vec!["en"]);
        assert_eq!(locale_candidates("", "en"), vec!["en"]);
    }

    #[test]
    fn bucket_limit_leaves_room_to_detect_truncation() {
        assert_eq!(MAX_CATALOG_FACET_VALUES, 20);
        assert!(MAX_CATALOG_FACETS >= 4);
    }
}
