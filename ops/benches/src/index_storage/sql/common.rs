pub fn link_sql(schema: &str) -> String {
    format!(
        r#"
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
    tenant_id,
    'product',
    product_id,
    locale,
    'variants',
    (variant_no - 1)::smallint,
    'variant',
    variant_id,
    locale
FROM idx_bench_source.variant;
INSERT INTO {schema}.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name,
    ordinal, target_entity, target_entity_id, target_locale
)
SELECT
    tenant_id,
    'variant',
    variant_id,
    locale,
    'sales_channels',
    ordinal,
    'sales_channel',
    channel_id,
    ''
FROM idx_bench_source.variant_channel;
CREATE INDEX link_source_lookup ON {schema}.link (
    tenant_id, source_entity, source_entity_id, source_locale, link_name, ordinal
);
CREATE INDEX link_target_lookup ON {schema}.link (
    tenant_id, target_entity, target_entity_id, target_locale, link_name
);
"#
    )
}
