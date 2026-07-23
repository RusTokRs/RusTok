use super::{Prototype, WorkloadContext};

pub fn churn_cycle_sql(prototype: Prototype, context: &WorkloadContext) -> String {
    match prototype {
        Prototype::Jsonb => jsonb_cycle(context),
        Prototype::TypedEav => eav_cycle(context),
        Prototype::HotProjection => hot_cycle(context),
    }
}

fn jsonb_cycle(context: &WorkloadContext) -> String {
    let tenant = context.tenant;
    let locale = &context.locale;
    let update_end = context.mutation_batch;
    let churn_start = context.churn_first_product;

    format!(
        r#"
UPDATE idx_bench_jsonb.entity AS entity
SET source_version = entity.source_version + 1,
    payload = jsonb_set(
        jsonb_set(
            entity.payload,
            '{{price_minor}}',
            to_jsonb((entity.payload->>'price_minor')::bigint + 17),
            false
        ),
        '{{rating_milli}}',
        to_jsonb((entity.payload->>'rating_milli')::bigint + 1),
        false
    )
FROM idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no <= {update_end}
  AND entity.tenant_id = {tenant}
  AND entity.entity_name = 'product'
  AND entity.locale = source.locale
  AND entity.entity_id = source.product_id;

DELETE FROM idx_bench_jsonb.link AS link
USING idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no >= {churn_start}
  AND link.tenant_id = {tenant}
  AND link.source_entity = 'product'
  AND link.source_locale = source.locale
  AND link.source_entity_id = source.product_id;

DELETE FROM idx_bench_jsonb.entity AS entity
USING idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no >= {churn_start}
  AND entity.tenant_id = {tenant}
  AND entity.entity_name = 'product'
  AND entity.locale = source.locale
  AND entity.entity_id = source.product_id;

INSERT INTO idx_bench_jsonb.entity (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    source_version, payload
)
SELECT
    tenant_id,
    'product',
    'product',
    1,
    product_id,
    locale,
    source_version,
    jsonb_build_object(
        'status', status,
        'title', title,
        'price_minor', price_minor,
        'rating_milli', rating_milli,
        'tags', to_jsonb(tags),
        'updated_at', updated_at
    )
FROM idx_bench_source.product
WHERE tenant_no = 1
  AND locale = {locale}
  AND product_no >= {churn_start};

INSERT INTO idx_bench_jsonb.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name,
    ordinal, target_entity, target_entity_id, target_locale
)
SELECT
    tenant_id,
    'product',
    product_id,
    locale,
    'variants',
    (variant_no - 1)::smallint,
    'variant',
    variant_id,
    locale
FROM idx_bench_source.variant
WHERE tenant_no = 1
  AND locale = {locale}
  AND product_no >= {churn_start};
"#
    )
}

fn eav_cycle(context: &WorkloadContext) -> String {
    let tenant = context.tenant;
    let locale = &context.locale;
    let update_end = context.mutation_batch;
    let churn_start = context.churn_first_product;

    format!(
        r#"
UPDATE idx_bench_eav.entity AS entity
SET source_version = entity.source_version + 1
FROM idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no <= {update_end}
  AND entity.tenant_id = {tenant}
  AND entity.entity_name = 'product'
  AND entity.locale = source.locale
  AND entity.entity_id = source.product_id;

UPDATE idx_bench_eav.field_value AS field
SET value_int = field.value_int
    + CASE field.field_name WHEN 'price_minor' THEN 17 ELSE 1 END
FROM idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no <= {update_end}
  AND field.tenant_id = {tenant}
  AND field.entity_name = 'product'
  AND field.locale = source.locale
  AND field.entity_id = source.product_id
  AND field.field_name IN ('price_minor', 'rating_milli');

DELETE FROM idx_bench_eav.field_value AS field
USING idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no >= {churn_start}
  AND field.tenant_id = {tenant}
  AND field.entity_name = 'product'
  AND field.locale = source.locale
  AND field.entity_id = source.product_id;

DELETE FROM idx_bench_eav.link AS link
USING idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no >= {churn_start}
  AND link.tenant_id = {tenant}
  AND link.source_entity = 'product'
  AND link.source_locale = source.locale
  AND link.source_entity_id = source.product_id;

DELETE FROM idx_bench_eav.entity AS entity
USING idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no >= {churn_start}
  AND entity.tenant_id = {tenant}
  AND entity.entity_name = 'product'
  AND entity.locale = source.locale
  AND entity.entity_id = source.product_id;

INSERT INTO idx_bench_eav.entity (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    source_version
)
SELECT tenant_id, 'product', 'product', 1, product_id, locale, source_version
FROM idx_bench_source.product
WHERE tenant_no = 1
  AND locale = {locale}
  AND product_no >= {churn_start};

INSERT INTO idx_bench_eav.field_value (
    tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_text
)
SELECT tenant_id, 'product', product_id, locale, 'status', 0, status
FROM idx_bench_source.product
WHERE tenant_no = 1 AND locale = {locale} AND product_no >= {churn_start}
UNION ALL
SELECT tenant_id, 'product', product_id, locale, 'title', 0, title
FROM idx_bench_source.product
WHERE tenant_no = 1 AND locale = {locale} AND product_no >= {churn_start};

INSERT INTO idx_bench_eav.field_value (
    tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_int
)
SELECT tenant_id, 'product', product_id, locale, 'price_minor', 0, price_minor
FROM idx_bench_source.product
WHERE tenant_no = 1 AND locale = {locale} AND product_no >= {churn_start}
UNION ALL
SELECT tenant_id, 'product', product_id, locale, 'rating_milli', 0, rating_milli
FROM idx_bench_source.product
WHERE tenant_no = 1 AND locale = {locale} AND product_no >= {churn_start};

INSERT INTO idx_bench_eav.field_value (
    tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_ts
)
SELECT tenant_id, 'product', product_id, locale, 'updated_at', 0, updated_at
FROM idx_bench_source.product
WHERE tenant_no = 1 AND locale = {locale} AND product_no >= {churn_start};

INSERT INTO idx_bench_eav.field_value (
    tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_text
)
SELECT
    product.tenant_id,
    'product',
    product.product_id,
    product.locale,
    'tags',
    tag.ordinality::integer - 1,
    tag.value
FROM idx_bench_source.product AS product
CROSS JOIN LATERAL unnest(product.tags) WITH ORDINALITY AS tag(value, ordinality)
WHERE product.tenant_no = 1
  AND product.locale = {locale}
  AND product.product_no >= {churn_start};

INSERT INTO idx_bench_eav.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name,
    ordinal, target_entity, target_entity_id, target_locale
)
SELECT
    tenant_id,
    'product',
    product_id,
    locale,
    'variants',
    (variant_no - 1)::smallint,
    'variant',
    variant_id,
    locale
FROM idx_bench_source.variant
WHERE tenant_no = 1
  AND locale = {locale}
  AND product_no >= {churn_start};
"#
    )
}

fn hot_cycle(context: &WorkloadContext) -> String {
    let tenant = context.tenant;
    let locale = &context.locale;
    let update_end = context.mutation_batch;
    let churn_start = context.churn_first_product;

    format!(
        r#"
UPDATE idx_bench_hot.product AS product
SET source_version = product.source_version + 1,
    price_minor = product.price_minor + 17,
    rating_milli = product.rating_milli + 1
FROM idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no <= {update_end}
  AND product.tenant_id = {tenant}
  AND product.locale = source.locale
  AND product.product_id = source.product_id;

DELETE FROM idx_bench_hot.link AS link
USING idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no >= {churn_start}
  AND link.tenant_id = {tenant}
  AND link.source_entity = 'product'
  AND link.source_locale = source.locale
  AND link.source_entity_id = source.product_id;

DELETE FROM idx_bench_hot.product AS product
USING idx_bench_source.product AS source
WHERE source.tenant_no = 1
  AND source.locale = {locale}
  AND source.product_no >= {churn_start}
  AND product.tenant_id = {tenant}
  AND product.locale = source.locale
  AND product.product_id = source.product_id;

INSERT INTO idx_bench_hot.product (
    tenant_id, product_id, locale, source_version, status, title, price_minor,
    rating_milli, tags, updated_at
)
SELECT
    tenant_id,
    product_id,
    locale,
    source_version,
    status,
    title,
    price_minor,
    rating_milli,
    tags,
    updated_at
FROM idx_bench_source.product
WHERE tenant_no = 1
  AND locale = {locale}
  AND product_no >= {churn_start};

INSERT INTO idx_bench_hot.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name,
    ordinal, target_entity, target_entity_id, target_locale
)
SELECT
    tenant_id,
    'product',
    product_id,
    locale,
    'variants',
    (variant_no - 1)::smallint,
    'variant',
    variant_id,
    locale
FROM idx_bench_source.variant
WHERE tenant_no = 1
  AND locale = {locale}
  AND product_no >= {churn_start};
"#
    )
}
