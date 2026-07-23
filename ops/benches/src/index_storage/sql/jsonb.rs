use super::{Workload, WorkloadContext};

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
        tenant_id, entity_name, locale, (payload->>'status'), entity_id
    )
    WHERE entity_name = 'product';
CREATE INDEX entity_price
    ON idx_bench_jsonb.entity (
        tenant_id,
        entity_name,
        locale,
        ((payload->>'price_minor')::bigint),
        entity_id
    )
    WHERE entity_name = 'product';
CREATE INDEX entity_channel_code
    ON idx_bench_jsonb.entity (
        tenant_id, entity_name, (payload->>'code'), entity_id
    )
    WHERE entity_name = 'sales_channel';
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
                "SELECT entity_id FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND entity_name = 'product' AND locale = {locale} AND payload->>'status' = 'published' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "price_range_sort",
            sql: format!(
                "SELECT entity_id, (payload->>'price_minor')::bigint AS price_minor FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND entity_name = 'product' AND locale = {locale} AND (payload->>'price_minor')::bigint BETWEEN 20000 AND 80000 ORDER BY (payload->>'price_minor')::bigint, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "multi_value_tag",
            sql: format!(
                "SELECT entity_id FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND entity_name = 'product' AND locale = {locale} AND payload @> '{{\"tags\":[\"tag-3\"]}}'::jsonb ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "two_hop_channel_filter",
            sql: format!(
                "SELECT DISTINCT product.entity_id AS entity_id FROM idx_bench_jsonb.entity AS product JOIN idx_bench_jsonb.link AS product_variant ON product_variant.tenant_id = product.tenant_id AND product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.entity_id AND product_variant.source_locale = product.locale AND product_variant.link_name = 'variants' JOIN idx_bench_jsonb.link AS variant_channel ON variant_channel.tenant_id = product_variant.tenant_id AND variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id = product_variant.target_entity_id AND variant_channel.source_locale = product_variant.target_locale AND variant_channel.link_name = 'sales_channels' JOIN idx_bench_jsonb.entity AS channel ON channel.tenant_id = variant_channel.tenant_id AND channel.entity_name = 'sales_channel' AND channel.entity_id = variant_channel.target_entity_id AND channel.locale = variant_channel.target_locale WHERE product.tenant_id = {tenant} AND product.entity_name = 'product' AND product.locale = {locale} AND channel.payload->>'code' = 'channel-1' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "keyset_page",
            sql: format!(
                "SELECT entity_id, (payload->>'price_minor')::bigint AS price_minor FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND entity_name = 'product' AND locale = {locale} AND ((payload->>'price_minor')::bigint, entity_id) > ({anchor_price}, {anchor_id}) ORDER BY (payload->>'price_minor')::bigint, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "exact_count",
            sql: format!(
                "SELECT count(*)::bigint AS result_count FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND entity_name = 'product' AND locale = {locale} AND payload->>'status' = 'published'"
            ),
        },
    ]
}
