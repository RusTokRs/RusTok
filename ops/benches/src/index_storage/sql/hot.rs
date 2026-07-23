use super::{MutationWorkload, Workload, WorkloadContext};

pub fn prototype_sql() -> String {
    r#"
DROP SCHEMA IF EXISTS idx_bench_hot CASCADE;
CREATE SCHEMA idx_bench_hot;

CREATE TABLE idx_bench_hot.product (
    tenant_id uuid NOT NULL,
    product_id uuid NOT NULL,
    locale text NOT NULL,
    source_version bigint NOT NULL,
    status text NOT NULL,
    title text NOT NULL,
    price_minor bigint NOT NULL,
    rating_milli bigint NOT NULL,
    tags text[] NOT NULL,
    updated_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, product_id, locale)
);

CREATE TABLE idx_bench_hot.variant (
    tenant_id uuid NOT NULL,
    variant_id uuid NOT NULL,
    locale text NOT NULL,
    source_version bigint NOT NULL,
    sku text NOT NULL,
    price_minor bigint NOT NULL,
    updated_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, variant_id, locale)
);

CREATE TABLE idx_bench_hot.sales_channel (
    tenant_id uuid NOT NULL,
    channel_id uuid NOT NULL,
    code text NOT NULL,
    region_code text NOT NULL,
    PRIMARY KEY (tenant_id, channel_id)
);

INSERT INTO idx_bench_hot.product
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
FROM idx_bench_source.product;

INSERT INTO idx_bench_hot.variant
SELECT tenant_id, variant_id, locale, source_version, sku, price_minor, updated_at
FROM idx_bench_source.variant;

INSERT INTO idx_bench_hot.sales_channel
SELECT tenant_id, channel_id, code, region_code
FROM idx_bench_source.channel;

CREATE INDEX product_status
    ON idx_bench_hot.product (tenant_id, locale, status, product_id);
CREATE INDEX product_price
    ON idx_bench_hot.product (tenant_id, locale, price_minor, product_id);
CREATE INDEX product_tags
    ON idx_bench_hot.product USING gin (tags);
CREATE INDEX channel_code
    ON idx_bench_hot.sales_channel (tenant_id, code, channel_id);
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
                "SELECT product_id AS entity_id FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND status = 'published' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "price_range_sort",
            sql: format!(
                "SELECT product_id AS entity_id, price_minor FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND price_minor BETWEEN 20000 AND 80000 ORDER BY price_minor, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "multi_value_tag",
            sql: format!(
                "SELECT product_id AS entity_id FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND tags @> ARRAY['tag-3']::text[] ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "two_hop_channel_filter",
            sql: format!(
                "SELECT DISTINCT product.product_id AS entity_id FROM idx_bench_hot.product AS product JOIN idx_bench_hot.link AS product_variant ON product_variant.tenant_id = product.tenant_id AND product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.product_id AND product_variant.source_locale = product.locale AND product_variant.link_name = 'variants' JOIN idx_bench_hot.link AS variant_channel ON variant_channel.tenant_id = product_variant.tenant_id AND variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id = product_variant.target_entity_id AND variant_channel.source_locale = product_variant.target_locale AND variant_channel.link_name = 'sales_channels' JOIN idx_bench_hot.sales_channel AS channel ON channel.tenant_id = variant_channel.tenant_id AND channel.channel_id = variant_channel.target_entity_id WHERE product.tenant_id = {tenant} AND product.locale = {locale} AND channel.code = 'channel-1' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "keyset_page",
            sql: format!(
                "SELECT product_id AS entity_id, price_minor FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND (price_minor, product_id) > ({anchor_price}, {anchor_id}) ORDER BY price_minor, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "exact_count",
            sql: format!(
                "SELECT count(*)::bigint AS result_count FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND status = 'published'"
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
                "WITH targets AS (SELECT product_id FROM idx_bench_source.product WHERE tenant_no = 1 AND locale = {locale} AND product_no <= {batch}), updated AS (UPDATE idx_bench_hot.product AS product SET source_version = product.source_version + 1, price_minor = product.price_minor + 17, rating_milli = product.rating_milli + 1 FROM targets WHERE product.tenant_id = {tenant} AND product.locale = {locale} AND product.product_id = targets.product_id RETURNING product.product_id) SELECT count(*)::bigint AS affected_entities FROM updated"
            ),
            expected_affected_entities: i64::from(batch),
        },
        MutationWorkload {
            name: "delete_product_batch",
            sql: format!(
                "WITH targets AS (SELECT product_id FROM idx_bench_source.product WHERE tenant_no = 1 AND locale = {locale} AND product_no <= {batch}), deleted_links AS (DELETE FROM idx_bench_hot.link AS link USING targets WHERE link.tenant_id = {tenant} AND link.source_entity = 'product' AND link.source_locale = {locale} AND link.source_entity_id = targets.product_id RETURNING 1), deleted_entities AS (DELETE FROM idx_bench_hot.product AS product USING targets WHERE product.tenant_id = {tenant} AND product.locale = {locale} AND product.product_id = targets.product_id RETURNING product.product_id) SELECT (SELECT count(*) FROM deleted_entities)::bigint AS affected_entities, (SELECT count(*) FROM deleted_links)::bigint AS affected_links, {deleted_links}::bigint AS expected_links"
            ),
            expected_affected_entities: i64::from(batch),
        },
    ]
}
