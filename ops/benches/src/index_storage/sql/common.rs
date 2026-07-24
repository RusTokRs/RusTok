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

pub fn assert_full_link_identity_sql(sql: String) -> String {
    for legacy in [
        "tenant_id, source_entity, source_entity_id, source_locale, link_name,\n    ordinal, target_entity, target_entity_id, target_locale",
        "product_variant.source_entity = 'product' AND product_variant.source_entity_id",
        "product_variant.target_entity = 'variant' JOIN",
        "variant_channel.source_entity = 'variant' AND variant_channel.source_entity_id",
        "variant_channel.target_entity = 'sales_channel' JOIN",
        "link.source_entity = 'product' AND link.source_locale",
    ] {
        assert!(
            !sql.contains(legacy),
            "generated benchmark SQL retained incomplete link identity: {legacy}"
        );
    }

    sql
}
