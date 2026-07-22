use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_core::events::{EventHandler, HandlerResult};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_product::services::{
    VirtualCategoryAttributeCondition, VirtualCategoryRuleV1,
    load_effective_product_form_from_storage, parse_virtual_category_rule_v1,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement,
    TransactionTrait,
};
use serde_json::Value as JsonValue;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::error::IndexResult;
use crate::traits::{
    Indexer, IndexerContext, IndexerRuntimeConfig, LocaleIndexer, run_bounded_reindex,
};

#[derive(Debug, FromQueryResult)]
struct ProductRow {
    id: Uuid,
    tenant_id: Uuid,
    status: String,
    vendor: Option<String>,
    metadata: JsonValue,
    primary_category_id: Option<Uuid>,
    category_name: Option<String>,
    category_path: Option<String>,
    published_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    created_at: chrono::DateTime<chrono::FixedOffset>,
    updated_at: chrono::DateTime<chrono::FixedOffset>,
    locale: Option<String>,
    title: Option<String>,
    handle: Option<String>,
    description: Option<String>,
    meta_title: Option<String>,
    meta_description: Option<String>,
}

#[derive(FromQueryResult)]
struct VariantAgg {
    variant_count: i64,
    in_stock: bool,
    total_inventory: i64,
    price_min: Option<i64>,
    price_max: Option<i64>,
}

#[derive(FromQueryResult)]
struct VirtualCategoryRow {
    id: Uuid,
    rule_config: JsonValue,
}

#[derive(FromQueryResult)]
struct VirtualProductFacts {
    status: String,
    primary_category_id: Option<Uuid>,
    in_stock: bool,
    price_min: Option<i64>,
    price_max: Option<i64>,
}

#[derive(FromQueryResult)]
struct CategoryAncestorRow {
    ancestor_id: Uuid,
}

#[derive(FromQueryResult)]
struct VirtualAttributeFactRow {
    attribute_code: String,
    value_key: Option<String>,
    value_number: Option<Decimal>,
}

fn virtual_category_rule_matches(
    rule: &VirtualCategoryRuleV1,
    facts: &VirtualProductFacts,
    primary_category_ancestors: &std::collections::HashSet<Uuid>,
    attribute_facts: &[VirtualAttributeFactRow],
) -> bool {
    if !rule.statuses.is_empty() && !rule.statuses.iter().any(|status| status == &facts.status) {
        return false;
    }
    if rule
        .primary_category_subtree_id
        .is_some_and(|category_id| !primary_category_ancestors.contains(&category_id))
    {
        return false;
    }
    if let Some(min) = rule.price_min {
        if facts.price_max.is_none_or(|price| price < min) {
            return false;
        }
    }
    if let Some(max) = rule.price_max {
        if facts.price_min.is_none_or(|price| price > max) {
            return false;
        }
    }
    if rule
        .in_stock
        .is_some_and(|expected| expected != facts.in_stock)
    {
        return false;
    }

    rule.attributes.iter().all(|attribute| {
        attribute_facts.iter().any(|fact| {
            if fact.attribute_code != attribute.code {
                return false;
            }
            match &attribute.condition {
                VirtualCategoryAttributeCondition::Eq { value } => {
                    fact.value_key.as_deref() == Some(value.as_str())
                }
                VirtualCategoryAttributeCondition::Range { min, max } => {
                    fact.value_number.is_some_and(|number| {
                        min.is_none_or(|minimum| number >= minimum)
                            && max.is_none_or(|maximum| number <= maximum)
                    })
                }
            }
        })
    })
}

#[derive(Clone)]
pub struct ProductIndexer {
    db: DatabaseConnection,
    runtime: IndexerRuntimeConfig,
}

impl ProductIndexer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_runtime(db, IndexerRuntimeConfig::load())
    }

    pub fn with_runtime(db: DatabaseConnection, runtime: IndexerRuntimeConfig) -> Self {
        Self { db, runtime }
    }

    fn backend(&self) -> DatabaseBackend {
        self.db.get_database_backend()
    }

    #[instrument(skip(self, ctx))]
    async fn build_index_product(
        &self,
        ctx: &IndexerContext,
        product_id: Uuid,
        locale: &str,
    ) -> IndexResult<Option<super::model::IndexProductModel>> {
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            SELECT
                p.id,
                p.tenant_id,
                p.status::text AS status,
                p.vendor,
                p.metadata,
                p.primary_category_id,
                c.path AS category_path,
                ct.name AS category_name,
                p.published_at,
                p.created_at,
                p.updated_at,
                pt.locale,
                pt.title,
                pt.handle,
                pt.description,
                pt.meta_title,
                pt.meta_description
            FROM products p
            LEFT JOIN product_translations pt
                ON pt.product_id = p.id AND pt.locale = $3
            LEFT JOIN catalog_categories c
                ON c.id = p.primary_category_id AND c.tenant_id = p.tenant_id
            LEFT JOIN catalog_category_translations ct
                ON ct.category_id = c.id AND ct.locale = $3
            WHERE p.id = $1
              AND p.tenant_id = $2
            "#,
            vec![product_id.into(), ctx.tenant_id.into(), locale.into()],
        );

        let row = ProductRow::find_by_statement(stmt)
            .one(&self.db)
            .await
            .map_err(crate::error::IndexError::from)?;

        let row = match row {
            Some(r) => r,
            None => {
                debug!(product_id = %product_id, "Product not found, skipping index");
                return Ok(None);
            }
        };

        let agg_stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            SELECT
                COUNT(pv.id)::bigint AS variant_count,
                COALESCE(SUM(inv.available_quantity), 0) > 0 AS in_stock,
                COALESCE(SUM(inv.available_quantity), 0)::bigint AS total_inventory,
                MIN(price_agg.min_amount)::bigint AS price_min,
                MAX(price_agg.max_amount)::bigint AS price_max
            FROM product_variants pv
            LEFT JOIN LATERAL (
                SELECT COALESCE(SUM(il.stocked_quantity - il.reserved_quantity), 0) AS available_quantity
                FROM inventory_items ii
                LEFT JOIN inventory_levels il ON il.inventory_item_id = ii.id
                WHERE ii.variant_id = pv.id
            ) inv ON TRUE
            LEFT JOIN LATERAL (
                SELECT
                    MIN(pr.amount) AS min_amount,
                    MAX(pr.amount) AS max_amount
                FROM prices pr
                WHERE pr.variant_id = pv.id
            ) price_agg ON TRUE
            WHERE pv.product_id = $1
              AND pv.tenant_id = $2
            "#,
            vec![product_id.into(), ctx.tenant_id.into()],
        );

        let agg = VariantAgg::find_by_statement(agg_stmt)
            .one(&self.db)
            .await
            .map_err(crate::error::IndexError::from)?
            .unwrap_or(VariantAgg {
                variant_count: 0,
                in_stock: false,
                total_inventory: 0,
                price_min: None,
                price_max: None,
            });

        let is_published = row.status == "active";
        let attributes = self
            .load_indexed_attributes(ctx, product_id, locale)
            .await?
            .filter(|attributes| {
                attributes
                    .as_object()
                    .map(|object| !object.is_empty())
                    .unwrap_or(false)
            })
            .unwrap_or(row.metadata);

        let model = super::model::IndexProductModel {
            id: Uuid::new_v4(),
            tenant_id: row.tenant_id,
            product_id: row.id,
            locale: row.locale.unwrap_or_else(|| locale.to_string()),
            status: row.status,
            is_published,
            title: row.title.unwrap_or_default(),
            subtitle: None,
            handle: row.handle.unwrap_or_default(),
            description: row.description,
            category_id: row.primary_category_id,
            category_name: row.category_name,
            category_path: row.category_path,
            tags: vec![],
            brand: row.vendor,
            currency: None,
            price_min: agg.price_min,
            price_max: agg.price_max,
            compare_at_price_min: None,
            compare_at_price_max: None,
            on_sale: false,
            in_stock: agg.in_stock,
            total_inventory: i32::try_from(agg.total_inventory).unwrap_or(i32::MAX),
            variant_count: i32::try_from(agg.variant_count).unwrap_or(i32::MAX),
            options: vec![],
            thumbnail_url: None,
            images: vec![],
            meta_title: row.meta_title,
            meta_description: row.meta_description,
            attributes,
            sales_count: 0,
            view_count: 0,
            rating: None,
            review_count: 0,
            published_at: row.published_at.map(|dt| dt.with_timezone(&chrono::Utc)),
            created_at: row.created_at.with_timezone(&chrono::Utc),
            updated_at: row.updated_at.with_timezone(&chrono::Utc),
        };

        Ok(Some(model))
    }

    async fn load_indexed_attributes(
        &self,
        ctx: &IndexerContext,
        product_id: Uuid,
        locale: &str,
    ) -> IndexResult<Option<JsonValue>> {
        #[derive(FromQueryResult)]
        struct AttributeProjection {
            attributes: JsonValue,
        }

        let effective_form =
            load_effective_product_form_from_storage(&self.db, ctx.tenant_id, product_id)
                .await
                .map_err(|error| crate::error::IndexError::Index(error.to_string()))?;
        let effective_bindings = effective_form
            .into_iter()
            .flat_map(|form| form.attributes)
            .filter(|binding| !binding.is_disabled)
            .collect::<Vec<_>>();
        if effective_bindings.is_empty() {
            return Ok(Some(JsonValue::Object(Default::default())));
        }
        let bindings_json = serde_json::to_value(effective_bindings)?;
        let stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            WITH effective AS (
                SELECT
                    (binding->>'attribute_id')::uuid AS attribute_id,
                    (binding->'visibility_overrides'->>'is_filterable')::boolean AS is_filterable,
                    (binding->'visibility_overrides'->>'is_searchable')::boolean AS is_searchable,
                    (binding->'visibility_overrides'->>'is_sortable')::boolean AS is_sortable,
                    (binding->'visibility_overrides'->>'show_on_storefront')::boolean AS show_on_storefront
                FROM jsonb_array_elements($4::jsonb) AS binding
            )
            SELECT COALESCE(
                jsonb_object_agg(
                    pa.code,
                    jsonb_build_object(
                        'attribute_id', pa.id,
                        'value_type', pa.value_type,
                        'value_key', COALESCE(
                            pao.code,
                            pav.value_text,
                            pav.value_integer::text,
                            pav.value_decimal::text,
                            pav.value_boolean::text,
                            pav.value_date::text,
                            pav.value_datetime::text
                        ),
                        'value_label', COALESCE(paot.label, pavt.value_text, pav.value_text, pao.code),
                        'value_number', COALESCE(pav.value_decimal, pav.value_integer::numeric),
                        'value_bool', pav.value_boolean,
                        'value_datetime', pav.value_datetime,
                        'is_filterable', COALESCE(e.is_filterable, pa.is_filterable),
                        'is_searchable', COALESCE(e.is_searchable, pa.is_searchable),
                        'is_sortable', COALESCE(e.is_sortable, pa.is_sortable),
                        'show_on_storefront', COALESCE(e.show_on_storefront, pa.show_on_storefront)
                    )
                ) FILTER (WHERE pa.id IS NOT NULL),
                '{}'::jsonb
            ) AS attributes
            FROM product_attribute_values pav
            JOIN effective e ON e.attribute_id = pav.attribute_id
            JOIN product_attributes pa
                ON pa.id = pav.attribute_id
               AND pa.tenant_id = pav.tenant_id
               AND pa.archived_at IS NULL
            LEFT JOIN product_attribute_value_translations pavt
                ON pavt.value_id = pav.id AND pavt.locale = $3
            LEFT JOIN product_attribute_value_options pavo
                ON pavo.value_id = pav.id
            LEFT JOIN product_attribute_options pao
                ON pao.id = pavo.option_id AND pao.archived_at IS NULL
            LEFT JOIN product_attribute_option_translations paot
                ON paot.option_id = pao.id AND paot.locale = $3
            WHERE pav.tenant_id = $1
              AND pav.product_id = $2
              AND COALESCE(e.show_on_storefront, pa.show_on_storefront) = TRUE
            "#,
            vec![
                ctx.tenant_id.into(),
                product_id.into(),
                locale.into(),
                bindings_json.into(),
            ],
        );

        AttributeProjection::find_by_statement(stmt)
            .one(&self.db)
            .await
            .map(|projection| projection.map(|projection| projection.attributes))
            .map_err(crate::error::IndexError::from)
    }

    async fn upsert_index_product(
        &self,
        model: &super::model::IndexProductModel,
    ) -> IndexResult<()> {
        let tags_json = JsonValue::Array(
            model
                .tags
                .iter()
                .map(|t| JsonValue::String(t.clone()))
                .collect(),
        );
        let options_json = serde_json::to_value(&model.options).unwrap_or(JsonValue::Array(vec![]));
        let images_json = serde_json::to_value(&model.images).unwrap_or(JsonValue::Array(vec![]));

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            r#"
            INSERT INTO index_products (
                id, tenant_id, product_id, locale, status, is_published,
                title, subtitle, handle, description,
                category_id, category_name, category_path,
                tags, brand, currency,
                price_min, price_max, compare_at_price_min, compare_at_price_max, on_sale,
                in_stock, total_inventory, variant_count, options,
                thumbnail_url, images,
                meta_title, meta_description, attributes,
                sales_count, view_count, rating, review_count,
                published_at, created_at, updated_at, indexed_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10,
                $11, $12, $13,
                $14, $15, $16,
                $17, $18, $19, $20, $21,
                $22, $23, $24, $25,
                $26, $27,
                $28, $29, $30,
                $31, $32, $33, $34,
                $35, $36, $37, NOW()
            )
            ON CONFLICT (product_id, locale) DO UPDATE SET
                status = EXCLUDED.status,
                is_published = EXCLUDED.is_published,
                title = EXCLUDED.title,
                subtitle = EXCLUDED.subtitle,
                handle = EXCLUDED.handle,
                description = EXCLUDED.description,
                category_id = EXCLUDED.category_id,
                category_name = EXCLUDED.category_name,
                category_path = EXCLUDED.category_path,
                tags = EXCLUDED.tags,
                brand = EXCLUDED.brand,
                currency = EXCLUDED.currency,
                price_min = EXCLUDED.price_min,
                price_max = EXCLUDED.price_max,
                compare_at_price_min = EXCLUDED.compare_at_price_min,
                compare_at_price_max = EXCLUDED.compare_at_price_max,
                on_sale = EXCLUDED.on_sale,
                in_stock = EXCLUDED.in_stock,
                total_inventory = EXCLUDED.total_inventory,
                variant_count = EXCLUDED.variant_count,
                options = EXCLUDED.options,
                thumbnail_url = EXCLUDED.thumbnail_url,
                images = EXCLUDED.images,
                meta_title = EXCLUDED.meta_title,
                meta_description = EXCLUDED.meta_description,
                attributes = EXCLUDED.attributes,
                sales_count = EXCLUDED.sales_count,
                view_count = EXCLUDED.view_count,
                rating = EXCLUDED.rating,
                review_count = EXCLUDED.review_count,
                published_at = EXCLUDED.published_at,
                updated_at = EXCLUDED.updated_at,
                indexed_at = NOW()
            "#,
            vec![
                model.id.into(),
                model.tenant_id.into(),
                model.product_id.into(),
                model.locale.clone().into(),
                model.status.clone().into(),
                model.is_published.into(),
                model.title.clone().into(),
                model.subtitle.clone().into(),
                model.handle.clone().into(),
                model.description.clone().into(),
                model.category_id.into(),
                model.category_name.clone().into(),
                model.category_path.clone().into(),
                tags_json.into(),
                model.brand.clone().into(),
                model.currency.clone().into(),
                model.price_min.into(),
                model.price_max.into(),
                model.compare_at_price_min.into(),
                model.compare_at_price_max.into(),
                model.on_sale.into(),
                model.in_stock.into(),
                model.total_inventory.into(),
                model.variant_count.into(),
                options_json.into(),
                model.thumbnail_url.clone().into(),
                images_json.into(),
                model.meta_title.clone().into(),
                model.meta_description.clone().into(),
                model.attributes.clone().into(),
                model.sales_count.into(),
                model.view_count.into(),
                model.rating.into(),
                model.review_count.into(),
                model.published_at.into(),
                model.created_at.into(),
                model.updated_at.into(),
            ],
        );

        self.db
            .execute(stmt)
            .await
            .map(|_| ())
            .map_err(crate::error::IndexError::from)
    }

    async fn refresh_virtual_category_assignments(
        &self,
        ctx: &IndexerContext,
        product_id: Uuid,
    ) -> IndexResult<()> {
        let facts = VirtualProductFacts::find_by_statement(Statement::from_sql_and_values(
            self.backend(),
            r#"
            SELECT
                p.status::text AS status,
                p.primary_category_id,
                EXISTS (
                    SELECT 1
                    FROM product_variants pv
                    JOIN inventory_items ii ON ii.variant_id = pv.id
                    JOIN inventory_levels il ON il.inventory_item_id = ii.id
                    WHERE pv.tenant_id = p.tenant_id AND pv.product_id = p.id
                      AND il.stocked_quantity - il.reserved_quantity > 0
                ) AS in_stock,
                (
                    SELECT MIN(pr.amount)::bigint
                    FROM product_variants pv
                    JOIN prices pr ON pr.variant_id = pv.id
                    WHERE pv.tenant_id = p.tenant_id AND pv.product_id = p.id
                ) AS price_min,
                (
                    SELECT MAX(pr.amount)::bigint
                    FROM product_variants pv
                    JOIN prices pr ON pr.variant_id = pv.id
                    WHERE pv.tenant_id = p.tenant_id AND pv.product_id = p.id
                ) AS price_max
            FROM products p
            WHERE p.tenant_id = $1 AND p.id = $2
            "#,
            vec![ctx.tenant_id.into(), product_id.into()],
        ))
        .one(&self.db)
        .await?;

        let Some(facts) = facts else {
            return Ok(());
        };
        let primary_category_ancestors =
            if let Some(primary_category_id) = facts.primary_category_id {
                CategoryAncestorRow::find_by_statement(Statement::from_sql_and_values(
                    self.backend(),
                    r#"
                SELECT ancestor_id
                FROM catalog_category_closure
                WHERE tenant_id = $1 AND descendant_id = $2
                "#,
                    vec![ctx.tenant_id.into(), primary_category_id.into()],
                ))
                .all(&self.db)
                .await?
                .into_iter()
                .map(|row| row.ancestor_id)
                .collect()
            } else {
                std::collections::HashSet::new()
            };

        let effective_attribute_ids =
            load_effective_product_form_from_storage(&self.db, ctx.tenant_id, product_id)
                .await
                .map_err(|error| crate::error::IndexError::Index(error.to_string()))?
                .into_iter()
                .flat_map(|form| form.attributes)
                .filter(|binding| !binding.is_disabled)
                .map(|binding| binding.attribute_id)
                .collect::<Vec<_>>();
        let attribute_facts = if effective_attribute_ids.is_empty() {
            Vec::new()
        } else {
            let mut values = vec![ctx.tenant_id.into(), product_id.into()];
            let placeholders = effective_attribute_ids
                .into_iter()
                .map(|attribute_id| {
                    values.push(attribute_id.into());
                    format!("${}", values.len())
                })
                .collect::<Vec<_>>()
                .join(", ");
            VirtualAttributeFactRow::find_by_statement(Statement::from_sql_and_values(
                self.backend(),
                format!(
                    r#"
                    SELECT
                        pa.code AS attribute_code,
                        COALESCE(
                            pao.code,
                            pav.value_text,
                            pav.value_integer::text,
                            pav.value_decimal::text,
                            pav.value_boolean::text,
                            pav.value_date::text,
                            pav.value_datetime::text
                        ) AS value_key,
                        COALESCE(pav.value_decimal, pav.value_integer::numeric) AS value_number
                    FROM product_attribute_values pav
                    JOIN product_attributes pa
                      ON pa.tenant_id = pav.tenant_id AND pa.id = pav.attribute_id
                     AND pa.archived_at IS NULL
                    LEFT JOIN product_attribute_value_options pavo ON pavo.value_id = pav.id
                    LEFT JOIN product_attribute_options pao
                      ON pao.id = pavo.option_id AND pao.archived_at IS NULL
                    WHERE pav.tenant_id = $1 AND pav.product_id = $2
                      AND pav.attribute_id IN ({placeholders})
                    "#
                ),
                values,
            ))
            .all(&self.db)
            .await?
        };

        let categories = VirtualCategoryRow::find_by_statement(Statement::from_sql_and_values(
            self.backend(),
            r#"
            SELECT id, rule_config
            FROM catalog_categories
            WHERE tenant_id = $1 AND kind = 'virtual'
              AND is_active = TRUE AND deleted_at IS NULL
            "#,
            vec![ctx.tenant_id.into()],
        ))
        .all(&self.db)
        .await?;
        let mut matched_category_ids = Vec::new();
        for category in categories {
            match parse_virtual_category_rule_v1(&category.rule_config) {
                Ok(rule)
                    if virtual_category_rule_matches(
                        &rule,
                        &facts,
                        &primary_category_ancestors,
                        &attribute_facts,
                    ) =>
                {
                    matched_category_ids.push(category.id);
                }
                Ok(_) => {}
                Err(error) => warn!(
                    category_id = %category.id,
                    error = %error,
                    "Skipping invalid virtual category rule"
                ),
            }
        }

        let txn = self.db.begin().await?;
        txn.execute(Statement::from_sql_and_values(
            self.backend(),
            "DELETE FROM virtual_category_product_assignments WHERE tenant_id = $1 AND product_id = $2",
            vec![ctx.tenant_id.into(), product_id.into()],
        ))
        .await?;
        for category_id in matched_category_ids {
            txn.execute(Statement::from_sql_and_values(
                self.backend(),
                r#"
                INSERT INTO virtual_category_product_assignments (
                    tenant_id, category_id, product_id, matched_at, match_reason
                ) VALUES ($1, $2, $3, NOW(), $4)
                "#,
                vec![
                    ctx.tenant_id.into(),
                    category_id.into(),
                    product_id.into(),
                    serde_json::json!({ "rule_version": 1 }).into(),
                ],
            ))
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    async fn refresh_product_category_projection(
        &self,
        ctx: &IndexerContext,
        product_id: Uuid,
        locale: &str,
    ) -> IndexResult<()> {
        let txn = self.db.begin().await?;
        txn.execute(Statement::from_sql_and_values(
            self.backend(),
            "DELETE FROM index_product_categories WHERE tenant_id = $1 AND product_id = $2 AND locale = $3",
            vec![ctx.tenant_id.into(), product_id.into(), locale.into()],
        ))
        .await?;
        txn.execute(Statement::from_sql_and_values(
            self.backend(),
            r#"
            WITH assignments AS (
                SELECT p.primary_category_id AS category_id,
                       'primary'::text AS assignment_kind,
                       0 AS position,
                       0 AS priority
                FROM products p
                WHERE p.tenant_id = $1 AND p.id = $2 AND p.primary_category_id IS NOT NULL
                UNION ALL
                SELECT pc.category_id, pc.assignment_kind, pc.position, 1 AS priority
                FROM product_categories pc
                WHERE pc.tenant_id = $1 AND pc.product_id = $2
                UNION ALL
                SELECT vcpa.category_id, 'virtual'::text, 0, 2 AS priority
                FROM virtual_category_product_assignments vcpa
                WHERE vcpa.tenant_id = $1 AND vcpa.product_id = $2
            ), deduplicated AS (
                SELECT DISTINCT ON (category_id)
                       category_id, assignment_kind, position
                FROM assignments
                ORDER BY category_id, priority
            )
            INSERT INTO index_product_categories (
                tenant_id, product_id, category_id, locale, category_kind,
                assignment_kind, path, name, position, indexed_at
            )
            SELECT $1, $2, d.category_id, $3, c.kind, d.assignment_kind,
                   c.path, ct.name, d.position, NOW()
            FROM deduplicated d
            JOIN catalog_categories c
              ON c.tenant_id = $1 AND c.id = d.category_id
             AND c.deleted_at IS NULL AND c.is_active = TRUE
            LEFT JOIN catalog_category_translations ct
              ON ct.category_id = c.id AND ct.locale = $3
            "#,
            vec![ctx.tenant_id.into(), product_id.into(), locale.into()],
        ))
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn refresh_product_attribute_projection(
        &self,
        ctx: &IndexerContext,
        product_id: Uuid,
        locale: &str,
    ) -> IndexResult<()> {
        let effective_form =
            load_effective_product_form_from_storage(&self.db, ctx.tenant_id, product_id)
                .await
                .map_err(|error| crate::error::IndexError::Index(error.to_string()))?;
        let effective_bindings = effective_form
            .into_iter()
            .flat_map(|form| form.attributes)
            .filter(|binding| !binding.is_disabled)
            .collect::<Vec<_>>();

        let txn = self.db.begin().await?;
        txn.execute(Statement::from_sql_and_values(
            self.backend(),
            "DELETE FROM index_product_attribute_values WHERE tenant_id = $1 AND product_id = $2 AND locale = $3",
            vec![ctx.tenant_id.into(), product_id.into(), locale.into()],
        ))
        .await?;

        if !effective_bindings.is_empty() {
            let bindings_json = serde_json::to_value(effective_bindings)?;
            txn.execute(Statement::from_sql_and_values(
                self.backend(),
                r#"
                    WITH effective AS (
                        SELECT
                            (binding->>'attribute_id')::uuid AS attribute_id,
                            (binding->'visibility_overrides'->>'is_filterable')::boolean AS is_filterable,
                            (binding->'visibility_overrides'->>'is_searchable')::boolean AS is_searchable,
                            (binding->'visibility_overrides'->>'is_sortable')::boolean AS is_sortable,
                            (binding->'visibility_overrides'->>'is_comparable')::boolean AS is_comparable,
                            (binding->'visibility_overrides'->>'show_on_storefront')::boolean AS show_on_storefront,
                            (binding->'visibility_overrides'->>'show_in_admin_grid')::boolean AS show_in_admin_grid
                        FROM jsonb_array_elements($4::jsonb) AS binding
                    ), channel_scope AS (
                        SELECT id AS channel_id
                        FROM channels
                        WHERE tenant_id = $1 AND is_active = TRUE
                        UNION ALL
                        SELECT NULL::uuid
                        WHERE NOT EXISTS (
                            SELECT 1 FROM channels
                            WHERE tenant_id = $1 AND is_active = TRUE
                        )
                    ), normalized AS (
                        SELECT
                            channel_scope.channel_id,
                            pa.id AS attribute_id,
                            pa.code AS attribute_code,
                            CASE
                                WHEN pa.value_type IN ('select', 'multiselect') THEN pao.code
                                WHEN pa.is_localized THEN pavt.value_text
                                ELSE COALESCE(
                                    pav.value_text,
                                    pav.value_integer::text,
                                    pav.value_decimal::text,
                                    pav.value_boolean::text,
                                    pav.value_date::text,
                                    pav.value_datetime::text,
                                    pav.value_json::text
                                )
                            END AS value_key,
                            CASE
                                WHEN pa.value_type IN ('select', 'multiselect') THEN paot.label
                                WHEN pa.is_localized THEN pavt.value_text
                                ELSE COALESCE(pav.value_text, pav.value_integer::text,
                                    pav.value_decimal::text, pav.value_boolean::text,
                                    pav.value_date::text, pav.value_datetime::text,
                                    pav.value_json::text)
                            END AS value_label,
                            COALESCE(pav.value_decimal, pav.value_integer::numeric) AS value_number,
                            pav.value_boolean AS value_bool,
                            pav.value_datetime AS value_datetime,
                            COALESCE(pacs.is_filterable, e.is_filterable, pa.is_filterable) AS is_filterable,
                            COALESCE(pacs.is_searchable, e.is_searchable, pa.is_searchable) AS is_searchable,
                            COALESCE(pacs.is_sortable, e.is_sortable, pa.is_sortable) AS is_sortable,
                            COALESCE(e.is_comparable, pa.is_comparable) AS is_comparable,
                            COALESCE(pacs.show_on_storefront, e.show_on_storefront, pa.show_on_storefront) AS show_on_storefront,
                            COALESCE(pacs.show_in_admin_grid, e.show_in_admin_grid, pa.show_in_admin_grid) AS show_in_admin_grid
                        FROM product_attribute_values pav
                        JOIN effective e ON e.attribute_id = pav.attribute_id
                        JOIN product_attributes pa
                          ON pa.tenant_id = pav.tenant_id AND pa.id = pav.attribute_id
                         AND pa.archived_at IS NULL
                        CROSS JOIN channel_scope
                        LEFT JOIN product_attribute_channel_settings pacs
                          ON pacs.tenant_id = pav.tenant_id
                         AND pacs.attribute_id = pav.attribute_id
                         AND pacs.channel_id = channel_scope.channel_id
                        LEFT JOIN product_attribute_value_translations pavt
                          ON pavt.value_id = pav.id AND pavt.locale = $3
                        LEFT JOIN product_attribute_value_options pavo
                          ON pavo.value_id = pav.id
                        LEFT JOIN product_attribute_options pao
                          ON pao.id = pavo.option_id AND pao.archived_at IS NULL
                        LEFT JOIN product_attribute_option_translations paot
                          ON paot.option_id = pao.id AND paot.locale = $3
                        WHERE pav.tenant_id = $1 AND pav.product_id = $2
                    )
                    INSERT INTO index_product_attribute_values (
                        id, tenant_id, product_id, locale, channel_id,
                        attribute_id, attribute_code, value_key, value_label,
                        value_number, value_bool, value_datetime, sort_value,
                        search_text, facet_bucket_key, is_filterable,
                        is_searchable, is_sortable, is_comparable,
                        show_on_storefront, show_in_admin_grid, is_detached, indexed_at
                    )
                    SELECT
                        md5($1::text || ':' || $2::text || ':' || $3 || ':' ||
                            COALESCE(channel_id::text, 'global') || ':' ||
                            attribute_id::text || ':' || value_key)::uuid,
                        $1, $2, $3, channel_id, attribute_id, attribute_code,
                        value_key, value_label, value_number, value_bool, value_datetime,
                        CASE WHEN is_sortable THEN value_key END,
                        CASE WHEN is_searchable THEN value_label END,
                        CASE WHEN is_filterable THEN value_key END,
                        is_filterable, is_searchable, is_sortable, is_comparable,
                        show_on_storefront, show_in_admin_grid, FALSE, NOW()
                    FROM normalized
                    WHERE value_key IS NOT NULL AND value_key <> ''
                      AND (is_filterable OR is_searchable OR is_sortable OR
                           is_comparable OR show_on_storefront OR show_in_admin_grid)
                    "#,
                vec![
                    ctx.tenant_id.into(),
                    product_id.into(),
                    locale.into(),
                    bindings_json.into(),
                ],
            ))
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    async fn delete_product_from_index(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> IndexResult<()> {
        let txn = self.db.begin().await?;
        for table in [
            "index_product_attribute_values",
            "index_product_categories",
            "index_products",
        ] {
            txn.execute(Statement::from_sql_and_values(
                self.backend(),
                format!("DELETE FROM {table} WHERE tenant_id = $1 AND product_id = $2"),
                vec![tenant_id.into(), product_id.into()],
            ))
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    async fn get_tenant_locales(&self, ctx: &IndexerContext) -> IndexResult<Vec<String>> {
        #[derive(FromQueryResult)]
        struct LocaleRow {
            locale: String,
        }

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "SELECT locale FROM tenant_locales WHERE tenant_id = $1",
            vec![ctx.tenant_id.into()],
        );

        let rows = LocaleRow::find_by_statement(stmt)
            .all(&self.db)
            .await
            .unwrap_or_default();

        if rows.is_empty() {
            Ok(vec!["en".to_string()])
        } else {
            Ok(rows.into_iter().map(|r| r.locale).collect())
        }
    }
}

#[async_trait]
impl Indexer for ProductIndexer {
    fn name(&self) -> &'static str {
        "product_indexer"
    }

    #[instrument(skip(self, ctx))]
    async fn index_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()> {
        self.refresh_virtual_category_assignments(ctx, entity_id)
            .await?;
        let locales = self.get_tenant_locales(ctx).await?;

        for locale in locales {
            let model = self.build_index_product(ctx, entity_id, &locale).await?;
            if let Some(m) = model {
                self.upsert_index_product(&m).await?;
                self.refresh_product_category_projection(ctx, entity_id, &locale)
                    .await?;
                self.refresh_product_attribute_projection(ctx, entity_id, &locale)
                    .await?;
                debug!(product_id = %entity_id, locale = locale, "Indexed product");
            }
        }

        Ok(())
    }

    #[instrument(skip(self, ctx))]
    async fn remove_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()> {
        debug!(product_id = %entity_id, "Removing product from index");
        self.delete_product_from_index(ctx.tenant_id, entity_id)
            .await
    }

    #[instrument(skip(self, ctx))]
    async fn reindex_all(&self, ctx: &IndexerContext) -> IndexResult<u64> {
        info!(tenant_id = %ctx.tenant_id, "Reindexing all products");

        #[derive(FromQueryResult)]
        struct IdRow {
            id: Uuid,
        }

        let stmt = Statement::from_sql_and_values(
            self.backend(),
            "SELECT id FROM products WHERE tenant_id = $1",
            vec![ctx.tenant_id.into()],
        );

        let rows = IdRow::find_by_statement(stmt)
            .all(&self.db)
            .await
            .unwrap_or_default();

        let ids = rows.into_iter().map(|row| row.id).collect();
        let stats = run_bounded_reindex(self.clone(), ctx, ids, "reindex_all").await;
        Ok(stats.scheduled)
    }
}

#[async_trait]
impl LocaleIndexer for ProductIndexer {
    async fn index_locale(
        &self,
        ctx: &IndexerContext,
        entity_id: Uuid,
        locale: &str,
    ) -> IndexResult<()> {
        self.refresh_virtual_category_assignments(ctx, entity_id)
            .await?;
        let model = self.build_index_product(ctx, entity_id, locale).await?;
        if let Some(m) = model {
            self.upsert_index_product(&m).await?;
            self.refresh_product_category_projection(ctx, entity_id, locale)
                .await?;
            self.refresh_product_attribute_projection(ctx, entity_id, locale)
                .await?;
        }
        Ok(())
    }

    async fn remove_locale(
        &self,
        ctx: &IndexerContext,
        entity_id: Uuid,
        locale: &str,
    ) -> IndexResult<()> {
        let txn = self.db.begin().await?;
        for table in [
            "index_product_attribute_values",
            "index_product_categories",
            "index_products",
        ] {
            txn.execute(Statement::from_sql_and_values(
                self.backend(),
                format!(
                    "DELETE FROM {table} WHERE tenant_id = $1 AND product_id = $2 AND locale = $3"
                ),
                vec![ctx.tenant_id.into(), entity_id.into(), locale.into()],
            ))
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl EventHandler for ProductIndexer {
    fn name(&self) -> &'static str {
        "product_indexer"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(
            event,
            DomainEvent::ProductCreated { .. }
                | DomainEvent::ProductUpdated { .. }
                | DomainEvent::ProductPublished { .. }
                | DomainEvent::ProductDeleted { .. }
                | DomainEvent::ProductAttributeCreated { .. }
                | DomainEvent::ProductAttributeUpdated { .. }
                | DomainEvent::ProductAttributeDeleted { .. }
                | DomainEvent::ProductAttributeOptionCreated { .. }
                | DomainEvent::ProductAttributeOptionUpdated { .. }
                | DomainEvent::ProductAttributeOptionDeleted { .. }
                | DomainEvent::ProductAttributeSchemaCreated { .. }
                | DomainEvent::ProductAttributeSchemaUpdated { .. }
                | DomainEvent::ProductAttributeSchemaDeleted { .. }
                | DomainEvent::ProductAttributeSchemaBindingsChanged { .. }
                | DomainEvent::CatalogCategoryCreated { .. }
                | DomainEvent::CatalogCategoryUpdated { .. }
                | DomainEvent::CatalogCategoryDeleted { .. }
                | DomainEvent::CatalogCategorySchemaModeChanged { .. }
                | DomainEvent::CatalogCategoryAttributesChanged { .. }
                | DomainEvent::ProductPrimaryCategoryChanged { .. }
                | DomainEvent::ProductCategoryAssignmentsChanged { .. }
                | DomainEvent::ProductAttributeValuesChanged { .. }
                | DomainEvent::VariantCreated { .. }
                | DomainEvent::VariantUpdated { .. }
                | DomainEvent::VariantDeleted { .. }
                | DomainEvent::InventoryUpdated { .. }
                | DomainEvent::PriceUpdated { .. }
        ) || matches!(
            event,
            DomainEvent::ReindexRequested { target_type, .. } if target_type == "product"
        )
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let ctx = IndexerContext::new_with_runtime(
            self.db.clone(),
            envelope.tenant_id,
            self.runtime.clone(),
        );

        match &envelope.event {
            DomainEvent::ProductCreated { product_id }
            | DomainEvent::ProductUpdated { product_id }
            | DomainEvent::ProductPublished { product_id } => {
                self.index_one(&ctx, *product_id).await?;
            }

            DomainEvent::ProductDeleted { product_id } => {
                self.remove_one(&ctx, *product_id).await?;
            }

            DomainEvent::ProductPrimaryCategoryChanged { product_id, .. }
            | DomainEvent::ProductCategoryAssignmentsChanged { product_id }
            | DomainEvent::ProductAttributeValuesChanged { product_id } => {
                self.index_one(&ctx, *product_id).await?;
            }

            DomainEvent::ProductAttributeCreated { .. }
            | DomainEvent::ProductAttributeUpdated { .. }
            | DomainEvent::ProductAttributeDeleted { .. }
            | DomainEvent::ProductAttributeOptionCreated { .. }
            | DomainEvent::ProductAttributeOptionUpdated { .. }
            | DomainEvent::ProductAttributeOptionDeleted { .. }
            | DomainEvent::ProductAttributeSchemaCreated { .. }
            | DomainEvent::ProductAttributeSchemaUpdated { .. }
            | DomainEvent::ProductAttributeSchemaDeleted { .. }
            | DomainEvent::ProductAttributeSchemaBindingsChanged { .. }
            | DomainEvent::CatalogCategoryCreated { .. }
            | DomainEvent::CatalogCategoryUpdated { .. }
            | DomainEvent::CatalogCategoryDeleted { .. }
            | DomainEvent::CatalogCategorySchemaModeChanged { .. }
            | DomainEvent::CatalogCategoryAttributesChanged { .. } => {
                self.reindex_all(&ctx).await?;
            }

            DomainEvent::VariantCreated { product_id, .. }
            | DomainEvent::VariantUpdated { product_id, .. }
            | DomainEvent::VariantDeleted { product_id, .. } => {
                self.index_one(&ctx, *product_id).await?;
            }

            DomainEvent::InventoryUpdated { product_id, .. } => {
                self.index_one(&ctx, *product_id).await?;
            }

            DomainEvent::PriceUpdated { product_id, .. } => {
                self.index_one(&ctx, *product_id).await?;
            }

            DomainEvent::ReindexRequested { target_id, .. } => {
                if let Some(id) = target_id {
                    self.index_one(&ctx, *id).await?;
                } else {
                    self.reindex_all(&ctx).await?;
                }
            }

            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod virtual_category_tests {
    use super::*;
    use rustok_product::services::{
        VirtualCategoryAttributeCondition, VirtualCategoryAttributeRule,
    };

    #[test]
    fn matches_all_bounded_v1_predicates() {
        let subtree_id = Uuid::new_v4();
        let facts = VirtualProductFacts {
            status: "active".into(),
            primary_category_id: Some(Uuid::new_v4()),
            in_stock: true,
            price_min: Some(1_000),
            price_max: Some(2_000),
        };
        let rule = VirtualCategoryRuleV1 {
            version: 1,
            statuses: vec!["active".into()],
            primary_category_subtree_id: Some(subtree_id),
            price_min: Some(1_500),
            price_max: Some(2_500),
            in_stock: Some(true),
            attributes: vec![
                VirtualCategoryAttributeRule {
                    code: "brand".into(),
                    condition: VirtualCategoryAttributeCondition::Eq {
                        value: "rustok".into(),
                    },
                },
                VirtualCategoryAttributeRule {
                    code: "weight".into(),
                    condition: VirtualCategoryAttributeCondition::Range {
                        min: Some(Decimal::new(10, 1)),
                        max: Some(Decimal::new(20, 1)),
                    },
                },
            ],
        };
        let ancestors = [subtree_id].into_iter().collect();
        let attributes = vec![
            VirtualAttributeFactRow {
                attribute_code: "brand".into(),
                value_key: Some("rustok".into()),
                value_number: None,
            },
            VirtualAttributeFactRow {
                attribute_code: "weight".into(),
                value_key: Some("1.5".into()),
                value_number: Some(Decimal::new(15, 1)),
            },
        ];

        assert!(virtual_category_rule_matches(
            &rule,
            &facts,
            &ancestors,
            &attributes
        ));
    }

    #[test]
    fn rejects_rule_when_an_effective_attribute_fact_is_absent() {
        let facts = VirtualProductFacts {
            status: "active".into(),
            primary_category_id: None,
            in_stock: false,
            price_min: None,
            price_max: None,
        };
        let rule = VirtualCategoryRuleV1 {
            version: 1,
            statuses: Vec::new(),
            primary_category_subtree_id: None,
            price_min: None,
            price_max: None,
            in_stock: None,
            attributes: vec![VirtualCategoryAttributeRule {
                code: "brand".into(),
                condition: VirtualCategoryAttributeCondition::Eq {
                    value: "rustok".into(),
                },
            }],
        };

        assert!(!virtual_category_rule_matches(
            &rule,
            &facts,
            &Default::default(),
            &[]
        ));
    }
}
