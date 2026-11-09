//! Product-owned read projections for the embedded PostgreSQL Index adapter.
//!
//! Templates with __channel_identity_source__ require the Channel owner's trusted
//! relation with tenant_id and generation columns. The caller composes it before
//! preparing the single PostgreSQL statement.

use sea_orm::{DbBackend, Statement};
use uuid::Uuid;

pub const PRODUCT_VARIANT_ROWS_CTE: &str = r#"
product_variant_index_union AS (
    SELECT
        FALSE AS is_deleted,
        v.tenant_id,
        v.id AS variant_id,
        v.product_id,
        v.index_revision,
        v.sku,
        v.barcode,
        v.shipping_profile_slug,
        v.ean,
        v.upc,
        v.inventory_policy,
        v.inventory_management,
        v.inventory_quantity,
        v.weight_unit,
        v.combination_identity,
        v.position
    FROM product_variants v
    WHERE v.tenant_id = $1

    UNION ALL

    SELECT
        TRUE AS is_deleted,
        tombstone.tenant_id,
        tombstone.variant_id,
        NULL::uuid AS product_id,
        tombstone.source_version AS index_revision,
        NULL::text AS sku,
        NULL::text AS barcode,
        NULL::text AS shipping_profile_slug,
        NULL::text AS ean,
        NULL::text AS upc,
        NULL::text AS inventory_policy,
        NULL::text AS inventory_management,
        NULL::integer AS inventory_quantity,
        NULL::text AS weight_unit,
        NULL::text AS combination_identity,
        NULL::integer AS position
    FROM product_variant_index_tombstones tombstone
    WHERE tombstone.tenant_id = $1
),
product_variant_index_rows AS (
    SELECT
        row.*,
        COUNT(*) OVER (
            PARTITION BY row.tenant_id, row.variant_id
        ) AS identity_count
    FROM product_variant_index_union row
)
"#;

pub const PRODUCT_VARIANT_ROW_SELECT: &str = r#"
SELECT
    row.is_deleted,
    row.identity_count,
    row.tenant_id,
    row.variant_id,
    row.product_id,
    row.index_revision,
    row.sku,
    row.barcode,
    row.shipping_profile_slug,
    row.ean,
    row.upc,
    row.inventory_policy,
    row.inventory_management,
    row.inventory_quantity,
    row.weight_unit,
    row.combination_identity,
    row.position
FROM product_variant_index_rows row
"#;

/// PostgreSQL CTE fragment used by the replacement Product source to materialize every active,
/// Product-scoped, filterable EAV value into the Product-owned canonical term grammar.
///
/// The fragment is tenant-scoped by `$1` and intentionally aggregates terms by Product rather than by
/// Product translation locale. Localized text terms carry their own canonical locale identity, so one
/// localized Product record can express the exact requested-locale/fallback-locale predicate without a
/// dynamic Index schema field per attribute code.
pub const PRODUCT_ATTRIBUTE_TERMS_CTE: &str = r#"
product_filterable_attribute_values AS (
    SELECT
        pav.id AS value_id,
        pav.product_id,
        pa.id AS attribute_id,
        pa.value_type,
        pa.is_localized,
        pav.value_text,
        pav.value_integer,
        pav.value_decimal,
        pav.value_boolean,
        pav.value_date,
        pav.value_datetime
    FROM product_attribute_values pav
    JOIN product_attributes pa
      ON pa.id = pav.attribute_id
     AND pa.tenant_id = pav.tenant_id
    WHERE pav.tenant_id = $1
      AND pav.detached_at IS NULL
      AND pa.archived_at IS NULL
      AND pa.is_filterable = TRUE
      AND pa.scope IN ('product', 'both')
),
product_attribute_term_rows AS (
    SELECT
        value.product_id,
        value.attribute_id::text || '|text||'
            || encode(convert_to(value.value_text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type IN ('text', 'textarea', 'richtext')
      AND value.is_localized = FALSE
      AND value.value_text IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|localized_text|'
            || encode(convert_to(translation.locale, 'UTF8'), 'hex') || '|'
            || encode(convert_to(translation.value_text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    JOIN product_attribute_value_translations translation
      ON translation.value_id = value.value_id
    WHERE value.value_type IN ('text', 'textarea', 'richtext')
      AND value.is_localized = TRUE
      AND translation.value_text IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|localized_present|'
            || encode(convert_to(translation.locale, 'UTF8'), 'hex') || '|' AS term
    FROM product_filterable_attribute_values value
    JOIN product_attribute_value_translations translation
      ON translation.value_id = value.value_id
    WHERE value.value_type IN ('text', 'textarea', 'richtext')
      AND value.is_localized = TRUE

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|integer||'
            || encode(convert_to(value.value_integer::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'integer'
      AND value.value_integer IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|decimal||'
            || encode(convert_to(trim_scale(value.value_decimal)::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'decimal'
      AND value.value_decimal IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|boolean||'
            || encode(
                convert_to(CASE WHEN value.value_boolean THEN 'true' ELSE 'false' END, 'UTF8'),
                'hex'
            ) AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'boolean'
      AND value.value_boolean IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|date||'
            || encode(convert_to(value.value_date::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'date'
      AND value.value_date IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|datetime||'
            || encode(
                convert_to(
                    ((extract(epoch FROM value.value_datetime) * 1000000)::bigint)::text,
                    'UTF8'
                ),
                'hex'
            ) AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'datetime'
      AND value.value_datetime IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|option||'
            || encode(convert_to(option_value.option_id::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    JOIN product_attribute_value_options option_value
      ON option_value.value_id = value.value_id
     AND option_value.tenant_id = $1
    WHERE value.value_type IN ('select', 'multiselect')
),
product_attribute_terms AS (
    SELECT
        product_id,
        jsonb_agg(term ORDER BY term) AS attribute_terms
    FROM (
        SELECT DISTINCT product_id, term
        FROM product_attribute_term_rows
    ) canonical_term
    GROUP BY product_id
)
"#;

pub const PRODUCT_ROWS_CTE: &str = r#"
product_tag_ids AS (
    SELECT
        product_tag.product_id,
        jsonb_agg(product_tag.term_id ORDER BY product_tag.term_id) AS tag_ids
    FROM product_tags product_tag
    WHERE product_tag.tenant_id = $1
    GROUP BY product_tag.product_id
),
channel_identity_generation AS (
    SELECT COALESCE(
        (
            SELECT generation
            FROM __channel_identity_source__ AS channel_generation_source
            WHERE tenant_id = $1
        ),
        0
    )::bigint AS generation
),
product_graph_projection AS (
    SELECT DISTINCT ON (projection.tenant_id, projection.product_id)
        projection.tenant_id,
        projection.product_id,
        projection.projection_epoch,
        projection.product_source_version,
        projection.relation_epoch,
        relation.channel_ids,
        freshness.product_source_version AS freshness_product_source_version,
        freshness.visibility_key AS freshness_visibility_key,
        freshness.channel_identity_generation AS freshness_channel_identity_generation,
        channel_generation.generation AS current_channel_identity_generation
    FROM product_index_graph_projection_snapshots projection
    JOIN product_sales_channel_index_relation_snapshots relation
      ON relation.tenant_id = projection.tenant_id
     AND relation.product_id = projection.product_id
     AND relation.relation_epoch = projection.relation_epoch
    LEFT JOIN LATERAL (
        SELECT
            witness.product_source_version,
            witness.visibility_key,
            witness.channel_identity_generation
        FROM product_sales_channel_index_relation_freshness_snapshots witness
        WHERE witness.tenant_id = projection.tenant_id
          AND witness.product_id = projection.product_id
          AND witness.relation_epoch = projection.relation_epoch
        ORDER BY witness.sequence_no DESC
        LIMIT 1
    ) freshness ON TRUE
    CROSS JOIN channel_identity_generation channel_generation
    WHERE projection.tenant_id = $1
    ORDER BY projection.tenant_id, projection.product_id, projection.projection_epoch DESC
),
product_index_union AS (
    SELECT
        FALSE AS is_deleted,
        p.tenant_id,
        p.id AS product_id,
        projection.projection_epoch AS source_version,
        p.index_revision AS observed_product_source_version,
        projection.product_source_version AS projected_product_source_version,
        projection.relation_epoch,
        projection.freshness_product_source_version,
        projection.freshness_visibility_key,
        projection.freshness_channel_identity_generation,
        projection.current_channel_identity_generation,
        p.metadata,
        p.status::text AS status,
        p.seller_id,
        p.vendor,
        p.product_type,
        p.primary_category_id,
        p.created_at,
        p.published_at,
        t.locale,
        t.title,
        t.handle,
        t.description,
        COALESCE(tags.tag_ids, '[]'::jsonb) AS tag_ids,
        COALESCE(attributes.attribute_terms, '[]'::jsonb) AS attribute_terms,
        COALESCE(
            (
                SELECT jsonb_agg(v.id ORDER BY v.id)
                FROM product_variants v
                WHERE v.tenant_id = p.tenant_id
                  AND v.product_id = p.id
            ),
            '[]'::jsonb
        ) AS variant_ids,
        projection.channel_ids AS sales_channel_ids
    FROM products p
    JOIN product_translations t
      ON t.product_id = p.id
     AND t.tenant_id = p.tenant_id
    LEFT JOIN product_graph_projection projection
      ON projection.tenant_id = p.tenant_id
     AND projection.product_id = p.id
    LEFT JOIN product_tag_ids tags
      ON tags.product_id = p.id
    LEFT JOIN product_attribute_terms attributes
      ON attributes.product_id = p.id
    WHERE p.tenant_id = $1

    UNION ALL

    SELECT
        TRUE AS is_deleted,
        tombstone.tenant_id,
        tombstone.product_id,
        projection.projection_epoch AS source_version,
        tombstone.source_version AS observed_product_source_version,
        projection.product_source_version AS projected_product_source_version,
        projection.relation_epoch,
        projection.freshness_product_source_version,
        projection.freshness_visibility_key,
        projection.freshness_channel_identity_generation,
        projection.current_channel_identity_generation,
        NULL::jsonb AS metadata,
        NULL::text AS status,
        NULL::text AS seller_id,
        NULL::text AS vendor,
        NULL::text AS product_type,
        NULL::uuid AS primary_category_id,
        NULL::timestamptz AS created_at,
        NULL::timestamptz AS published_at,
        tombstone.locale,
        NULL::text AS title,
        NULL::text AS handle,
        NULL::text AS description,
        '[]'::jsonb AS tag_ids,
        '[]'::jsonb AS attribute_terms,
        '[]'::jsonb AS variant_ids,
        projection.channel_ids AS sales_channel_ids
    FROM product_index_tombstones tombstone
    LEFT JOIN product_graph_projection projection
      ON projection.tenant_id = tombstone.tenant_id
     AND projection.product_id = tombstone.product_id
    WHERE tombstone.tenant_id = $1
),
product_index_rows AS (
    SELECT
        row.*,
        COUNT(*) OVER (
            PARTITION BY row.tenant_id, row.product_id, row.locale
        ) AS identity_count
    FROM product_index_union row
)
"#;

pub const PRODUCT_ROW_SELECT: &str = r#"
SELECT
    row.is_deleted,
    row.identity_count,
    row.tenant_id,
    row.product_id,
    row.source_version,
    row.observed_product_source_version,
    row.projected_product_source_version,
    row.relation_epoch,
    row.freshness_product_source_version,
    row.freshness_visibility_key,
    row.freshness_channel_identity_generation,
    row.current_channel_identity_generation,
    row.metadata,
    row.status,
    row.seller_id,
    row.vendor,
    row.product_type,
    row.primary_category_id,
    row.created_at,
    row.published_at,
    row.locale,
    row.title,
    row.handle,
    row.description,
    row.tag_ids,
    row.attribute_terms,
    row.variant_ids,
    row.sales_channel_ids
FROM product_index_rows row
"#;

pub const DUE_TENANT_SQL: &str = r#"
SELECT
    state.tenant_id,
    COALESCE(channel_generation.generation, 0)::bigint AS current_channel_identity_generation
FROM product_sales_channel_index_relation_convergence_state state
LEFT JOIN __channel_identity_source__ channel_generation
  ON channel_generation.tenant_id = state.tenant_id
WHERE state.available_at <= CURRENT_TIMESTAMP
  AND (state.lease_token IS NULL OR state.lease_expires_at <= CURRENT_TIMESTAMP)
  AND (
      state.sweep_generation IS NOT NULL
      OR state.channel_identity_generation IS NULL
      OR state.channel_identity_generation < COALESCE(channel_generation.generation, 0)
      OR EXISTS (
          SELECT 1
          FROM product_sales_channel_index_relation_convergence_requests request
          WHERE request.tenant_id = state.tenant_id
            AND request.sequence_no > state.visibility_cursor
      )
  )
ORDER BY state.available_at, state.updated_at, state.tenant_id
LIMIT 1
"#;

pub const PRODUCT_QUERY_MATERIALIZED_FRESHNESS: &str = r#"
EXISTS (
    SELECT 1
    FROM products owner_product
    JOIN LATERAL (
        SELECT
            projection.projection_epoch,
            projection.product_source_version,
            projection.relation_epoch
        FROM product_index_graph_projection_snapshots projection
        WHERE projection.tenant_id = {{entity}}.tenant_id
          AND projection.product_id = {{entity}}.entity_id
        ORDER BY projection.projection_epoch DESC
        LIMIT 1
    ) current_projection ON TRUE
    JOIN LATERAL (
        SELECT
            witness.product_source_version,
            witness.channel_identity_generation
        FROM product_sales_channel_index_relation_freshness_snapshots witness
        WHERE witness.tenant_id = {{entity}}.tenant_id
          AND witness.product_id = {{entity}}.entity_id
          AND witness.relation_epoch = current_projection.relation_epoch
        ORDER BY witness.sequence_no DESC
        LIMIT 1
    ) current_freshness ON TRUE
    WHERE owner_product.tenant_id = {{entity}}.tenant_id
      AND owner_product.id = {{entity}}.entity_id
      AND {{entity}}.source_version = current_projection.projection_epoch
      AND current_projection.product_source_version = owner_product.index_revision
      AND current_freshness.product_source_version <= owner_product.index_revision
      AND current_freshness.channel_identity_generation = COALESCE(
          (
              SELECT channel_generation.generation
              FROM __channel_identity_source__ channel_generation
              WHERE channel_generation.tenant_id = {{entity}}.tenant_id
          ),
          0
      )
      AND NOT EXISTS (
          SELECT 1
          FROM product_sales_channel_index_relation_convergence_requests request
          WHERE request.tenant_id = {{entity}}.tenant_id
            AND request.product_id = {{entity}}.entity_id
            AND request.product_source_version > current_freshness.product_source_version
      )
      AND EXISTS (
          SELECT 1
          FROM product_translations translation
          WHERE translation.tenant_id = {{entity}}.tenant_id
            AND translation.product_id = {{entity}}.entity_id
            AND translation.locale = {{entity}}.locale_key
      )
)
"#;

pub const PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS: &str = r#"
EXISTS (
    SELECT 1
    FROM product_variants owner_variant
    WHERE owner_variant.tenant_id = {{entity}}.tenant_id
      AND owner_variant.id = {{entity}}.entity_id
      AND owner_variant.index_revision = {{entity}}.source_version
)
"#;

pub const PRODUCT_LOCALE_ABSENCE_SQL: &str = r#"
SELECT
    CAST(projection.projection_epoch AS TEXT) AS source_version_text,
    product.index_revision AS observed_product_source_version,
    product.metadata,
    freshness.product_source_version AS freshness_product_source_version,
    freshness.visibility_key AS freshness_visibility_key,
    freshness.channel_identity_generation AS freshness_channel_identity_generation,
    COALESCE(
        (
            SELECT generation
            FROM __channel_identity_source__ AS channel_generation_source
            WHERE tenant_id = product.tenant_id
        ),
        0
    )::bigint AS current_channel_identity_generation
FROM products product
JOIN LATERAL (
    SELECT
        projection.projection_epoch,
        projection.product_source_version,
        projection.relation_epoch
    FROM product_index_graph_projection_snapshots projection
    WHERE projection.tenant_id = product.tenant_id
      AND projection.product_id = product.id
    ORDER BY projection.projection_epoch DESC
    LIMIT 1
) projection
  ON projection.product_source_version = product.index_revision
JOIN product_sales_channel_index_relation_snapshots relation
  ON relation.tenant_id = product.tenant_id
 AND relation.product_id = product.id
 AND relation.relation_epoch = projection.relation_epoch
JOIN LATERAL (
    SELECT
        witness.product_source_version,
        witness.visibility_key,
        witness.channel_identity_generation
    FROM product_sales_channel_index_relation_freshness_snapshots witness
    WHERE witness.tenant_id = product.tenant_id
      AND witness.product_id = product.id
      AND witness.relation_epoch = projection.relation_epoch
    ORDER BY witness.sequence_no DESC
    LIMIT 1
) freshness ON TRUE
WHERE product.tenant_id = $1
  AND product.id = $2
  AND NOT EXISTS (
      SELECT 1
      FROM product_translations translation
      WHERE translation.tenant_id = product.tenant_id
        AND translation.product_id = product.id
        AND translation.locale = $3
  )
  AND NOT EXISTS (
      SELECT 1
      FROM product_index_tombstones tombstone
      WHERE tombstone.tenant_id = product.tenant_id
        AND tombstone.product_id = product.id
        AND tombstone.locale = $3
  )
LIMIT 1
"#;

/// Stable identity page and visibility row for relation convergence.
pub fn relation_product_ids(
    tenant_id: Uuid,
    after_product_id: Option<Uuid>,
    limit: i64,
) -> Statement {
    let (sql, values) = match after_product_id {
        Some(after_product_id) => (
            "SELECT id FROM products WHERE tenant_id = $1 AND id > $2 ORDER BY id ASC LIMIT $3",
            vec![tenant_id.into(), after_product_id.into(), limit.into()],
        ),
        None => (
            "SELECT id FROM products WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2",
            vec![tenant_id.into(), limit.into()],
        ),
    };
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}

pub fn relation_visibility(tenant_id: Uuid, product_id: Uuid) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT metadata, index_revision FROM products WHERE tenant_id = $1 AND id = $2",
        [tenant_id.into(), product_id.into()],
    )
}
