pub fn link_sql(schema: &str) -> String {
    format!(
        r#"
CREATE TABLE {schema}.link (
    tenant_id uuid NOT NULL,
    source_module text NOT NULL,
    source_entity text NOT NULL,
    source_schema_version integer NOT NULL CHECK (source_schema_version > 0),
    source_entity_id uuid NOT NULL,
    source_locale text NOT NULL,
    link_name text NOT NULL,
    ordinal smallint NOT NULL,
    target_module text NOT NULL,
    target_entity text NOT NULL,
    target_schema_version integer NOT NULL CHECK (target_schema_version > 0),
    target_entity_id uuid NOT NULL,
    target_locale text NOT NULL,
    PRIMARY KEY (
        tenant_id,
        source_module,
        source_entity,
        source_schema_version,
        source_entity_id,
        source_locale,
        link_name,
        ordinal,
        target_module,
        target_entity,
        target_schema_version,
        target_entity_id,
        target_locale
    )
);
INSERT INTO {schema}.link (
    tenant_id,
    source_module,
    source_entity,
    source_schema_version,
    source_entity_id,
    source_locale,
    link_name,
    ordinal,
    target_module,
    target_entity,
    target_schema_version,
    target_entity_id,
    target_locale
)
SELECT
    tenant_id,
    'product',
    'product',
    1,
    product_id,
    locale,
    'variants',
    (variant_no - 1)::smallint,
    'product',
    'variant',
    1,
    variant_id,
    locale
FROM idx_bench_source.variant;
INSERT INTO {schema}.link (
    tenant_id,
    source_module,
    source_entity,
    source_schema_version,
    source_entity_id,
    source_locale,
    link_name,
    ordinal,
    target_module,
    target_entity,
    target_schema_version,
    target_entity_id,
    target_locale
)
SELECT
    tenant_id,
    'product',
    'variant',
    1,
    variant_id,
    locale,
    'sales_channels',
    ordinal,
    'channel',
    'sales_channel',
    1,
    channel_id,
    ''
FROM idx_bench_source.variant_channel;
CREATE INDEX link_source_lookup ON {schema}.link (
    tenant_id,
    source_module,
    source_entity,
    source_schema_version,
    source_entity_id,
    source_locale,
    link_name,
    ordinal
);
CREATE INDEX link_target_lookup ON {schema}.link (
    tenant_id,
    target_module,
    target_entity,
    target_schema_version,
    target_entity_id,
    target_locale,
    link_name
);
"#
    )
}

pub fn qualify_link_identity_sql(sql: String) -> String {
    const LEGACY_INSERT_COLUMNS: &str = "tenant_id, source_entity, source_entity_id, source_locale, link_name,\n    ordinal, target_entity, target_entity_id, target_locale";
    const FULL_INSERT_COLUMNS: &str = "tenant_id, source_module, source_entity, source_schema_version, source_entity_id, source_locale, link_name,\n    ordinal, target_module, target_entity, target_schema_version, target_entity_id, target_locale";
    const LEGACY_PRODUCT_LINK_SELECT: &str = r#"SELECT
    tenant_id,
    'product',
    product_id,
    locale,
    'variants',
    (variant_no - 1)::smallint,
    'variant',
    variant_id,
    locale"#;
    const FULL_PRODUCT_LINK_SELECT: &str = r#"SELECT
    tenant_id,
    'product',
    'product',
    1,
    product_id,
    locale,
    'variants',
    (variant_no - 1)::smallint,
    'product',
    'variant',
    1,
    variant_id,
    locale"#;

    let qualified = sql
        .replace(LEGACY_INSERT_COLUMNS, FULL_INSERT_COLUMNS)
        .replace(LEGACY_PRODUCT_LINK_SELECT, FULL_PRODUCT_LINK_SELECT)
        .replace(
            "product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.entity_id",
            "product_variant.source_module = 'product' AND product_variant.source_entity = 'product' AND product_variant.source_schema_version = 1 AND product_variant.source_entity_id = product.entity_id",
        )
        .replace(
            "product_variant.source_entity = 'product' AND product_variant.source_entity_id = product.product_id",
            "product_variant.source_module = 'product' AND product_variant.source_entity = 'product' AND product_variant.source_schema_version = 1 AND product_variant.source_entity_id = product.product_id",
        )
        .replace(
            "product_variant.target_entity = 'variant'",
            "product_variant.target_module = 'product' AND product_variant.target_entity = 'variant' AND product_variant.target_schema_version = 1",
        )
        .replace(
            "variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id = product_variant.target_entity_id",
            "variant_channel.source_module = product_variant.target_module AND variant_channel.source_entity = product_variant.target_entity AND variant_channel.source_schema_version = product_variant.target_schema_version AND variant_channel.source_entity_id = product_variant.target_entity_id",
        )
        .replace(
            "variant_channel.target_entity = 'sales_channel'",
            "variant_channel.target_module = 'channel' AND variant_channel.target_entity = 'sales_channel' AND variant_channel.target_schema_version = 1",
        )
        .replace(
            "link.source_entity = 'product' AND link.source_locale",
            "link.source_module = 'product' AND link.source_entity = 'product' AND link.source_schema_version = 1 AND link.source_locale",
        );

    for legacy in [
        LEGACY_INSERT_COLUMNS,
        LEGACY_PRODUCT_LINK_SELECT,
        "product_variant.source_entity = 'product' AND product_variant.source_entity_id",
        "product_variant.target_entity = 'variant' AND product_variant.target_entity_id",
        "variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id",
        "variant_channel.target_entity = 'sales_channel' AND variant_channel.target_entity_id",
        "link.source_entity = 'product' AND link.source_locale",
    ] {
        assert!(
            !qualified.contains(legacy),
            "generated benchmark SQL retained incomplete link identity: {legacy}"
        );
    }

    qualified
}
