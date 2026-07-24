use super::{MutationWorkload, Workload, WorkloadContext};

pub fn prototype_sql() -> String {
    r#"
DROP SCHEMA IF EXISTS idx_bench_jsonb CASCADE;
CREATE SCHEMA idx_bench_jsonb;

CREATE TABLE idx_bench_jsonb.entity (
    tenant_id uuid NOT NULL,
    module_name text NOT NULL,
    entity_name text NOT NULL,
    schema_version integer NOT NULL,
    entity_id uuid NOT NULL,
    locale text NOT NULL,
    source_version bigint NOT NULL,
    payload jsonb NOT NULL,
    PRIMARY KEY (
        tenant_id, module_name, entity_name, schema_version, entity_id, locale
    )
);

INSERT INTO idx_bench_jsonb.entity
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
FROM idx_bench_source.product;

INSERT INTO idx_bench_jsonb.entity
SELECT
    tenant_id,
    'product',
    'variant',
    1,
    variant_id,
    locale,
    source_version,
    jsonb_build_object(
        'sku', sku,
        'price_minor', price_minor,
        'product_id', product_id,
        'updated_at', updated_at
    )
FROM idx_bench_source.variant;

INSERT INTO idx_bench_jsonb.entity
SELECT
    tenant_id,
    'channel',
    'sales_channel',
    1,
    channel_id,
    '',
    1,
    jsonb_build_object('code', code, 'region_code', region_code)
FROM idx_bench_source.channel;

CREATE INDEX entity_payload_gin
    ON idx_bench_jsonb.entity USING gin (payload jsonb_path_ops);
CREATE INDEX entity_status
    ON idx_bench_jsonb.entity (
        tenant_id, locale, (payload->>'status'), entity_id
    )
    WHERE module_name = 'product'
      AND entity_name = 'product'
      AND schema_version = 1;
CREATE INDEX entity_price
    ON idx_bench_jsonb.entity (
        tenant_id,
        locale,
        ((payload->>'price_minor')::bigint),
        entity_id
    )
    WHERE module_name = 'product'
      AND entity_name = 'product'
      AND schema_version = 1;
CREATE INDEX entity_channel_code
    ON idx_bench_jsonb.entity (
        tenant_id, (payload->>'code'), entity_id, locale
    )
    WHERE module_name = 'channel'
      AND entity_name = 'sales_channel'
      AND schema_version = 1;
"#
    .to_owned()
}

pub fn workloads(context: &WorkloadContext) -> Vec<Workload> {
    let tenant = context.tenant;
    let locale = &context.locale;
    let anchor_price = context.anchor_price;
    let anchor_id = &context.anchor_id;

    vec![
        Workload {
            name: "status_equality",
            sql: format!(
                "SELECT entity_id FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND module_name = 'product' AND entity_name = 'product' AND schema_version = 1 AND locale = {locale} AND payload->>'status' = 'published' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "price_range_sort",
            sql: format!(
                "SELECT entity_id, (payload->>'price_minor')::bigint AS price_minor FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND module_name = 'product' AND entity_name = 'product' AND schema_version = 1 AND locale = {locale} AND (payload->>'price_minor')::bigint BETWEEN 20000 AND 80000 ORDER BY (payload->>'price_minor')::bigint, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "multi_value_tag",
            sql: format!(
                "SELECT entity_id FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND module_name = 'product' AND entity_name = 'product' AND schema_version = 1 AND locale = {locale} AND payload @> '{{\"tags\":[\"tag-3\"]}}'::jsonb ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "two_hop_channel_filter",
            sql: format!(
                "SELECT DISTINCT product.entity_id AS entity_id FROM idx_bench_jsonb.entity AS product JOIN idx_bench_jsonb.link AS product_variant ON product_variant.tenant_id = product.tenant_id AND product_variant.source_module = product.module_name AND product_variant.source_entity = product.entity_name AND product_variant.source_schema_version = product.schema_version AND product_variant.source_entity_id = product.entity_id AND product_variant.source_locale = product.locale AND product_variant.link_name = 'variants' AND product_variant.target_module = 'product' AND product_variant.target_entity = 'variant' AND product_variant.target_schema_version = 1 JOIN idx_bench_jsonb.link AS variant_channel ON variant_channel.tenant_id = product_variant.tenant_id AND variant_channel.source_module = product_variant.target_module AND variant_channel.source_entity = product_variant.target_entity AND variant_channel.source_schema_version = product_variant.target_schema_version AND variant_channel.source_entity_id = product_variant.target_entity_id AND variant_channel.source_locale = product_variant.target_locale AND variant_channel.link_name = 'sales_channels' AND variant_channel.target_module = 'channel' AND variant_channel.target_entity = 'sales_channel' AND variant_channel.target_schema_version = 1 JOIN idx_bench_jsonb.entity AS channel ON channel.tenant_id = variant_channel.tenant_id AND channel.module_name = variant_channel.target_module AND channel.entity_name = variant_channel.target_entity AND channel.schema_version = variant_channel.target_schema_version AND channel.entity_id = variant_channel.target_entity_id AND channel.locale = variant_channel.target_locale WHERE product.tenant_id = {tenant} AND product.module_name = 'product' AND product.entity_name = 'product' AND product.schema_version = 1 AND product.locale = {locale} AND channel.payload->>'code' = 'channel-1' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "keyset_page",
            sql: format!(
                "SELECT entity_id, (payload->>'price_minor')::bigint AS price_minor FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND module_name = 'product' AND entity_name = 'product' AND schema_version = 1 AND locale = {locale} AND ((payload->>'price_minor')::bigint, entity_id) > ({anchor_price}, {anchor_id}) ORDER BY (payload->>'price_minor')::bigint, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "exact_count",
            sql: format!(
                "SELECT count(*)::bigint AS result_count FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND module_name = 'product' AND entity_name = 'product' AND schema_version = 1 AND locale = {locale} AND payload->>'status' = 'published'"
            ),
        },
    ]
}

pub fn mutation_workloads(context: &WorkloadContext) -> Vec<MutationWorkload> {
    let tenant = context.tenant;
    let locale = &context.locale;
    let batch = context.mutation_batch;
    let deleted_links = context.expected_deleted_links();

    vec![
        MutationWorkload {
            name: "update_product_batch",
            sql: format!(
                "WITH targets AS (SELECT product_id FROM idx_bench_source.product WHERE tenant_no = 1 AND locale = {locale} AND product_no <= {batch}), updated AS (UPDATE idx_bench_jsonb.entity AS entity SET source_version = entity.source_version + 1, payload = jsonb_set(jsonb_set(entity.payload, '{{price_minor}}', to_jsonb((entity.payload->>'price_minor')::bigint + 17), false), '{{rating_milli}}', to_jsonb((entity.payload->>'rating_milli')::bigint + 1), false) FROM targets WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND entity.entity_id = targets.product_id RETURNING entity.entity_id) SELECT count(*)::bigint AS affected_entities, NULL::bigint AS affected_fields, NULL::bigint AS expected_fields, NULL::bigint AS affected_links, NULL::bigint AS expected_links FROM updated"
            ),
            expected_affected_entities: i64::from(batch),
        },
        MutationWorkload {
            name: "delete_product_batch",
            sql: format!(
                "WITH targets AS (SELECT product_id FROM idx_bench_source.product WHERE tenant_no = 1 AND locale = {locale} AND product_no <= {batch}), deleted_links AS (DELETE FROM idx_bench_jsonb.link AS link USING targets WHERE link.tenant_id = {tenant} AND link.source_module = 'product' AND link.source_entity = 'product' AND link.source_schema_version = 1 AND link.source_locale = {locale} AND link.source_entity_id = targets.product_id RETURNING 1), deleted_entities AS (DELETE FROM idx_bench_jsonb.entity AS entity USING targets WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND entity.entity_id = targets.product_id RETURNING entity.entity_id) SELECT (SELECT count(*) FROM deleted_entities)::bigint AS affected_entities, NULL::bigint AS affected_fields, NULL::bigint AS expected_fields, (SELECT count(*) FROM deleted_links)::bigint AS affected_links, {deleted_links}::bigint AS expected_links"
            ),
            expected_affected_entities: i64::from(batch),
        },
    ]
}
