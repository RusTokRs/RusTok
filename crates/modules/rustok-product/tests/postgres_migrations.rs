use rust_decimal::Decimal;
use rustok_test_utils::{
    assert_postgres_column_contract as assert_column_contract,
    assert_postgres_column_missing as assert_column_missing,
    assert_postgres_table_missing as assert_table_missing, assert_postgres_url, connect_postgres,
    create_postgres_database, drop_postgres_database_if_exists, postgres_database_url,
    unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use serde_json::Value;

struct ProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn target_schema_supports_lifecycle_and_removes_transitional_columns() {
    if let Err(error) = run_target_schema_checks().await {
        panic!("Product target-schema PostgreSQL checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access and seeds up to one million products"]
async fn storefront_queries_use_indexes_at_representative_scales() {
    if let Err(error) = run_storefront_query_plan_checks().await {
        panic!("Product storefront PostgreSQL query-plan checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn tenant_storage_constraints_reject_cross_tenant_catalog_writes() {
    if let Err(error) = run_tenant_storage_checks().await {
        panic!("Product tenant-storage PostgreSQL checks failed: {error}");
    }
}

async fn run_target_schema_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_schema", |db| async move {
        assert_target_schema(&db).await?;
        assert_target_values(&db).await?;

        ProductMigrator::down(&db, None).await?;
        assert_table_missing(&db, "products").await?;

        ProductMigrator::up(&db, None).await?;
        assert_target_schema(&db).await
    })
    .await
}

async fn run_storefront_query_plan_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_plans", |db| async move {
        db.execute_unprepared(
            r#"
INSERT INTO tenants (id)
SELECT md5('product-plan-tenant-' || tenant_number::text)::uuid
FROM generate_series(1, 10) AS tenant_number;
"#,
        )
        .await?;

        let mut previous_rows = 0;
        for total_rows in [10_000, 100_000, 1_000_000] {
            seed_products(&db, previous_rows + 1, total_rows).await?;
            db.execute_unprepared("ANALYZE products").await?;

            let page_plan = explain_storefront_page(&db).await?;
            let count_plan = explain_storefront_count(&db).await?;
            let page_ms = execution_time_ms(&page_plan)?;
            let count_ms = execution_time_ms(&count_plan)?;
            let page_plan_text = page_plan.to_string();
            let count_plan_text = count_plan.to_string();

            if !page_plan_text.contains("idx_products_storefront_published") {
                return Err(format!(
                    "storefront page plan at {total_rows} rows did not use \
                     idx_products_storefront_published: {page_plan_text}"
                )
                .into());
            }
            if total_rows >= 100_000
                && !count_plan_text.contains("idx_products_storefront_published")
            {
                return Err(format!(
                    "storefront count plan at {total_rows} rows did not use \
                     idx_products_storefront_published: {count_plan_text}"
                )
                .into());
            }

            println!(
                "product_storefront_plan rows={total_rows} page_ms={page_ms:.3} \
                 count_ms={count_ms:.3}"
            );
            previous_rows = total_rows;
        }
        Ok(())
    })
    .await
}

async fn run_tenant_storage_checks() -> Result<(), Box<dyn std::error::Error>> {
    with_product_postgres_database("rustok_product_tenants", |db| async move {
        db.execute_unprepared(
            r#"
INSERT INTO tenants (id) VALUES
    ('10000000-0000-0000-0000-000000000001'),
    ('20000000-0000-0000-0000-000000000002');

INSERT INTO products (id, tenant_id) VALUES
    ('10000000-0000-0000-0000-000000000101', '10000000-0000-0000-0000-000000000001'),
    ('20000000-0000-0000-0000-000000000202', '20000000-0000-0000-0000-000000000002');

BEGIN;
INSERT INTO catalog_categories (id, tenant_id, code, slug, path) VALUES
    ('10000000-0000-0000-0000-000000000111', '10000000-0000-0000-0000-000000000001', 'root', 'root', 'root'),
    ('20000000-0000-0000-0000-000000000222', '20000000-0000-0000-0000-000000000002', 'root', 'root', 'root');
INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth) VALUES
    ('10000000-0000-0000-0000-000000000001', '10000000-0000-0000-0000-000000000111', '10000000-0000-0000-0000-000000000111', 0),
    ('20000000-0000-0000-0000-000000000002', '20000000-0000-0000-0000-000000000222', '20000000-0000-0000-0000-000000000222', 0);
COMMIT;

INSERT INTO product_attributes (id, tenant_id, code, value_type) VALUES
    ('10000000-0000-0000-0000-000000000121', '10000000-0000-0000-0000-000000000001', 'material', 'text'),
    ('20000000-0000-0000-0000-000000000242', '20000000-0000-0000-0000-000000000002', 'material', 'text');

INSERT INTO product_attribute_schemas (id, tenant_id, code, status) VALUES
    ('10000000-0000-0000-0000-000000000131', '10000000-0000-0000-0000-000000000001', 'default', 'active'),
    ('20000000-0000-0000-0000-000000000262', '20000000-0000-0000-0000-000000000002', 'default', 'active');

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('10000000-0000-0000-0000-000000000141', '10000000-0000-0000-0000-000000000101', '10000000-0000-0000-0000-000000000001', 'en-US', 'Tenant one', 'shared'),
    ('20000000-0000-0000-0000-000000000282', '20000000-0000-0000-0000-000000000202', '20000000-0000-0000-0000-000000000002', 'en-US', 'Tenant two', 'shared');

INSERT INTO catalog_category_translations (id, category_id, locale, name) VALUES
    ('10000000-0000-0000-0000-000000000151', '10000000-0000-0000-0000-000000000111', 'en-US', 'Shared'),
    ('20000000-0000-0000-0000-000000000292', '20000000-0000-0000-0000-000000000222', 'en-US', 'Shared');
INSERT INTO product_attribute_translations (id, attribute_id, locale, label) VALUES
    ('10000000-0000-0000-0000-000000000161', '10000000-0000-0000-0000-000000000121', 'en-US', 'Shared'),
    ('20000000-0000-0000-0000-000000000302', '20000000-0000-0000-0000-000000000242', 'en-US', 'Shared');
INSERT INTO product_attribute_schema_translations (id, schema_id, locale, name) VALUES
    ('10000000-0000-0000-0000-000000000171', '10000000-0000-0000-0000-000000000131', 'en-US', 'Shared'),
    ('20000000-0000-0000-0000-000000000312', '20000000-0000-0000-0000-000000000262', 'en-US', 'Shared');
"#,
        )
        .await?;

        for (sql, constraint) in [
            (
                "INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) \
                 VALUES ('30000000-0000-0000-0000-000000000001', \
                 '10000000-0000-0000-0000-000000000101', \
                 '20000000-0000-0000-0000-000000000002', 'de-DE', 'Cross', 'cross')",
                "fk_product_translations_product_tenant",
            ),
            (
                "INSERT INTO catalog_categories (id, tenant_id, parent_id, code, slug, path, level) \
                 VALUES ('30000000-0000-0000-0000-000000000002', \
                 '20000000-0000-0000-0000-000000000002', \
                 '10000000-0000-0000-0000-000000000111', 'cross', 'cross', 'root/cross', 1)",
                "fk_catalog_categories_parent_tenant",
            ),
            (
                "INSERT INTO product_attribute_schema_attributes \
                 (id, tenant_id, schema_id, attribute_id) VALUES \
                 ('30000000-0000-0000-0000-000000000003', \
                 '20000000-0000-0000-0000-000000000002', \
                 '20000000-0000-0000-0000-000000000262', \
                 '10000000-0000-0000-0000-000000000121')",
                "fk_product_attribute_schema_attributes_attribute_tenant",
            ),
            (
                "INSERT INTO category_attribute_schema_assignments \
                 (id, tenant_id, category_id, mode, schema_id) VALUES \
                 ('30000000-0000-0000-0000-000000000004', \
                 '20000000-0000-0000-0000-000000000002', \
                 '20000000-0000-0000-0000-000000000222', 'use_schema', \
                 '10000000-0000-0000-0000-000000000131')",
                "fk_category_attribute_schema_assignments_schema_tenant",
            ),
            (
                "INSERT INTO product_attribute_values \
                 (id, tenant_id, product_id, attribute_id, value_text) VALUES \
                 ('30000000-0000-0000-0000-000000000005', \
                 '20000000-0000-0000-0000-000000000002', \
                 '20000000-0000-0000-0000-000000000202', \
                 '10000000-0000-0000-0000-000000000121', 'cross')",
                "not owned by tenant",
            ),
            (
                "INSERT INTO product_categories (tenant_id, product_id, category_id) VALUES \
                 ('20000000-0000-0000-0000-000000000002', \
                 '20000000-0000-0000-0000-000000000202', \
                 '10000000-0000-0000-0000-000000000111')",
                "fk_product_categories_category_tenant",
            ),
        ] {
            assert_constraint_rejection(&db, sql, constraint).await?;
        }

        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                r#"
SELECT
    (SELECT COUNT(*) FROM catalog_category_translations translation
     JOIN catalog_categories owner ON owner.id = translation.category_id
     WHERE owner.tenant_id = '10000000-0000-0000-0000-000000000001') AS category_count,
    (SELECT COUNT(*) FROM product_attribute_translations translation
     JOIN product_attributes owner ON owner.id = translation.attribute_id
     WHERE owner.tenant_id = '10000000-0000-0000-0000-000000000001') AS attribute_count,
    (SELECT COUNT(*) FROM product_attribute_schema_translations translation
     JOIN product_attribute_schemas owner ON owner.id = translation.schema_id
     WHERE owner.tenant_id = '10000000-0000-0000-0000-000000000001') AS schema_count
"#,
            ))
            .await?
            .ok_or("tenant-owned translation query returned no row")?;
        for column in ["category_count", "attribute_count", "schema_count"] {
            let count: i64 = row.try_get("", column)?;
            if count != 1 {
                return Err(format!(
                    "tenant-owned translation join {column} returned {count}, expected 1"
                )
                .into());
            }
        }

        Ok(())
    })
    .await
}

async fn assert_constraint_rejection(
    db: &DatabaseConnection,
    sql: &str,
    constraint: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = db
        .execute_raw(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect_err("cross-tenant Product storage write must be rejected");
    if !error.to_string().contains(constraint) {
        return Err(format!(
            "cross-tenant write expected {constraint}, got an unexpected error: {error}"
        )
        .into());
    }
    Ok(())
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

async fn seed_products(
    db: &DatabaseConnection,
    first_row: i32,
    last_row: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_raw(Statement::from_string(
        DbBackend::Postgres,
        format!(
            r#"
INSERT INTO products (
    id,
    tenant_id,
    status,
    metadata,
    created_at,
    updated_at,
    published_at
)
SELECT
    md5('product-plan-product-' || item::text)::uuid,
    md5('product-plan-tenant-' || ((item % 10) + 1)::text)::uuid,
    (
        CASE
            WHEN item % 13 = 0 THEN 'draft'
            WHEN item % 17 = 0 THEN 'archived'
            ELSE 'active'
        END
    )::product_status_enum,
    CASE
        WHEN item % 7 = 0 THEN
            '{{"channel_visibility":{{"allowed_channel_slugs":["wholesale"]}}}}'::jsonb
        ELSE '{{}}'::jsonb
    END,
    TIMESTAMPTZ '2025-01-01 00:00:00+00' + item * INTERVAL '1 second',
    TIMESTAMPTZ '2025-01-01 00:00:00+00' + item * INTERVAL '1 second',
    TIMESTAMPTZ '2025-01-01 00:00:00+00' + item * INTERVAL '1 second'
FROM generate_series({first_row}, {last_row}) AS item
"#
        ),
    ))
    .await?;
    Ok(())
}

async fn explain_storefront_page(
    db: &DatabaseConnection,
) -> Result<Value, Box<dyn std::error::Error>> {
    explain_json(
        db,
        r#"
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT id
FROM products
WHERE tenant_id = md5('product-plan-tenant-1')::uuid
  AND status = 'active'::product_status_enum
  AND published_at IS NOT NULL
  AND COALESCE(
      metadata #> '{channel_visibility,allowed_channel_slugs}',
      '[]'::jsonb
  ) = '[]'::jsonb
ORDER BY published_at DESC, created_at DESC
LIMIT 48
"#,
    )
    .await
}

async fn explain_storefront_count(
    db: &DatabaseConnection,
) -> Result<Value, Box<dyn std::error::Error>> {
    explain_json(
        db,
        r#"
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT COUNT(*)
FROM products
WHERE tenant_id = md5('product-plan-tenant-1')::uuid
  AND status = 'active'::product_status_enum
  AND published_at IS NOT NULL
  AND COALESCE(
      metadata #> '{channel_visibility,allowed_channel_slugs}',
      '[]'::jsonb
  ) = '[]'::jsonb
"#,
    )
    .await
}

async fn explain_json(
    db: &DatabaseConnection,
    sql: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .ok_or("EXPLAIN must return one row")?;
    Ok(row.try_get("", "QUERY PLAN")?)
}

fn execution_time_ms(plan: &Value) -> Result<f64, Box<dyn std::error::Error>> {
    plan.as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("Execution Time"))
        .and_then(Value::as_f64)
        .ok_or_else(|| "EXPLAIN plan is missing Execution Time".into())
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

async fn assert_target_schema(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    for (table, columns) in [
        (
            "products",
            &[
                "is_gift_card",
                "discountable",
                "weight",
                "length",
                "height",
                "width",
                "hs_code",
                "origin_country",
                "mid_code",
                "external_id",
                "deleted_at",
            ][..],
        ),
        ("product_translations", &["subtitle", "material"][..]),
        ("product_options", &["name", "values"][..]),
        ("product_images", &["url", "metadata", "created_at"][..]),
        (
            "product_variants",
            &[
                "length",
                "height",
                "width",
                "hs_code",
                "origin_country",
                "mid_code",
                "metadata",
                "deleted_at",
            ][..],
        ),
    ] {
        for column in columns {
            assert_column_missing(db, table, column).await?;
        }
    }

    assert_column_contract(db, "product_images", "media_id", "uuid", false).await?;
    assert_column_contract(db, "product_variants", "weight", "numeric", true).await?;
    Ok(())
}

async fn assert_target_values(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
INSERT INTO tenants (id)
VALUES ('00000000-0000-0000-0000-000000000001');
INSERT INTO products (id, tenant_id)
VALUES (
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000001'
);
INSERT INTO product_variants (id, product_id, tenant_id, weight)
VALUES (
    '00000000-0000-0000-0000-000000000121',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000001',
    1.25
);
"#,
    )
    .await?;

    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT weight FROM product_variants \
             WHERE id = '00000000-0000-0000-0000-000000000121'",
        ))
        .await?
        .ok_or("inserted weighted variant must exist")?;
    let weight: Decimal = row.try_get("", "weight")?;
    if weight != Decimal::new(125, 2) {
        return Err(format!("variant weight precision was not preserved: {weight}").into());
    }

    let error = db
        .execute_unprepared(
            r#"
INSERT INTO product_images (id, product_id, media_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000131',
    '00000000-0000-0000-0000-000000000101',
    NULL,
    0
)
"#,
        )
        .await
        .expect_err("a Product image without a Media-owned UUID must be rejected");
    if !error.to_string().contains("media_id") {
        return Err(format!("missing media_id returned an unexpected error: {error}").into());
    }
    Ok(())
}
