//! PostgreSQL checks for the owner-counted catalog facets.
//!
//! The facet counters are the only place where the storefront and admin populations are computed,
//! so their SQL is verified against a real database instead of a portable test schema: the two
//! scopes (published storefront rows versus every lifecycle status), the drill-down rules (a facet
//! ignores its own selection, intersects every other one, ORs the values of one attribute and ANDs
//! several attributes), the bucket vocabulary of typed columns, the truncation flag at the value
//! limit, the locale chain of attribute and option labels, and the tenant boundary. Every number in
//! this file is a hand-written expectation over the seeded catalog, so a change in a counting rule
//! has to be repeated here on purpose.
//!
//! The tests need PostgreSQL admin access and are ignored by default:
//!
//! ```text
//! RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://postgres:postgres@localhost:5432/postgres \
//!     cargo test -p rustok-product --test postgres_facet_counts -- --ignored
//! ```

use rustok_product::{
    AdminProductListQuery, CatalogService, StorefrontCatalogFacet, StorefrontProductListQuery,
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, mock_transactional_event_bus, postgres_database_url,
    unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use uuid::Uuid;

struct ProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

// ── fixed identifiers of the seeded catalog ─────────────────────────────────────────────────────

const TENANT_A: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000001);
const TENANT_B: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000002);
const CATEGORY_A: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000100);

const ATTR_COLOR: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000201);
const ATTR_WATERPROOF: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000202);
const ATTR_MATERIAL: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000203);
const ATTR_HIDDEN: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000204);
const ATTR_VARIANT_ONLY: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000205);
const ATTR_RETIRED: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000206);
const ATTR_PAYLOAD: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000207);
const ATTR_CARD_HIDDEN: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000208);

const OPTION_RED: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000301);
const OPTION_BLUE: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000302);
const OPTION_GREEN: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000303);
const OPTION_IVORY: Uuid = Uuid::from_u128(0x10000000_0000_0000_0000_000000000304);

const TENANT_B_ATTR_COLOR: Uuid = Uuid::from_u128(0x20000000_0000_0000_0000_000000000201);
const TENANT_B_OPTION_RED: Uuid = Uuid::from_u128(0x20000000_0000_0000_0000_000000000301);
const TENANT_B_PRODUCT: Uuid = Uuid::from_u128(0x20000000_0000_0000_0000_000000000401);

fn product(index: u32) -> Uuid {
    Uuid::from_u128(0x10000000_0000_0000_0000_000000000400 + index as u128)
}

const RED: &str = "10000000-0000-0000-0000-000000000301";
const BLUE: &str = "10000000-0000-0000-0000-000000000302";
const GREEN: &str = "10000000-0000-0000-0000-000000000303";
const IVORY: &str = "10000000-0000-0000-0000-000000000304";

// ── harness ─────────────────────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn storefront_facets_count_only_published_channel_visible_products() {
    if let Err(error) = run_storefront_population_checks().await {
        panic!("storefront facet population checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn admin_facets_count_every_lifecycle_status_and_narrow_by_status() {
    if let Err(error) = run_admin_population_checks().await {
        panic!("admin facet population checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn facet_buckets_are_drill_down_counts_of_the_other_filters() {
    if let Err(error) = run_drill_down_checks().await {
        panic!("facet drill-down checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn facet_labels_and_buckets_follow_the_locale_chain() {
    if let Err(error) = run_locale_chain_checks().await {
        panic!("facet locale checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn facet_requests_are_normalized_and_reject_uncountable_attributes() {
    if let Err(error) = run_request_validation_checks().await {
        panic!("facet request validation checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn enumerable_facets_are_truncated_at_the_value_limit() {
    if let Err(error) = run_truncation_checks().await {
        panic!("facet truncation checks failed: {error}");
    }
}

async fn with_product_postgres_database<T, F, Fut>(
    prefix: &str,
    test: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(DatabaseConnection) -> Fut,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_owned());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name(prefix);
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let test_result = async {
        let db = connect_postgres(&target_url).await?;
        create_prerequisites(&db).await?;

        ProductMigrator::up(&db, None).await?;
        let result = test(db.clone()).await;
        db.close().await?;
        result
    }
    .await;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    test_result
}

async fn create_prerequisites(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
CREATE TABLE tenants (
    id UUID PRIMARY KEY
);
CREATE TABLE taxonomy_terms (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    UNIQUE (tenant_id, id)
);
"#,
    )
    .await?;
    let manager = SchemaManager::new(db);
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

// ── seeding ─────────────────────────────────────────────────────────────────────────────────────

/// Publishes the shared facet fixture: two tenants, one category, seven attributes and ten products
/// whose status, publication date, channel restriction and values cover every counting rule.
async fn seed_catalog(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO tenants (id) VALUES ('{TENANT_A}'), ('{TENANT_B}');

INSERT INTO taxonomy_terms (id, tenant_id) VALUES ('{CATEGORY_A}', '{TENANT_A}');
INSERT INTO catalog_categories (id, tenant_id, code, slug, path) VALUES
    ('{CATEGORY_A}', '{TENANT_A}', 'lamps', 'lamps', 'lamps');

INSERT INTO product_attributes
    (id, tenant_id, code, value_type, scope, is_filterable, show_on_storefront, position)
VALUES
    ('{ATTR_COLOR}', '{TENANT_A}', 'color', 'select', 'product', TRUE, TRUE, 0),
    ('{ATTR_WATERPROOF}', '{TENANT_A}', 'waterproof', 'boolean', 'product', TRUE, TRUE, 1),
    ('{ATTR_MATERIAL}', '{TENANT_A}', 'material', 'text', 'product', TRUE, TRUE, 2),
    ('{ATTR_HIDDEN}', '{TENANT_A}', 'hidden', 'text', 'product', FALSE, TRUE, 3),
    ('{ATTR_VARIANT_ONLY}', '{TENANT_A}', 'variant_only', 'text', 'variant', TRUE, TRUE, 4),
    ('{ATTR_RETIRED}', '{TENANT_A}', 'retired', 'text', 'product', TRUE, TRUE, 5),
    ('{ATTR_PAYLOAD}', '{TENANT_A}', 'payload', 'json', 'product', TRUE, TRUE, 6),
    -- Filterable, but hidden from the product card: `show_on_storefront` steers the specification
    -- row of the detail contract, never catalog filtering.
    ('{ATTR_CARD_HIDDEN}', '{TENANT_A}', 'card_hidden', 'select', 'product', TRUE, FALSE, 7);

UPDATE product_attributes SET archived_at = now() WHERE id = '{ATTR_RETIRED}';

INSERT INTO product_attribute_translations (id, attribute_id, locale, label, facet_label) VALUES
    ('{attr_color_en}', '{ATTR_COLOR}', 'en-US', 'Colour', 'Colour filter'),
    ('{attr_color_de}', '{ATTR_COLOR}', 'de-DE', 'Farbe', NULL),
    ('{attr_waterproof_en}', '{ATTR_WATERPROOF}', 'en-US', 'Waterproof', NULL);

INSERT INTO product_attribute_options (id, tenant_id, attribute_id, code, position) VALUES
    ('{OPTION_RED}', '{TENANT_A}', '{ATTR_COLOR}', 'red', 0),
    ('{OPTION_BLUE}', '{TENANT_A}', '{ATTR_COLOR}', 'blue', 1),
    ('{OPTION_GREEN}', '{TENANT_A}', '{ATTR_COLOR}', 'green', 2),
    ('{OPTION_IVORY}', '{TENANT_A}', '{ATTR_CARD_HIDDEN}', 'ivory', 0);

INSERT INTO product_attribute_option_translations (id, option_id, locale, label) VALUES
    ('{option_red_en}', '{OPTION_RED}', 'en-US', 'Red'),
    ('{option_red_de}', '{OPTION_RED}', 'de-DE', 'Rot'),
    ('{option_blue_en}', '{OPTION_BLUE}', 'en-US', 'Blue'),
    ('{option_ivory_en}', '{OPTION_IVORY}', 'en-US', 'Ivory');

INSERT INTO products (id, tenant_id, status, published_at, primary_category_id, metadata) VALUES
    ('{p1}', '{TENANT_A}', 'active', now(), NULL, '{{}}'::jsonb),
    ('{p2}', '{TENANT_A}', 'active', now(), NULL, '{{}}'::jsonb),
    ('{p3}', '{TENANT_A}', 'active', now(), '{CATEGORY_A}', '{{}}'::jsonb),
    ('{p4}', '{TENANT_A}', 'active', now(), '{CATEGORY_A}', '{{}}'::jsonb),
    ('{p5}', '{TENANT_A}', 'active', now(), NULL, '{{"channel_visibility": {{"allowed_channel_slugs": ["eu-store"]}}}}'::jsonb),
    ('{p6}', '{TENANT_A}', 'active', NULL, NULL, '{{}}'::jsonb),
    ('{p7}', '{TENANT_A}', 'draft', NULL, NULL, '{{}}'::jsonb),
    ('{p8}', '{TENANT_A}', 'archived', NULL, NULL, '{{}}'::jsonb),
    ('{p9}', '{TENANT_A}', 'active', now(), NULL, '{{}}'::jsonb),
    ('{p10}', '{TENANT_A}', 'active', now(), NULL, '{{}}'::jsonb),
    ('{tenant_b_product}', '{TENANT_B}', 'active', now(), NULL, '{{}}'::jsonb);

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{title_p1}', '{p1}', '{TENANT_A}', 'en-US', 'Desk Lamp', 'desk-lamp-1'),
    ('{title_p2}', '{p2}', '{TENANT_A}', 'en-US', 'Desk Lamp', 'desk-lamp-2'),
    ('{title_p3}', '{p3}', '{TENANT_A}', 'en-US', 'Table', 'table-3'),
    ('{title_p4}', '{p4}', '{TENANT_A}', 'en-US', 'Table', 'table-4'),
    ('{title_p5}', '{p5}', '{TENANT_A}', 'en-US', 'Desk Lamp', 'desk-lamp-5'),
    ('{title_p6}', '{p6}', '{TENANT_A}', 'en-US', 'Garage Lamp', 'garage-lamp-6'),
    ('{title_p7}', '{p7}', '{TENANT_A}', 'en-US', 'Draft Lamp', 'draft-lamp-7'),
    ('{title_p8}', '{p8}', '{TENANT_A}', 'en-US', 'Archived Lamp', 'archived-lamp-8'),
    ('{title_p9}', '{p9}', '{TENANT_A}', 'en-US', 'Widget', 'widget-9'),
    ('{title_p10}', '{p10}', '{TENANT_A}', 'en-US', 'Widget', 'widget-10'),
    ('{title_tenant_b}', '{tenant_b_product}', '{TENANT_B}', 'en-US', 'Tenant two lamp', 'tenant-two-lamp');

-- Attribute values are inserted from the carriers themselves, and their ids are derived from the
-- feature they carry (`md5(prefix || product_id)`): the option links below can then address a value
-- row without a second set of id literals, while every product still keeps at most one value per
-- attribute (`uq_product_attribute_values`).
INSERT INTO product_attribute_values (id, tenant_id, product_id, attribute_id)
SELECT
    md5('facet-color-' || carrier.id::text)::uuid,
    carrier.tenant_id,
    carrier.id,
    '{ATTR_COLOR}'::uuid
FROM products carrier
WHERE carrier.id IN (
    '{p1}'::uuid, '{p2}'::uuid, '{p3}'::uuid, '{p4}'::uuid, '{p5}'::uuid,
    '{p6}'::uuid, '{p7}'::uuid, '{p8}'::uuid, '{p9}'::uuid
);

-- Color values are dictionary references: the scalar columns stay NULL and the option link carries
-- the bucket.
INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id)
SELECT value_row.tenant_id, value_row.id, link.option_id
FROM product_attribute_values value_row
JOIN (
    VALUES
        ('{p1}'::uuid, '{OPTION_RED}'::uuid),
        ('{p2}'::uuid, '{OPTION_RED}'::uuid),
        ('{p3}'::uuid, '{OPTION_BLUE}'::uuid),
        ('{p4}'::uuid, '{OPTION_BLUE}'::uuid),
        ('{p5}'::uuid, '{OPTION_RED}'::uuid),
        ('{p6}'::uuid, '{OPTION_GREEN}'::uuid),
        ('{p7}'::uuid, '{OPTION_RED}'::uuid),
        ('{p8}'::uuid, '{OPTION_BLUE}'::uuid),
        ('{p9}'::uuid, '{OPTION_GREEN}'::uuid)
) AS link(product_id, option_id) ON link.product_id = value_row.product_id
WHERE value_row.attribute_id = '{ATTR_COLOR}'::uuid;

INSERT INTO product_attribute_values (id, tenant_id, product_id, attribute_id)
SELECT
    md5('facet-card-hidden-' || carrier.id::text)::uuid,
    carrier.tenant_id,
    carrier.id,
    '{ATTR_CARD_HIDDEN}'::uuid
FROM products carrier
WHERE carrier.id = '{p9}'::uuid;

INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id)
SELECT value_row.tenant_id, value_row.id, '{OPTION_IVORY}'::uuid
FROM product_attribute_values value_row
WHERE value_row.attribute_id = '{ATTR_CARD_HIDDEN}'::uuid;

INSERT INTO product_attribute_values
    (id, tenant_id, product_id, attribute_id, value_boolean)
SELECT
    md5('facet-waterproof-' || flag.id::text)::uuid,
    '{TENANT_A}'::uuid,
    flag.id,
    '{ATTR_WATERPROOF}'::uuid,
    flag.value
FROM (
    VALUES
        ('{p1}'::uuid, TRUE),
        ('{p2}'::uuid, FALSE),
        ('{p3}'::uuid, TRUE),
        ('{p5}'::uuid, TRUE),
        ('{p6}'::uuid, TRUE),
        ('{p7}'::uuid, TRUE),
        ('{p8}'::uuid, TRUE)
) AS flag(id, value);

INSERT INTO product_attribute_values
    (id, tenant_id, product_id, attribute_id, value_text)
SELECT
    md5('facet-material-' || material.id::text)::uuid,
    '{TENANT_A}'::uuid,
    material.id,
    '{ATTR_MATERIAL}'::uuid,
    material.value
FROM (
    VALUES
        ('{p1}'::uuid, 'Leather'),
        ('{p2}'::uuid, 'Leather'),
        ('{p4}'::uuid, 'Cotton'),
        ('{p5}'::uuid, 'Leather'),
        ('{p6}'::uuid, 'Leather'),
        ('{p7}'::uuid, 'Leather'),
        ('{p8}'::uuid, 'Leather')
) AS material(id, value);

-- The other tenant carries its own dictionary and a product with a red value: a tenant-scoped
-- count must never see it, even though the attribute code is the same.
INSERT INTO product_attributes
    (id, tenant_id, code, value_type, scope, is_filterable, show_on_storefront)
VALUES
    ('{TENANT_B_ATTR_COLOR}', '{TENANT_B}', 'color', 'select', 'product', TRUE, TRUE);
INSERT INTO product_attribute_options (id, tenant_id, attribute_id, code, position) VALUES
    ('{TENANT_B_OPTION_RED}', '{TENANT_B}', '{TENANT_B_ATTR_COLOR}', 'red', 0);
INSERT INTO product_attribute_values (id, tenant_id, product_id, attribute_id) VALUES
    (
        md5('facet-tenant-b-color')::uuid,
        '{TENANT_B}',
        '{tenant_b_product}',
        '{TENANT_B_ATTR_COLOR}'
    );
INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id)
SELECT '{TENANT_B}'::uuid, value_row.id, '{TENANT_B_OPTION_RED}'::uuid
FROM product_attribute_values value_row
WHERE value_row.attribute_id = '{TENANT_B_ATTR_COLOR}'::uuid;
"#,
        attr_color_en = uuid(0x10000000_0000_0000_0000_000000000801),
        attr_color_de = uuid(0x10000000_0000_0000_0000_000000000802),
        attr_waterproof_en = uuid(0x10000000_0000_0000_0000_000000000803),
        option_red_en = uuid(0x10000000_0000_0000_0000_000000000901),
        option_red_de = uuid(0x10000000_0000_0000_0000_000000000902),
        option_blue_en = uuid(0x10000000_0000_0000_0000_000000000903),
        option_ivory_en = uuid(0x10000000_0000_0000_0000_000000000904),
        p1 = product(1),
        p2 = product(2),
        p3 = product(3),
        p4 = product(4),
        p5 = product(5),
        p6 = product(6),
        p7 = product(7),
        p8 = product(8),
        p9 = product(9),
        p10 = product(10),
        tenant_b_product = TENANT_B_PRODUCT,
        title_p1 = uuid(0x10000000_0000_0000_0000_000000001001),
        title_p2 = uuid(0x10000000_0000_0000_0000_000000001002),
        title_p3 = uuid(0x10000000_0000_0000_0000_000000001003),
        title_p4 = uuid(0x10000000_0000_0000_0000_000000001004),
        title_p5 = uuid(0x10000000_0000_0000_0000_000000001005),
        title_p6 = uuid(0x10000000_0000_0000_0000_000000001006),
        title_p7 = uuid(0x10000000_0000_0000_0000_000000001007),
        title_p8 = uuid(0x10000000_0000_0000_0000_000000001008),
        title_p9 = uuid(0x10000000_0000_0000_0000_000000001009),
        title_p10 = uuid(0x10000000_0000_0000_0000_000000001010),
        title_tenant_b = uuid(0x20000000_0000_0000_0000_000000001001),
    ))
    .await?;
    Ok(())
}

fn uuid(value: u128) -> String {
    Uuid::from_u128(value).to_string()
}

/// Seeds one dictionary attribute with `buckets` options, each carried by its own published product.
async fn seed_dictionary_attribute(
    db: &DatabaseConnection,
    attribute_id: Uuid,
    code: &str,
    product_prefix: u128,
    value_prefix: u128,
    option_prefix: u128,
    buckets: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(&format!(
        "INSERT INTO tenants (id) VALUES ('{TENANT_A}') ON CONFLICT DO NOTHING;
         INSERT INTO product_attributes
             (id, tenant_id, code, value_type, scope, is_filterable, show_on_storefront)
         VALUES ('{attribute_id}', '{TENANT_A}', '{code}', 'select', 'product', TRUE, TRUE);"
    ))
    .await?;

    for index in 0..buckets {
        let option_id = Uuid::from_u128(option_prefix + index as u128);
        let product_id = Uuid::from_u128(product_prefix + index as u128);
        let value_id = Uuid::from_u128(value_prefix + index as u128);
        let title_id = Uuid::from_u128(0x30000000_0000_0000_0000_000000000000 + index as u128);
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO product_attribute_options (id, tenant_id, attribute_id, code, position) \
             VALUES ($1, $2, $3, $4, $5)",
            vec![
                option_id.into(),
                TENANT_A.into(),
                attribute_id.into(),
                format!("{code}_{index:02}").into(),
                (index as i32).into(),
            ],
        ))
        .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO products (id, tenant_id, status, published_at) \
             VALUES ($1, $2, 'active', now())",
            vec![product_id.into(), TENANT_A.into()],
        ))
        .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) \
             VALUES ($1, $2, $3, 'en-US', $4, $5)",
            vec![
                title_id.into(),
                product_id.into(),
                TENANT_A.into(),
                format!("{code} bucket {index:02}").into(),
                format!("{code}-bucket-{index:02}").into(),
            ],
        ))
        .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO product_attribute_values (id, tenant_id, product_id, attribute_id) \
             VALUES ($1, $2, $3, $4)",
            vec![
                value_id.into(),
                TENANT_A.into(),
                product_id.into(),
                attribute_id.into(),
            ],
        ))
        .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id) \
             VALUES ($1, $2, $3)",
            vec![TENANT_A.into(), value_id.into(), option_id.into()],
        ))
        .await?;
    }
    Ok(())
}

// ── helpers over the facet answer ───────────────────────────────────────────────────────────────

fn facet<'a>(facets: &'a [StorefrontCatalogFacet], code: &str) -> &'a StorefrontCatalogFacet {
    facets
        .iter()
        .find(|facet| facet.code == code)
        .unwrap_or_else(|| panic!("facet {code} is missing from {facets:?}"))
}

/// Buckets of one facet as `(value, count)` pairs, in the order the owner answered them.
fn buckets(facet: &StorefrontCatalogFacet) -> Vec<(String, u64)> {
    facet
        .values
        .iter()
        .map(|value| (value.value.clone(), value.count))
        .collect()
}

/// Buckets of one facet as `(value, label, count)` triples.
fn labelled_buckets(facet: &StorefrontCatalogFacet) -> Vec<(String, String, u64)> {
    facet
        .values
        .iter()
        .map(|value| (value.value.clone(), value.label.clone(), value.count))
        .collect()
}

fn service(db: &DatabaseConnection) -> CatalogService {
    CatalogService::new(db.clone(), mock_transactional_event_bus())
}

async fn storefront_facets(
    db: &DatabaseConnection,
    locale: &str,
    fallback_locale: Option<&str>,
    channel: Option<&str>,
    query: &StorefrontProductListQuery,
    codes: &[&str],
) -> Result<Vec<StorefrontCatalogFacet>, Box<dyn std::error::Error>> {
    let codes = codes
        .iter()
        .map(|code| (*code).to_string())
        .collect::<Vec<_>>();
    Ok(service(db)
        .storefront_catalog_facets(TENANT_A, locale, fallback_locale, channel, query, &codes)
        .await?)
}

async fn admin_facets(
    db: &DatabaseConnection,
    locale: &str,
    query: &AdminProductListQuery,
    codes: &[&str],
) -> Result<Vec<StorefrontCatalogFacet>, Box<dyn std::error::Error>> {
    let codes = codes
        .iter()
        .map(|code| (*code).to_string())
        .collect::<Vec<_>>();
    Ok(service(db)
        .admin_catalog_facets(TENANT_A, locale, Some("en-US"), query, &codes)
        .await?)
}

fn storefront_query(
    search: Option<&str>,
    category_id: Option<Uuid>,
    filters: &[&str],
) -> StorefrontProductListQuery {
    StorefrontProductListQuery::try_new_with_attribute_filters(
        search.map(str::to_string),
        category_id,
        None,
        None,
        filters.iter().map(|filter| (*filter).to_string()).collect(),
    )
    .expect("storefront query")
}

fn admin_query(
    search: Option<&str>,
    status: Option<&str>,
    category_id: Option<Uuid>,
    filters: &[&str],
) -> AdminProductListQuery {
    AdminProductListQuery::try_from_transport_with_attribute_filters(
        search.map(str::to_string),
        status.map(str::to_string),
        category_id.map(|value| value.to_string()),
        None,
        None,
        filters.iter().map(|filter| (*filter).to_string()).collect(),
    )
    .expect("admin query")
}

// ── checks ──────────────────────────────────────────────────────────────────────────────────────

async fn run_storefront_population_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_facet_storefront", |db| async move {
        seed_catalog(&db).await?;

        // Published, unrestricted products only: p1..p4 and p9 carry a color, p6 is active but
        // never published, p5 is restricted to another channel, p7/p8 are draft/archived.
        let facets = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &["color", "waterproof", "material", "card_hidden"],
        )
        .await?;

        let color = facet(&facets, "color");
        assert_eq!(color.total_products, 5, "published color carriers");
        assert_eq!(
            buckets(color),
            vec![
                (RED.to_string(), 2),
                (BLUE.to_string(), 2),
                (GREEN.to_string(), 1),
            ],
            "count-descending buckets with a stable option tie-break"
        );
        assert!(color.is_enumerable);
        assert!(!color.is_truncated);
        assert_eq!(color.value_type, "select");
        assert!(!color.is_localized);

        let waterproof = facet(&facets, "waterproof");
        assert_eq!(waterproof.total_products, 3);
        assert_eq!(
            buckets(waterproof),
            vec![("true".to_string(), 2), ("false".to_string(), 1)]
        );

        // A text attribute has no bucket vocabulary, but it still answers the drill-down total: a
        // panel keeps its free-form input and needs the number behind every other filter.
        let material = facet(&facets, "material");
        assert_eq!(material.total_products, 3);
        assert!(!material.is_enumerable);
        assert!(material.values.is_empty(), "open domains never enumerate");
        assert!(!material.is_truncated);

        // `show_on_storefront = FALSE` hides the attribute from the product card's specification
        // list; it is still a catalog filter, so the owner keeps counting it. The boundary is
        // deliberate: wave 12 applied the flag in the storefront attribute projection only, and the
        // facet contract stays the catalog's filter vocabulary.
        let card_hidden = facet(&facets, "card_hidden");
        assert_eq!(card_hidden.total_products, 1);
        assert_eq!(buckets(card_hidden), vec![(IVORY.to_string(), 1)]);
        assert_eq!(
            card_hidden.label, "card_hidden",
            "an attribute without translations is labelled by its code"
        );

        // The channel the product allows includes it; any other channel does not.
        let eu_channel = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            Some("eu-store"),
            &storefront_query(None, None, &[]),
            &["color"],
        )
        .await?;
        let eu_color = facet(&eu_channel, "color");
        assert_eq!(eu_color.total_products, 6, "channel-visible population");
        assert_eq!(
            buckets(eu_color),
            vec![
                (RED.to_string(), 3),
                (BLUE.to_string(), 2),
                (GREEN.to_string(), 1),
            ]
        );

        let other_channel = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            Some("us-store"),
            &storefront_query(None, None, &[]),
            &["color"],
        )
        .await?;
        assert_eq!(
            facet(&other_channel, "color").total_products,
            5,
            "a product restricted to another channel stays out"
        );

        // Search and category narrow the same population the list shows.
        let searched = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(Some("Lamp"), None, &[]),
            &["color"],
        )
        .await?;
        let searched_color = facet(&searched, "color");
        assert_eq!(
            searched_color.total_products, 3,
            "published titles matching `Lamp`: p1, p2 and p5"
        );
        assert_eq!(buckets(searched_color), vec![(RED.to_string(), 3)]);

        let categorized = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, Some(CATEGORY_A), &[]),
            &["color"],
        )
        .await?;
        let categorized_color = facet(&categorized, "color");
        assert_eq!(categorized_color.total_products, 2);
        assert_eq!(buckets(categorized_color), vec![(BLUE.to_string(), 2)]);

        // The same dictionary lives in a second tenant: its products never enter these numbers.
        let other_tenant = service(&db)
            .storefront_catalog_facets(
                TENANT_B,
                "en-US",
                Some("en-US"),
                None,
                &storefront_query(None, None, &[]),
                &["color".to_string()],
            )
            .await?;
        let other_tenant_color = facet(&other_tenant, "color");
        assert_eq!(other_tenant_color.total_products, 1);
        assert_eq!(buckets(other_tenant_color).len(), 1);

        Ok(())
    })
    .await
}

async fn run_admin_population_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_facet_admin", |db| async move {
        seed_catalog(&db).await?;

        // Admin facets describe the grid population: every lifecycle status, published or not.
        let facets = admin_facets(
            &db,
            "en-US",
            &admin_query(None, None, None, &[]),
            &["color", "waterproof"],
        )
        .await?;
        let color = facet(&facets, "color");
        assert_eq!(color.total_products, 9, "drafts and archived rows included");
        assert_eq!(
            buckets(color),
            vec![
                (RED.to_string(), 4),
                (BLUE.to_string(), 3),
                (GREEN.to_string(), 2),
            ]
        );
        assert_eq!(facet(&facets, "waterproof").total_products, 7);

        // A status filter narrows the buckets exactly like the admin list.
        let drafts = admin_facets(
            &db,
            "en-US",
            &admin_query(None, Some("draft"), None, &[]),
            &["color"],
        )
        .await?;
        let draft_color = facet(&drafts, "color");
        assert_eq!(draft_color.total_products, 1);
        assert_eq!(buckets(draft_color), vec![(RED.to_string(), 1)]);

        let archived = admin_facets(
            &db,
            "en-US",
            &admin_query(None, Some("archived"), None, &[]),
            &["color"],
        )
        .await?;
        assert_eq!(facet(&archived, "color").total_products, 1);

        Ok(())
    })
    .await
}

async fn run_drill_down_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_facet_drill_down", |db| async move {
        seed_catalog(&db).await?;

        // Selecting a boolean bucket removes it from its own facet (the panel must keep showing the
        // alternative) and narrows every other facet.
        let boolean_selected = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &["waterproof=true"]),
            &["color", "waterproof", "material"],
        )
        .await?;
        let color = facet(&boolean_selected, "color");
        assert_eq!(color.total_products, 2, "color under `waterproof=true`");
        assert_eq!(
            buckets(color),
            vec![(RED.to_string(), 1), (BLUE.to_string(), 1)]
        );
        let waterproof = facet(&boolean_selected, "waterproof");
        assert_eq!(
            waterproof.total_products, 3,
            "a facet ignores its own selection"
        );
        assert_eq!(
            buckets(waterproof),
            vec![("true".to_string(), 2), ("false".to_string(), 1)]
        );
        assert_eq!(
            facet(&boolean_selected, "material").total_products,
            1,
            "material under `waterproof=true`"
        );

        // A dictionary selection narrows the other facet and leaves its own buckets untouched.
        let dictionary_selected = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[&format!("color={RED}")]),
            &["color", "waterproof"],
        )
        .await?;
        let own = facet(&dictionary_selected, "color");
        assert_eq!(own.total_products, 5);
        assert_eq!(
            buckets(own),
            vec![
                (RED.to_string(), 2),
                (BLUE.to_string(), 2),
                (GREEN.to_string(), 1),
            ]
        );
        let narrowed = facet(&dictionary_selected, "waterproof");
        assert_eq!(narrowed.total_products, 2);
        assert_eq!(
            buckets(narrowed),
            vec![("true".to_string(), 1), ("false".to_string(), 1)]
        );

        // Several values of one attribute are an OR inside that attribute (`PROD-FACET-SELECT-001`):
        // a shopper ticking a second value widens the selection instead of failing the query, the
        // facet keeps ignoring its own whole selection and every other facet narrows by the OR.
        let widened = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(
                None,
                None,
                &[&format!("color={RED}"), &format!("color={BLUE}")],
            ),
            &["color", "material"],
        )
        .await?;
        let widened_own = facet(&widened, "color");
        assert_eq!(
            widened_own.total_products, 5,
            "a facet ignores every value of its own selection"
        );
        assert_eq!(
            buckets(widened_own),
            vec![
                (RED.to_string(), 2),
                (BLUE.to_string(), 2),
                (GREEN.to_string(), 1),
            ]
        );
        assert_eq!(
            facet(&widened, "material").total_products,
            3,
            "red or blue products with a material"
        );

        // Several attributes stay an AND of their selections: the OR narrows this facet to the
        // published red or blue products, and the second attribute narrows it further.
        let and_between_attributes = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[&format!("color={RED}"), "waterproof=true"]),
            &["material"],
        )
        .await?;
        assert_eq!(
            facet(&and_between_attributes, "material").total_products,
            1,
            "red and waterproof products with a material"
        );

        Ok(())
    })
    .await
}

async fn run_locale_chain_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_facet_locale", |db| async move {
        seed_catalog(&db).await?;

        // Requested locale first, fallback second, the code last: `de-DE` resolves the attribute
        // label, one option label, the English fallback of another option and the raw option code.
        let german = storefront_facets(
            &db,
            "de-DE",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &["color", "waterproof", "material"],
        )
        .await?;
        let color = facet(&german, "color");
        assert_eq!(color.label, "Farbe");
        assert_eq!(
            labelled_buckets(color),
            vec![
                (RED.to_string(), "Rot".to_string(), 2),
                (BLUE.to_string(), "Blue".to_string(), 2),
                (GREEN.to_string(), "green".to_string(), 1),
            ]
        );
        assert_eq!(
            facet(&german, "waterproof").label,
            "Waterproof",
            "a label without a requested locale falls back"
        );

        // The facet label wins over the grid label when the cataloguer filled it.
        let english = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &["color"],
        )
        .await?;
        assert_eq!(facet(&english, "color").label, "Colour filter");

        // An attribute without any translation is reported by its code, never by an empty label.
        assert_eq!(
            facet(&german, "material").label,
            "material",
            "no translation in any candidate locale falls back to the code"
        );

        Ok(())
    })
    .await
}

async fn run_request_validation_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_facet_requests", |db| async move {
        seed_catalog(&db).await?;

        // Codes are trimmed and de-duplicated case-insensitively, and the answer follows the order
        // of the request instead of the attribute table.
        let normalized = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &[" waterproof ", "COLOR", "color"],
        )
        .await?;
        assert_eq!(
            normalized
                .iter()
                .map(|facet| facet.code.as_str())
                .collect::<Vec<_>>(),
            vec!["waterproof", "color"]
        );

        // No codes means no counting at all: the panel renders nothing and the owner runs no query.
        let empty = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &[],
        )
        .await?;
        assert!(empty.is_empty());

        // A code that is unknown, not filterable, variant-scoped or archived is a validation error
        // rather than an empty facet, so a stale panel cannot silently render zero.
        for code in ["nosuch", "hidden", "variant_only", "retired"] {
            let error = storefront_facets(
                &db,
                "en-US",
                Some("en-US"),
                None,
                &storefront_query(None, None, &[]),
                &[code],
            )
            .await
            .expect_err("uncountable attribute code");
            assert!(
                error
                    .to_string()
                    .contains("not available as a product filter"),
                "{code} must be rejected as unavailable, got {error}"
            );
        }

        // JSON has no filter projection, and the limit is enforced before any attribute is read.
        let json_error = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &["payload"],
        )
        .await
        .expect_err("json attribute");
        assert!(
            json_error
                .to_string()
                .contains("cannot be used in attribute_filters"),
            "json attributes must be rejected, got {json_error}"
        );

        let too_many = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];
        let limit_error = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &too_many,
        )
        .await
        .expect_err("facet limit");
        assert!(
            limit_error.to_string().contains("at most"),
            "the facet limit must be enforced, got {limit_error}"
        );

        // The list limits count attributes, not values: one attribute may carry several selections
        // (`color=red;color=blue`), so the attribute bound and the entry bound are two separate
        // numbers and both answer with the same message shape as the facet limit.
        let eight_attributes = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .map(|code| format!("{code}=1"))
            .to_vec();
        assert!(
            StorefrontProductListQuery::try_new_with_attribute_filters(
                None,
                None,
                None,
                None,
                eight_attributes,
            )
            .is_ok(),
            "eight attributes are the documented attribute bound"
        );

        let nine_attributes = ["a", "b", "c", "d", "e", "f", "g", "h", "i"]
            .map(|code| format!("{code}=1"))
            .to_vec();
        let attribute_limit = StorefrontProductListQuery::try_new_with_attribute_filters(
            None,
            None,
            None,
            None,
            nine_attributes,
        )
        .expect_err("nine attributes exceed the attribute bound");
        assert!(
            attribute_limit.to_string().contains("at most 8 attributes"),
            "unexpected attribute-bound error: {attribute_limit}"
        );

        // Eight attributes may carry up to twenty values each, which is exactly the entry bound; one
        // entry more is rejected before any attribute is read.
        let mut too_many_entries = Vec::new();
        for code in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            for value in 0..21 {
                too_many_entries.push(format!("{code}={value}"));
            }
        }
        assert_eq!(too_many_entries.len(), 168);
        let entry_limit = StorefrontProductListQuery::try_new_with_attribute_filters(
            None,
            None,
            None,
            None,
            too_many_entries,
        )
        .expect_err("more entries than the documented bound");
        assert!(
            entry_limit
                .to_string()
                .contains("at most 160 code=value entries"),
            "unexpected entry-bound error: {entry_limit}"
        );

        // The admin scope shares the same request rules.
        let admin_error = service(&db)
            .admin_catalog_facets(
                TENANT_A,
                "en-US",
                Some("en-US"),
                &admin_query(None, None, None, &[]),
                &["nosuch".to_string()],
            )
            .await
            .expect_err("unknown admin facet code");
        assert!(
            admin_error
                .to_string()
                .contains("not available as a product filter")
        );

        Ok(())
    })
    .await
}

async fn run_truncation_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_facet_truncation", |db| async move {
        // Twenty-one buckets is one more than the panel renders, twenty is exactly the limit.
        seed_dictionary_attribute(
            &db,
            Uuid::from_u128(0x40000000_0000_0000_0000_000000000001),
            "size",
            0x40000000_0000_0000_0000_000000001000,
            0x40000000_0000_0000_0000_000000002000,
            0x40000000_0000_0000_0000_000000003000,
            21,
        )
        .await?;
        seed_dictionary_attribute(
            &db,
            Uuid::from_u128(0x40000000_0000_0000_0000_000000000002),
            "finish",
            0x40000000_0000_0000_0000_000000004000,
            0x40000000_0000_0000_0000_000000005000,
            0x40000000_0000_0000_0000_000000006000,
            20,
        )
        .await?;

        let facets = storefront_facets(
            &db,
            "en-US",
            Some("en-US"),
            None,
            &storefront_query(None, None, &[]),
            &["size", "finish"],
        )
        .await?;

        let size = facet(&facets, "size");
        assert_eq!(size.total_products, 21);
        assert_eq!(size.values.len(), 20, "the panel limit is the value limit");
        assert!(
            size.is_truncated,
            "a cut bucket list must report truncation instead of hiding it"
        );
        let first = Uuid::from_u128(0x40000000_0000_0000_0000_000000003000).to_string();
        assert_eq!(
            size.values.first().map(|value| value.value.as_str()),
            Some(first.as_str()),
            "equal counts fall back to the option id order"
        );

        let finish = facet(&facets, "finish");
        assert_eq!(finish.total_products, 20);
        assert_eq!(finish.values.len(), 20);
        assert!(
            !finish.is_truncated,
            "a list that exactly fills the limit is not truncated"
        );

        Ok(())
    })
    .await
}
