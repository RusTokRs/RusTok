use super::{MutationWorkload, Workload, WorkloadContext};

pub fn prototype_sql() -> String {
    r#"
DROP SCHEMA IF EXISTS idx_bench_eav CASCADE;
CREATE SCHEMA idx_bench_eav;

CREATE TABLE idx_bench_eav.entity (
    tenant_id uuid NOT NULL,
    module_name text NOT NULL,
    entity_name text NOT NULL,
    schema_version integer NOT NULL,
    entity_id uuid NOT NULL,
    locale text NOT NULL,
    source_version bigint NOT NULL,
    PRIMARY KEY (
        tenant_id, module_name, entity_name, schema_version, entity_id, locale
    )
);

CREATE TABLE idx_bench_eav.field_value (
    tenant_id uuid NOT NULL,
    module_name text NOT NULL,
    entity_name text NOT NULL,
    schema_version integer NOT NULL,
    entity_id uuid NOT NULL,
    locale text NOT NULL,
    field_name text NOT NULL,
    ordinal integer NOT NULL,
    value_bool boolean,
    value_int bigint,
    value_numeric numeric,
    value_text text,
    value_uuid uuid,
    value_ts timestamptz,
    CHECK (
        num_nonnulls(
            value_bool, value_int, value_numeric, value_text, value_uuid, value_ts
        ) = 1
    ),
    PRIMARY KEY (
        tenant_id, module_name, entity_name, schema_version, entity_id, locale,
        field_name, ordinal
    ),
    FOREIGN KEY (
        tenant_id, module_name, entity_name, schema_version, entity_id, locale
    ) REFERENCES idx_bench_eav.entity (
        tenant_id, module_name, entity_name, schema_version, entity_id, locale
    ) ON DELETE CASCADE
);

INSERT INTO idx_bench_eav.entity
SELECT tenant_id, 'product', 'product', 1, product_id, locale, source_version
FROM idx_bench_source.product;
INSERT INTO idx_bench_eav.entity
SELECT tenant_id, 'product', 'variant', 1, variant_id, locale, source_version
FROM idx_bench_source.variant;
INSERT INTO idx_bench_eav.entity
SELECT tenant_id, 'channel', 'sales_channel', 1, channel_id, '', 1
FROM idx_bench_source.channel;

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_text
)
SELECT tenant_id, 'product', 'product', 1, product_id, locale, 'status', 0, status
FROM idx_bench_source.product
UNION ALL
SELECT tenant_id, 'product', 'product', 1, product_id, locale, 'title', 0, title
FROM idx_bench_source.product;

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_int
)
SELECT tenant_id, 'product', 'product', 1, product_id, locale, 'price_minor', 0, price_minor
FROM idx_bench_source.product
UNION ALL
SELECT tenant_id, 'product', 'product', 1, product_id, locale, 'rating_milli', 0, rating_milli
FROM idx_bench_source.product;

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_ts
)
SELECT tenant_id, 'product', 'product', 1, product_id, locale, 'updated_at', 0, updated_at
FROM idx_bench_source.product;

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_text
)
SELECT
    product.tenant_id,
    'product',
    'product',
    1,
    product.product_id,
    product.locale,
    'tags',
    tag.ordinality::integer - 1,
    tag.value
FROM idx_bench_source.product AS product
CROSS JOIN LATERAL unnest(product.tags) WITH ORDINALITY AS tag(value, ordinality);

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_text
)
SELECT tenant_id, 'product', 'variant', 1, variant_id, locale, 'sku', 0, sku
FROM idx_bench_source.variant;

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_int
)
SELECT tenant_id, 'product', 'variant', 1, variant_id, locale, 'price_minor', 0, price_minor
FROM idx_bench_source.variant;

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_uuid
)
SELECT tenant_id, 'product', 'variant', 1, variant_id, locale, 'product_id', 0, product_id
FROM idx_bench_source.variant;

INSERT INTO idx_bench_eav.field_value (
    tenant_id, module_name, entity_name, schema_version, entity_id, locale,
    field_name, ordinal, value_text
)
SELECT tenant_id, 'channel', 'sales_channel', 1, channel_id, '', 'code', 0, code
FROM idx_bench_source.channel
UNION ALL
SELECT tenant_id, 'channel', 'sales_channel', 1, channel_id, '', 'region_code', 0, region_code
FROM idx_bench_source.channel;

CREATE INDEX field_text_lookup
    ON idx_bench_eav.field_value (
        tenant_id, module_name, entity_name, schema_version, locale, field_name,
        value_text, entity_id
    )
    WHERE value_text IS NOT NULL;
CREATE INDEX field_int_lookup
    ON idx_bench_eav.field_value (
        tenant_id, module_name, entity_name, schema_version, locale, field_name,
        value_int, entity_id
    )
    WHERE value_int IS NOT NULL;
CREATE INDEX field_uuid_lookup
    ON idx_bench_eav.field_value (
        tenant_id, module_name, entity_name, schema_version, locale, field_name,
        value_uuid, entity_id
    )
    WHERE value_uuid IS NOT NULL;
CREATE INDEX field_ts_lookup
    ON idx_bench_eav.field_value (
        tenant_id, module_name, entity_name, schema_version, locale, field_name,
        value_ts, entity_id
    )
    WHERE value_ts IS NOT NULL;
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
                "SELECT entity.entity_id AS entity_id FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS status ON status.tenant_id = entity.tenant_id AND status.module_name = entity.module_name AND status.entity_name = entity.entity_name AND status.schema_version = entity.schema_version AND status.entity_id = entity.entity_id AND status.locale = entity.locale AND status.field_name = 'status' WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND status.value_text = 'published' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "price_range_sort",
            sql: format!(
                "SELECT entity.entity_id AS entity_id, price.value_int AS price_minor FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS price ON price.tenant_id = entity.tenant_id AND price.module_name = entity.module_name AND price.entity_name = entity.entity_name AND price.schema_version = entity.schema_version AND price.entity_id = entity.entity_id AND price.locale = entity.locale AND price.field_name = 'price_minor' WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND price.value_int BETWEEN 20000 AND 80000 ORDER BY price_minor, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "multi_value_tag",
            sql: format!(
                "SELECT DISTINCT entity.entity_id AS entity_id FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS tag ON tag.tenant_id = entity.tenant_id AND tag.module_name = entity.module_name AND tag.entity_name = entity.entity_name AND tag.schema_version = entity.schema_version AND tag.entity_id = entity.entity_id AND tag.locale = entity.locale AND tag.field_name = 'tags' WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND tag.value_text = 'tag-3' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "two_hop_channel_filter",
            sql: format!(
                "SELECT DISTINCT product.entity_id AS entity_id FROM idx_bench_eav.entity AS product JOIN idx_bench_eav.link AS product_variant ON product_variant.tenant_id = product.tenant_id AND product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.entity_id AND product_variant.source_locale = product.locale AND product_variant.link_name = 'variants' AND product_variant.target_entity = 'variant' JOIN idx_bench_eav.link AS variant_channel ON variant_channel.tenant_id = product_variant.tenant_id AND variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id = product_variant.target_entity_id AND variant_channel.source_locale = product_variant.target_locale AND variant_channel.link_name = 'sales_channels' AND variant_channel.target_entity = 'sales_channel' JOIN idx_bench_eav.field_value AS channel_code ON channel_code.tenant_id = variant_channel.tenant_id AND channel_code.module_name = 'channel' AND channel_code.entity_name = 'sales_channel' AND channel_code.schema_version = 1 AND channel_code.entity_id = variant_channel.target_entity_id AND channel_code.locale = variant_channel.target_locale AND channel_code.field_name = 'code' WHERE product.tenant_id = {tenant} AND product.module_name = 'product' AND product.entity_name = 'product' AND product.schema_version = 1 AND product.locale = {locale} AND channel_code.value_text = 'channel-1' ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "keyset_page",
            sql: format!(
                "SELECT entity.entity_id AS entity_id, price.value_int AS price_minor FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS price ON price.tenant_id = entity.tenant_id AND price.module_name = entity.module_name AND price.entity_name = entity.entity_name AND price.schema_version = entity.schema_version AND price.entity_id = entity.entity_id AND price.locale = entity.locale AND price.field_name = 'price_minor' WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND (price.value_int, entity.entity_id) > ({anchor_price}, {anchor_id}) ORDER BY price_minor, entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "exact_count",
            sql: format!(
                "SELECT count(*)::bigint AS result_count FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS status ON status.tenant_id = entity.tenant_id AND status.module_name = entity.module_name AND status.entity_name = entity.entity_name AND status.schema_version = entity.schema_version AND status.entity_id = entity.entity_id AND status.locale = entity.locale AND status.field_name = 'status' WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND status.value_text = 'published'"
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
                "WITH targets AS (SELECT product_id FROM idx_bench_source.product WHERE tenant_no = 1 AND locale = {locale} AND product_no <= {batch}), updated_entities AS (UPDATE idx_bench_eav.entity AS entity SET source_version = entity.source_version + 1 FROM targets WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND entity.entity_id = targets.product_id RETURNING entity.entity_id), updated_fields AS (UPDATE idx_bench_eav.field_value AS field SET value_int = field.value_int + CASE field.field_name WHEN 'price_minor' THEN 17 ELSE 1 END FROM targets WHERE field.tenant_id = {tenant} AND field.module_name = 'product' AND field.entity_name = 'product' AND field.schema_version = 1 AND field.locale = {locale} AND field.entity_id = targets.product_id AND field.field_name IN ('price_minor', 'rating_milli') RETURNING field.entity_id) SELECT (SELECT count(*) FROM updated_entities)::bigint AS affected_entities, (SELECT count(*) FROM updated_fields)::bigint AS affected_fields"
            ),
            expected_affected_entities: i64::from(batch),
        },
        MutationWorkload {
            name: "delete_product_batch",
            sql: format!(
                "WITH targets AS (SELECT product_id FROM idx_bench_source.product WHERE tenant_no = 1 AND locale = {locale} AND product_no <= {batch}), deleted_fields AS (DELETE FROM idx_bench_eav.field_value AS field USING targets WHERE field.tenant_id = {tenant} AND field.module_name = 'product' AND field.entity_name = 'product' AND field.schema_version = 1 AND field.locale = {locale} AND field.entity_id = targets.product_id RETURNING 1), deleted_links AS (DELETE FROM idx_bench_eav.link AS link USING targets WHERE link.tenant_id = {tenant} AND link.source_entity = 'product' AND link.source_locale = {locale} AND link.source_entity_id = targets.product_id RETURNING 1), deleted_entities AS (DELETE FROM idx_bench_eav.entity AS entity USING targets WHERE entity.tenant_id = {tenant} AND entity.module_name = 'product' AND entity.entity_name = 'product' AND entity.schema_version = 1 AND entity.locale = {locale} AND entity.entity_id = targets.product_id RETURNING entity.entity_id) SELECT (SELECT count(*) FROM deleted_entities)::bigint AS affected_entities, (SELECT count(*) FROM deleted_fields)::bigint AS affected_fields, (SELECT count(*) FROM deleted_links)::bigint AS affected_links, {deleted_links}::bigint AS expected_links"
            ),
            expected_affected_entities: i64::from(batch),
        },
    ]
}
