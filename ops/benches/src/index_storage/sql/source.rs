use super::{DatasetConfig, SOURCE_SCHEMA, sql_literal};

pub fn dataset_sql(config: &DatasetConfig) -> String {
    let locales = config
        .locales
        .iter()
        .enumerate()
        .map(|(index, locale)| format!("({}, {})", sql_literal(locale), index + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let last_channel_ordinal = config.sales_channels_per_variant.saturating_sub(1);

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
FROM generate_series(1, {tenants}) AS generated(tenant_no);

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
    generated.channel_no,
    tenant.tenant_id,
    md5(
        'channel:' || tenant.tenant_no::text || ':' || generated.channel_no::text
    )::uuid,
    'channel-' || generated.channel_no::text,
    CASE generated.channel_no % 4
        WHEN 0 THEN 'na'
        WHEN 1 THEN 'eu'
        WHEN 2 THEN 'apac'
        ELSE 'latam'
    END
FROM {SOURCE_SCHEMA}.tenant AS tenant
CROSS JOIN generate_series(1, {channels_per_tenant}) AS generated(channel_no);

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
    generated.product_no,
    locale_map.locale_no,
    tenant.tenant_id,
    md5(
        'product:' || tenant.tenant_no::text || ':' || generated.product_no::text
    )::uuid,
    locale_map.locale,
    generated.product_no::bigint,
    CASE generated.product_no % 4
        WHEN 0 THEN 'draft'
        WHEN 1 THEN 'published'
        WHEN 2 THEN 'published'
        ELSE 'archived'
    END,
    'Product ' || tenant.tenant_no::text || '-' || generated.product_no::text
        || ' (' || locale_map.locale || ')',
    500 + ((generated.product_no::bigint * 7919 + tenant.tenant_no::bigint * 101) % 200000),
    1000 + ((generated.product_no::bigint * 37 + tenant.tenant_no::bigint * 11) % 4001),
    ARRAY[
        'tag-' || (generated.product_no % 17)::text,
        'tag-' || ((generated.product_no * 7) % 31)::text,
        'tier-' || (generated.product_no % 5)::text
    ]::text[],
    timestamptz '2025-01-01 00:00:00+00'
        + make_interval(
            secs => (
                (generated.product_no * 97 + tenant.tenant_no * 13 + locale_map.locale_no * 7)
                % 31536000
            )::double precision
        )
FROM {SOURCE_SCHEMA}.tenant AS tenant
CROSS JOIN generate_series(1, {products_per_tenant}) AS generated(product_no)
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
    generated.variant_no,
    product.tenant_id,
    md5(
        'variant:' || product.tenant_no::text || ':' || product.product_no::text
        || ':' || generated.variant_no::text
    )::uuid,
    product.product_id,
    product.locale,
    product.source_version * 10 + generated.variant_no,
    'SKU-' || product.tenant_no::text || '-' || product.product_no::text
        || '-' || generated.variant_no::text,
    product.price_minor + generated.variant_no * 125,
    product.updated_at + make_interval(secs => generated.variant_no::double precision)
FROM {SOURCE_SCHEMA}.product AS product
CROSS JOIN generate_series(1, {variants_per_product}) AS generated(variant_no);

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
    generated.ordinal::smallint
FROM {SOURCE_SCHEMA}.variant AS variant
CROSS JOIN generate_series(0, {last_channel_ordinal}) AS generated(ordinal)
JOIN {SOURCE_SCHEMA}.channel AS channel
  ON channel.tenant_no = variant.tenant_no
 AND channel.channel_no = 1 + (
        (variant.product_no + variant.variant_no + generated.ordinal)
        % {channels_per_tenant}
    );

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
