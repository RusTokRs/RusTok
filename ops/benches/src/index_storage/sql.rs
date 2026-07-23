use serde::Serialize;

use super::DatasetConfig;

pub const SOURCE_SCHEMA: &str = "idx_bench_source";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Prototype {
    Jsonb,
    TypedEav,
    HotProjection,
}

impl Prototype {
    pub const ALL: [Self; 3] = [Self::Jsonb, Self::TypedEav, Self::HotProjection];

    pub const fn schema(self) -> &'static str {
        match self {
            Self::Jsonb => "idx_bench_jsonb",
            Self::TypedEav => "idx_bench_eav",
            Self::HotProjection => "idx_bench_hot",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Workload {
    pub name: &'static str,
    pub sql: String,
}

pub fn source_dataset_sql(config: &DatasetConfig) -> String {
    let locales = values_list(&config.locales);
    format!(
        r#"
DROP SCHEMA IF EXISTS {SOURCE_SCHEMA} CASCADE;
CREATE SCHEMA {SOURCE_SCHEMA};

CREATE TABLE {SOURCE_SCHEMA}.tenant (
    tenant_no integer PRIMARY KEY,
    tenant_id uuid NOT NULL UNIQUE
);
INSERT INTO {SOURCE_SCHEMA}.tenant (tenant_no, tenant_id)
SELECT tenant_no, md5('tenant:' || tenant_no::text)::uuid
FROM generate_series(1, {tenants}) AS tenant_no;

CREATE TABLE {SOURCE_SCHEMA}.channel (
    tenant_no integer NOT NULL,
    channel_no integer NOT NULL,
    tenant_id uuid NOT NULL,
    channel_id uuid NOT NULL,
    code text NOT NULL,
    region_code text NOT NULL,
    PRIMARY KEY (tenant_id, channel_id)
);
INSERT INTO {SOURCE_SCHEMA}.channel (
    tenant_no, channel_no, tenant_id, channel_id, code, region_code
)
SELECT
    tenant.tenant_no,
    channel_no,
    tenant.tenant_id,
    md5('channel:' || tenant.tenant_no::text || ':' || channel_no::text)::uuid,
    'channel-' || channel_no::text,
    CASE channel_no % 4
        WHEN 0 THEN 'na'
        WHEN 1 THEN 'eu'
        WHEN 2 THEN 'apac'
        ELSE 'latam'
    END
FROM {SOURCE_SCHEMA}.tenant AS tenant
CROSS JOIN generate_series(1, {channels_per_tenant}) AS channel_no;

CREATE TABLE {SOURCE_SCHEMA}.product (
    tenant_no integer NOT NULL,
    product_no integer NOT NULL,
    locale_no integer NOT NULL,
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
WITH locale_map(locale, locale_no) AS (VALUES {locales})
INSERT INTO {SOURCE_SCHEMA}.product (
    tenant_no, product_no, locale_no, tenant_id, product_id, locale,
    source_version, status, title, price_minor, rating_milli, tags, updated_at
)
SELECT
    tenant.tenant_no,
    product_no,
    locale_map.locale_no,
    tenant.tenant_id,
    md5('product:' || tenant.tenant_no::text || ':' || product_no::text)::uuid,
    locale_map.locale,
    product_no::bigint,
    CASE product_no % 4
        WHEN 0 THEN 'draft'
        WHEN 1 THEN 'published'
        WHEN 2 THEN 'published'
        ELSE 'archived'
    END,
    'Product ' || tenant.tenant_no::text || '-' || product_no::text || ' (' || locale_map.locale || ')',
    500 + ((product_no::bigint * 7919 + tenant.tenant_no::bigint * 101) % 200000),
    1000 + ((product_no::bigint * 37 + tenant.tenant_no::bigint * 11) % 4001),
    ARRAY[
        'tag-' || (product_no % 17)::text,
        'tag-' || ((product_no * 7) % 31)::text,
        'tier-' || (product_no % 5)::text
    ]::text[],
    timestamptz '2025-01-01 00:00:00+00'
        + make_interval(secs => ((product_no * 97 + tenant.tenant_no * 13 + locale_map.locale_no * 7) % 31536000))
FROM {SOURCE_SCHEMA}.tenant AS tenant
CROSS JOIN generate_series(1, {products_per_tenant}) AS product_no
CROSS JOIN locale_map;

CREATE TABLE {SOURCE_SCHEMA}.variant (
    tenant_no integer NOT NULL,
    product_no integer NOT NULL,
    variant_no integer NOT NULL,
    tenant_id uuid NOT NULL,
    variant_id uuid NOT NULL,
    product_id uuid NOT NULL,
    locale text NOT NULL,
    source_version bigint NOT NULL,
    sku text NOT NULL,
    price_minor bigint NOT NULL,
    updated_at timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, variant_id, locale)
);
INSERT INTO {SOURCE_SCHEMA}.variant (
    tenant_no, product_no, variant_no, tenant_id, variant_id, product_id,
    locale, source_version, sku, price_minor, updated_at
)
SELECT
    product.tenant_no,
    product.product_no,
    variant_no,
    product.tenant_id,
    md5(
        'variant:' || product.tenant_no::text || ':' || product.product_no::text || ':' || variant_no::text
    )::uuid,
    product.product_id,
    product.locale,
    product.source_version * 10 + variant_no,
    'SKU-' || product.tenant_no::text || '-' || product.product_no::text || '-' || variant_no::text,
    product.price_minor + variant_no * 125,
    product.updated_at + make_interval(secs => variant_no)
FROM {SOURCE_SCHEMA}.product AS product
CROSS JOIN generate_series(1, {variants_per_product}) AS variant_no;

CREATE TABLE {SOURCE_SCHEMA}.variant_channel (
    tenant_id uuid NOT NULL,
    variant_id uuid NOT NULL,
    locale text NOT NULL,
    channel_id uuid NOT NULL,
    ordinal smallint NOT NULL,
    PRIMARY KEY (tenant_id, variant_id, locale, channel_id)
);
INSERT INTO {SOURCE_SCHEMA}.variant_channel (
    tenant_id, variant_id, locale, channel_id, ordinal
)
SELECT
    variant.tenant_id,
    variant.variant_id,
    variant.locale,
    channel.channel_id,
    link_ordinal::smallint
FROM {SOURCE_SCHEMA}.variant AS variant
CROSS JOIN generate_series(0, 1) AS link_ordinal
JOIN {SOURCE_SCHEMA}.channel AS channel
  ON channel.tenant_no = variant.tenant_no
 AND channel.channel_no = 1 + ((variant.product_no + variant.variant_no + link_ordinal) % {channels_per_tenant});

ANALYZE {SOURCE_SCHEMA}.tenant;
ANALYZE {SOURCE_SCHEMA}.channel;
ANALYZE {SOURCE_SCHEMA}.product;
ANALYZE {SOURCE_SCHEMA}.variant;
ANALYZE {SOURCE_SCHEMA}.variant_channel;
"#,
        tenants = config.tenants,
        channels_per_tenant = config.channels_per_tenant,
        products_per_tenant = config.products_per_tenant,
        variants_per_product = config.variants_per_product,
    )
}

pub fn prototype_sql(prototype: Prototype) -> &'static str {
    match prototype {
        Prototype::Jsonb => JSONB_SQL,
        Prototype::TypedEav => EAV_SQL,
        Prototype::HotProjection => HOT_SQL,
    }
}

pub fn workloads(prototype: Prototype, config: &DatasetConfig) -> Vec<Workload> {
    let tenant = "md5('tenant:1')::uuid";
    let locale = sql_literal(&config.locales[0]);
    let anchor_no = (config.products_per_tenant / 2).max(1);
    let anchor_price = 500 + ((i64::from(anchor_no) * 7919 + 101) % 200000);
    let anchor_id = format!("md5('product:1:{anchor_no}')::uuid");

    match prototype {
        Prototype::Jsonb => jsonb_workloads(tenant, &locale, anchor_price, &anchor_id),
        Prototype::TypedEav => eav_workloads(tenant, &locale, anchor_price, &anchor_id),
        Prototype::HotProjection => hot_workloads(tenant, &locale, anchor_price, &anchor_id),
    }
}

fn jsonb_workloads(tenant: &str, locale: &str, anchor_price: i64, anchor_id: &str) -> Vec<Workload> {
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
                "SELECT entity_id FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND entity_name = 'product' AND locale = {locale} AND payload->'tags' @> '[\"tag-3\"]'::jsonb ORDER BY entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "two_hop_channel_filter",
            sql: format!(
                "SELECT DISTINCT product.entity_id FROM idx_bench_jsonb.entity AS product JOIN idx_bench_jsonb.link AS product_variant ON product_variant.tenant_id = product.tenant_id AND product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.entity_id AND product_variant.source_locale = product.locale AND product_variant.link_name = 'variants' JOIN idx_bench_jsonb.link AS variant_channel ON variant_channel.tenant_id = product_variant.tenant_id AND variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id = product_variant.target_entity_id AND variant_channel.source_locale = product_variant.target_locale AND variant_channel.link_name = 'sales_channels' JOIN idx_bench_jsonb.entity AS channel ON channel.tenant_id = variant_channel.tenant_id AND channel.entity_name = 'sales_channel' AND channel.entity_id = variant_channel.target_entity_id AND channel.locale = variant_channel.target_locale WHERE product.tenant_id = {tenant} AND product.entity_name = 'product' AND product.locale = {locale} AND channel.payload->>'code' = 'channel-1' ORDER BY product.entity_id LIMIT 100"
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
                "SELECT count(*) FROM idx_bench_jsonb.entity WHERE tenant_id = {tenant} AND entity_name = 'product' AND locale = {locale} AND payload->>'status' = 'published'"
            ),
        },
    ]
}

fn eav_workloads(tenant: &str, locale: &str, anchor_price: i64, anchor_id: &str) -> Vec<Workload> {
    vec![
        Workload {
            name: "status_equality",
            sql: format!(
                "SELECT entity.entity_id FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS status ON status.tenant_id = entity.tenant_id AND status.entity_name = entity.entity_name AND status.entity_id = entity.entity_id AND status.locale = entity.locale AND status.field_name = 'status' WHERE entity.tenant_id = {tenant} AND entity.entity_name = 'product' AND entity.locale = {locale} AND status.value_text = 'published' ORDER BY entity.entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "price_range_sort",
            sql: format!(
                "SELECT entity.entity_id, price.value_int AS price_minor FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS price ON price.tenant_id = entity.tenant_id AND price.entity_name = entity.entity_name AND price.entity_id = entity.entity_id AND price.locale = entity.locale AND price.field_name = 'price_minor' WHERE entity.tenant_id = {tenant} AND entity.entity_name = 'product' AND entity.locale = {locale} AND price.value_int BETWEEN 20000 AND 80000 ORDER BY price.value_int, entity.entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "multi_value_tag",
            sql: format!(
                "SELECT DISTINCT entity.entity_id FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS tag ON tag.tenant_id = entity.tenant_id AND tag.entity_name = entity.entity_name AND tag.entity_id = entity.entity_id AND tag.locale = entity.locale AND tag.field_name = 'tags' WHERE entity.tenant_id = {tenant} AND entity.entity_name = 'product' AND entity.locale = {locale} AND tag.value_text = 'tag-3' ORDER BY entity.entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "two_hop_channel_filter",
            sql: format!(
                "SELECT DISTINCT product.entity_id FROM idx_bench_eav.entity AS product JOIN idx_bench_eav.link AS product_variant ON product_variant.tenant_id = product.tenant_id AND product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.entity_id AND product_variant.source_locale = product.locale AND product_variant.link_name = 'variants' JOIN idx_bench_eav.link AS variant_channel ON variant_channel.tenant_id = product_variant.tenant_id AND variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id = product_variant.target_entity_id AND variant_channel.source_locale = product_variant.target_locale AND variant_channel.link_name = 'sales_channels' JOIN idx_bench_eav.field_value AS channel_code ON channel_code.tenant_id = variant_channel.tenant_id AND channel_code.entity_name = 'sales_channel' AND channel_code.entity_id = variant_channel.target_entity_id AND channel_code.locale = variant_channel.target_locale AND channel_code.field_name = 'code' WHERE product.tenant_id = {tenant} AND product.entity_name = 'product' AND product.locale = {locale} AND channel_code.value_text = 'channel-1' ORDER BY product.entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "keyset_page",
            sql: format!(
                "SELECT entity.entity_id, price.value_int AS price_minor FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS price ON price.tenant_id = entity.tenant_id AND price.entity_name = entity.entity_name AND price.entity_id = entity.entity_id AND price.locale = entity.locale AND price.field_name = 'price_minor' WHERE entity.tenant_id = {tenant} AND entity.entity_name = 'product' AND entity.locale = {locale} AND (price.value_int, entity.entity_id) > ({anchor_price}, {anchor_id}) ORDER BY price.value_int, entity.entity_id LIMIT 100"
            ),
        },
        Workload {
            name: "exact_count",
            sql: format!(
                "SELECT count(*) FROM idx_bench_eav.entity AS entity JOIN idx_bench_eav.field_value AS status ON status.tenant_id = entity.tenant_id AND status.entity_name = entity.entity_name AND status.entity_id = entity.entity_id AND status.locale = entity.locale AND status.field_name = 'status' WHERE entity.tenant_id = {tenant} AND entity.entity_name = 'product' AND entity.locale = {locale} AND status.value_text = 'published'"
            ),
        },
    ]
}

fn hot_workloads(tenant: &str, locale: &str, anchor_price: i64, anchor_id: &str) -> Vec<Workload> {
    vec![
        Workload {
            name: "status_equality",
            sql: format!(
                "SELECT product_id FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND status = 'published' ORDER BY product_id LIMIT 100"
            ),
        },
        Workload {
            name: "price_range_sort",
            sql: format!(
                "SELECT product_id, price_minor FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND price_minor BETWEEN 20000 AND 80000 ORDER BY price_minor, product_id LIMIT 100"
            ),
        },
        Workload {
            name: "multi_value_tag",
            sql: format!(
                "SELECT product_id FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND tags @> ARRAY['tag-3']::text[] ORDER BY product_id LIMIT 100"
            ),
        },
        Workload {
            name: "two_hop_channel_filter",
            sql: format!(
                "SELECT DISTINCT product.product_id FROM idx_bench_hot.product AS product JOIN idx_bench_hot.link AS product_variant ON product_variant.tenant_id = product.tenant_id AND product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.product_id AND product_variant.source_locale = product.locale AND product_variant.link_name = 'variants' JOIN idx_bench_hot.link AS variant_channel ON variant_channel.tenant_id = product_variant.tenant_id AND variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id = product_variant.target_entity_id AND variant_channel.source_locale = product_variant.target_locale AND variant_channel.link_name = 'sales_channels' JOIN idx_bench_hot.sales_channel AS channel ON channel.tenant_id = variant_channel.tenant_id AND channel.channel_id = variant_channel.target_entity_id WHERE product.tenant_id = {tenant} AND product.locale = {locale} AND channel.code = 'channel-1' ORDER BY product.product_id LIMIT 100"
            ),
        },
        Workload {
            name: "keyset_page",
            sql: format!(
                "SELECT product_id, price_minor FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND (price_minor, product_id) > ({anchor_price}, {anchor_id}) ORDER BY price_minor, product_id LIMIT 100"
            ),
        },
        Workload {
            name: "exact_count",
            sql: format!(
                "SELECT count(*) FROM idx_bench_hot.product WHERE tenant_id = {tenant} AND locale = {locale} AND status = 'published'"
            ),
        },
    ]
}

fn values_list(locales: &[String]) -> String {
    locales
        .iter()
        .enumerate()
        .map(|(index, locale)| format!("({}, {})", sql_literal(locale), index + 1))
        .collect::<Vec<_>>()
        .join(", ")
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

const LINK_TABLE: &str = r#"
CREATE TABLE {schema}.link (
    tenant_id uuid NOT NULL,
    source_entity text NOT NULL,
    source_entity_id uuid NOT NULL,
    source_locale text NOT NULL,
    link_name text NOT NULL,
    ordinal smallint NOT NULL,
    target_entity text NOT NULL,
    target_entity_id uuid NOT NULL,
    target_locale text NOT NULL,
    PRIMARY KEY (
        tenant_id, source_entity, source_entity_id, source_locale,
        link_name, ordinal, target_entity_id, target_locale
    )
);
INSERT INTO {schema}.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name,
    ordinal, target_entity, target_entity_id, target_locale
)
SELECT
    tenant_id, 'product', product_id, locale, 'variants',
    (variant_no - 1)::smallint, 'variant', variant_id, locale
FROM idx_bench_source.variant;
INSERT INTO {schema}.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name,
    ordinal, target_entity, target_entity_id, target_locale
)
SELECT
    tenant_id, 'variant', variant_id, locale, 'sales_channels',
    ordinal, 'sales_channel', channel_id, ''
FROM idx_bench_source.variant_channel;
CREATE INDEX link_source_lookup ON {schema}.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name, ordinal
);
CREATE INDEX link_target_lookup ON {schema}.link (
    tenant_id, target_entity, target_entity_id, target_locale, link_name
);
"#;

const JSONB_SQL: &str = r#"
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
    PRIMARY KEY (tenant_id, module_name, entity_name, schema_version, entity_id, locale)
);
INSERT INTO idx_bench_jsonb.entity
SELECT tenant_id, 'product', 'product', 1, product_id, locale, source_version,
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
SELECT tenant_id, 'product', 'variant', 1, variant_id, locale, source_version,
       jsonb_build_object(
           'sku', sku,
           'price_minor', price_minor,
           'product_id', product_id,
           'updated_at', updated_at
       )
FROM idx_bench_source.variant;
INSERT INTO idx_bench_jsonb.entity
SELECT tenant_id, 'channel', 'sales_channel', 1, channel_id, '', 1,
       jsonb_build_object('code', code, 'region_code', region_code)
FROM idx_bench_source.channel;
CREATE INDEX entity_payload_gin ON idx_bench_jsonb.entity USING gin (payload jsonb_path_ops);
CREATE INDEX entity_status ON idx_bench_jsonb.entity (
    tenant_id, entity_name, locale, (payload->>'status'), entity_id
) WHERE entity_name = 'product';
CREATE INDEX entity_price ON idx_bench_jsonb.entity (
    tenant_id, entity_name, locale, ((payload->>'price_minor')::bigint), entity_id
) WHERE entity_name = 'product';
CREATE INDEX entity_channel_code ON idx_bench_jsonb.entity (
    tenant_id, entity_name, (payload->>'code'), entity_id
) WHERE entity_name = 'sales_channel';
"#;

const EAV_SQL: &str = r#"
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
    PRIMARY KEY (tenant_id, module_name, entity_name, schema_version, entity_id, locale)
);
CREATE TABLE idx_bench_eav.field_value (
    tenant_id uuid NOT NULL,
    entity_name text NOT NULL,
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
    CHECK (num_nonnulls(value_bool, value_int, value_numeric, value_text, value_uuid, value_ts) = 1),
    PRIMARY KEY (tenant_id, entity_name, entity_id, locale, field_name, ordinal)
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
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_text)
SELECT tenant_id, 'product', product_id, locale, 'status', 0, status FROM idx_bench_source.product
UNION ALL
SELECT tenant_id, 'product', product_id, locale, 'title', 0, title FROM idx_bench_source.product;
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_int)
SELECT tenant_id, 'product', product_id, locale, 'price_minor', 0, price_minor FROM idx_bench_source.product
UNION ALL
SELECT tenant_id, 'product', product_id, locale, 'rating_milli', 0, rating_milli FROM idx_bench_source.product;
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_ts)
SELECT tenant_id, 'product', product_id, locale, 'updated_at', 0, updated_at FROM idx_bench_source.product;
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_text)
SELECT product.tenant_id, 'product', product.product_id, product.locale, 'tags', tag.ordinality::integer - 1, tag.value
FROM idx_bench_source.product AS product
CROSS JOIN LATERAL unnest(product.tags) WITH ORDINALITY AS tag(value, ordinality);
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_text)
SELECT tenant_id, 'variant', variant_id, locale, 'sku', 0, sku FROM idx_bench_source.variant;
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_int)
SELECT tenant_id, 'variant', variant_id, locale, 'price_minor', 0, price_minor FROM idx_bench_source.variant;
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_uuid)
SELECT tenant_id, 'variant', variant_id, locale, 'product_id', 0, product_id FROM idx_bench_source.variant;
INSERT INTO idx_bench_eav.field_value (tenant_id, entity_name, entity_id, locale, field_name, ordinal, value_text)
SELECT tenant_id, 'sales_channel', channel_id, '', 'code', 0, code FROM idx_bench_source.channel
UNION ALL
SELECT tenant_id, 'sales_channel', channel_id, '', 'region_code', 0, region_code FROM idx_bench_source.channel;
CREATE INDEX field_text_lookup ON idx_bench_eav.field_value (
    tenant_id, entity_name, locale, field_name, value_text, entity_id
) WHERE value_text IS NOT NULL;
CREATE INDEX field_int_lookup ON idx_bench_eav.field_value (
    tenant_id, entity_name, locale, field_name, value_int, entity_id
) WHERE value_int IS NOT NULL;
CREATE INDEX field_uuid_lookup ON idx_bench_eav.field_value (
    tenant_id, entity_name, locale, field_name, value_uuid, entity_id
) WHERE value_uuid IS NOT NULL;
CREATE INDEX field_ts_lookup ON idx_bench_eav.field_value (
    tenant_id, entity_name, locale, field_name, value_ts, entity_id
) WHERE value_ts IS NOT NULL;
"#;

const HOT_SQL: &str = r#"
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
SELECT tenant_id, product_id, locale, source_version, status, title,
       price_minor, rating_milli, tags, updated_at
FROM idx_bench_source.product;
INSERT INTO idx_bench_hot.variant
SELECT tenant_id, variant_id, locale, source_version, sku, price_minor, updated_at
FROM idx_bench_source.variant;
INSERT INTO idx_bench_hot.sales_channel
SELECT tenant_id, channel_id, code, region_code
FROM idx_bench_source.channel;
CREATE INDEX product_status ON idx_bench_hot.product (tenant_id, locale, status, product_id);
CREATE INDEX product_price ON idx_bench_hot.product (tenant_id, locale, price_minor, product_id);
CREATE INDEX product_tags ON idx_bench_hot.product USING gin (tags);
CREATE INDEX channel_code ON idx_bench_hot.sales_channel (tenant_id, code, channel_id);
"#;

pub fn full_prototype_sql(prototype: Prototype) -> String {
    format!(
        "{}\n{}\nANALYZE {};\nANALYZE {}.link;\n",
        prototype_sql(prototype),
        LINK_TABLE.replace("{schema}", prototype.schema()),
        prototype.schema(),
        prototype.schema(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_storage::{DatasetConfig, DatasetScale};

    #[test]
    fn generated_sql_is_deterministic_and_separates_links() {
        let config = DatasetConfig::for_scale(
            DatasetScale::Smoke,
            vec!["en-US".to_owned(), "ru-RU".to_owned()],
        )
        .unwrap();
        assert_eq!(source_dataset_sql(&config), source_dataset_sql(&config));
        for prototype in Prototype::ALL {
            let sql = full_prototype_sql(prototype);
            assert!(sql.contains(".link"));
            assert!(sql.contains("source_entity"));
            assert!(sql.contains("target_entity"));
        }
    }
}
