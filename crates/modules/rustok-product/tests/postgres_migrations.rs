use rust_decimal::Decimal;
use rustok_test_utils::{
    assert_postgres_column_contract as assert_column_contract,
    assert_postgres_column_missing as assert_column_missing,
    assert_postgres_table_missing as assert_table_missing,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::Value;

#[path = "support/postgres.rs"]
mod postgres;
use postgres::{ProductMigrator, with_product_postgres_database};

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
        assert_variant_membership_revision_contract(&db).await?;

        let latest = ProductMigrator::migrations()
            .pop()
            .ok_or("Product migration plan must not be empty")?;
        ProductMigrator::down(&db, Some(1)).await?;
        let pending = ProductMigrator::get_pending_migrations(&db).await?;
        if pending.len() != 1 || pending[0].name() != latest.name() {
            return Err(
                format!("Product rollback must leave only {} pending", latest.name()).into(),
            );
        }

        ProductMigrator::up(&db, Some(1)).await?;
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

INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, scope_value, canonical_key) VALUES
    ('10000000-0000-0000-0000-000000000111', '10000000-0000-0000-0000-000000000001', 'category', 'module', 'product', 'root'),
    ('10000000-0000-0000-0000-000000000113', '10000000-0000-0000-0000-000000000001', 'category', 'module', 'product', 'unbound'),
    ('20000000-0000-0000-0000-000000000222', '20000000-0000-0000-0000-000000000002', 'category', 'module', 'product', 'root');

INSERT INTO catalog_categories (id, tenant_id, code, path) VALUES
    ('10000000-0000-0000-0000-000000000111', '10000000-0000-0000-0000-000000000001', 'root', 'root'),
    ('20000000-0000-0000-0000-000000000222', '20000000-0000-0000-0000-000000000002', 'root', 'root');

INSERT INTO product_attributes (id, tenant_id, code, value_type) VALUES
    ('10000000-0000-0000-0000-000000000121', '10000000-0000-0000-0000-000000000001', 'material', 'text'),
    ('20000000-0000-0000-0000-000000000242', '20000000-0000-0000-0000-000000000002', 'material', 'text');

INSERT INTO product_attribute_schemas (id, tenant_id, code, status) VALUES
    ('10000000-0000-0000-0000-000000000131', '10000000-0000-0000-0000-000000000001', 'default', 'active'),
    ('20000000-0000-0000-0000-000000000262', '20000000-0000-0000-0000-000000000002', 'default', 'active');

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('10000000-0000-0000-0000-000000000141', '10000000-0000-0000-0000-000000000101', '10000000-0000-0000-0000-000000000001', 'en-US', 'Tenant one', 'shared'),
    ('20000000-0000-0000-0000-000000000282', '20000000-0000-0000-0000-000000000202', '20000000-0000-0000-0000-000000000002', 'en-US', 'Tenant two', 'shared');

INSERT INTO catalog_category_seo_translations (category_id, tenant_id, locale, meta_title) VALUES
    ('10000000-0000-0000-0000-000000000111', '10000000-0000-0000-0000-000000000001', 'en-US', 'Shared'),
    ('20000000-0000-0000-0000-000000000222', '20000000-0000-0000-0000-000000000002', 'en-US', 'Shared');
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
                "INSERT INTO catalog_categories (id, tenant_id, code, path, level) \
                 VALUES ('10000000-0000-0000-0000-000000000113', \
                 '20000000-0000-0000-0000-000000000002', \
                 'cross', 'root/cross', 1)",
                "fk_catalog_categories_taxonomy_term",
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
    (SELECT COUNT(*) FROM catalog_category_seo_translations translation
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
        ("catalog_categories", &["parent_id", "slug", "position"][..]),
    ] {
        for column in columns {
            assert_column_missing(db, table, column).await?;
        }
    }

    assert_column_contract(db, "product_images", "media_id", "uuid", false).await?;
    assert_column_contract(db, "product_variants", "weight", "numeric", true).await?;
    assert_column_contract(db, "products", "revision", "integer", false).await?;
    assert_column_contract(db, "product_variants", "revision", "bigint", false).await?;
    assert_table_missing(db, "product_catalog_category_taxonomy_bindings").await?;
    assert_table_missing(db, "catalog_category_closure").await?;
    assert_table_missing(db, "catalog_category_translations").await?;
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

    let revision_row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT revision, index_revision FROM product_variants \
             WHERE id = '00000000-0000-0000-0000-000000000121'",
        ))
        .await?
        .ok_or("inserted variant must expose its initial revisions")?;
    let initial_revision: i64 = revision_row.try_get("", "revision")?;
    let initial_index_revision: i64 = revision_row.try_get("", "index_revision")?;
    if initial_revision != 1 || initial_index_revision != 1 {
        return Err(format!(
            "new variant revisions must start at 1, got business={initial_revision}, index={initial_index_revision}"
        ).into());
    }

    db.execute_unprepared(
        "UPDATE product_variants SET inventory_quantity = inventory_quantity + 1 \
         WHERE id = '00000000-0000-0000-0000-000000000121'",
    )
    .await?;
    let updated_revision_row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT revision, index_revision FROM product_variants \
             WHERE id = '00000000-0000-0000-0000-000000000121'",
        ))
        .await?
        .ok_or("updated variant must remain available")?;
    let updated_revision: i64 = updated_revision_row.try_get("", "revision")?;
    let updated_index_revision: i64 = updated_revision_row.try_get("", "index_revision")?;
    if updated_revision != 2 || updated_index_revision != 2 {
        return Err(format!(
            "variant row updates must advance both counters once, got business={updated_revision}, index={updated_index_revision}"
        ).into());
    }

    // Pricing's owner touch updates the Index watermark without changing another field.
    // The independent business revision trigger must still advance the editor predecessor.
    db.execute_unprepared(
        "UPDATE product_variants SET index_revision = index_revision \
         WHERE id = '00000000-0000-0000-0000-000000000121'",
    )
    .await?;
    let touched_revision_row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT revision, index_revision FROM product_variants \
             WHERE id = '00000000-0000-0000-0000-000000000121'",
        ))
        .await?
        .ok_or("touched variant must remain available")?;
    let touched_revision: i64 = touched_revision_row.try_get("", "revision")?;
    let touched_index_revision: i64 = touched_revision_row.try_get("", "index_revision")?;
    if touched_revision != 3 || touched_index_revision != 3 {
        return Err(format!(
            "Pricing owner touch must advance both counters once, got business={touched_revision}, index={touched_index_revision}"
        ).into());
    }

    // A reused hard-delete identity must seed its business revision above the retained
    // predecessor as well as seeding the Index watermark above the tombstone.
    db.execute_unprepared(
        r#"
INSERT INTO product_variants (id, product_id, tenant_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000122',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000001',
    1
)
"#,
    )
    .await?;
    db.execute_unprepared(
        "UPDATE product_variants SET inventory_quantity = inventory_quantity + 1 \
         WHERE id = '00000000-0000-0000-0000-000000000122'",
    )
    .await?;
    db.execute_unprepared(
        "DELETE FROM product_variants WHERE id = '00000000-0000-0000-0000-000000000122'",
    )
    .await?;
    db.execute_unprepared(
        r#"
INSERT INTO product_variants (id, product_id, tenant_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000122',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000001',
    1
)
"#,
    )
    .await?;
    let recreated_revision_row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT revision, index_revision FROM product_variants \
             WHERE id = '00000000-0000-0000-0000-000000000122'",
        ))
        .await?
        .ok_or("recreated variant must expose retained revisions")?;
    let recreated_revision: i64 = recreated_revision_row.try_get("", "revision")?;
    let recreated_index_revision: i64 = recreated_revision_row.try_get("", "index_revision")?;
    if recreated_revision != 4 || recreated_index_revision != 4 {
        return Err(format!(
            "recreated variant revisions must advance above the delete tombstone, got business={recreated_revision}, index={recreated_index_revision}"
        ).into());
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

    db.execute_unprepared(
        r#"
INSERT INTO product_images (id, product_id, media_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000132',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000133',
    0
)
"#,
    )
    .await?;
    assert_constraint_rejection(
        db,
        r#"
INSERT INTO product_images (id, product_id, media_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000134',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000135',
    -1
)
"#,
        "chk_product_images_position_nonnegative",
    )
    .await?;
    assert_constraint_rejection(
        db,
        r#"
INSERT INTO product_images (id, product_id, media_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000136',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000137',
    0
)
"#,
        "uq_product_images_product_position",
    )
    .await?;

    // Build one configured axis through raw storage writes, then prove the deferred state
    // trigger also protects direct changes to product_variant_attribute_values. This closes the
    // gap where only option-row writes were previously observed by the invariant trigger.
    db.execute_unprepared(
        r#"
BEGIN;
INSERT INTO product_attributes (id, tenant_id, code, value_type, scope)
VALUES (
    '00000000-0000-0000-0000-000000000141',
    '00000000-0000-0000-0000-000000000001',
    'size',
    'select',
    'variant'
);
INSERT INTO product_attributes (id, tenant_id, code, value_type, scope)
VALUES (
    '00000000-0000-0000-0000-000000000146',
    '00000000-0000-0000-0000-000000000001',
    'color',
    'select',
    'variant'
);
INSERT INTO product_attribute_options (id, tenant_id, attribute_id, code)
VALUES (
    '00000000-0000-0000-0000-000000000142',
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000141',
    'small'
);
INSERT INTO product_variant_axes (id, tenant_id, product_id, attribute_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000143',
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000141',
    0
);
INSERT INTO product_variant_axis_values (id, tenant_id, axis_id, option_id, position)
VALUES (
    '00000000-0000-0000-0000-000000000144',
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000143',
    '00000000-0000-0000-0000-000000000142',
    0
);
INSERT INTO product_variant_attribute_values (id, tenant_id, variant_id, attribute_id)
VALUES (
    '00000000-0000-0000-0000-000000000145',
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000121',
    '00000000-0000-0000-0000-000000000141'
);
INSERT INTO product_variant_attribute_value_options (tenant_id, value_id, option_id)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000145',
    '00000000-0000-0000-0000-000000000142'
);
COMMIT;
"#,
    )
    .await?;
    assert_constraint_rejection(
        db,
        "UPDATE product_variant_attribute_values \
         SET attribute_id = '00000000-0000-0000-0000-000000000146' \
         WHERE id = '00000000-0000-0000-0000-000000000145'",
        "does not have exactly one active allowed assignment",
    )
    .await?;
    assert_constraint_rejection(
        db,
        "UPDATE product_attribute_options SET archived_at = now() \
         WHERE id = '00000000-0000-0000-0000-000000000142'",
        "inactive or mismatched variant axis option",
    )
    .await?;
    assert_constraint_rejection(
        db,
        "DELETE FROM product_variant_attribute_values \
         WHERE id = '00000000-0000-0000-0000-000000000145'",
        "does not have exactly one active allowed assignment",
    )
    .await?;
    Ok(())
}


async fn product_revision_pair(
    db: &DatabaseConnection,
    product_id: &str,
) -> Result<(i32, i64), Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT revision, index_revision FROM products WHERE id = '{product_id}'"
            ),
        ))
        .await?
        .ok_or("Product revision row must exist")?;
    Ok((
        row.try_get("", "revision")?,
        row.try_get("", "index_revision")?,
    ))
}

async fn product_variant_revision(
    db: &DatabaseConnection,
    tenant_id: &str,
    variant_id: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            format!("SELECT revision FROM product_variants WHERE tenant_id = '{tenant_id}' AND id = '{variant_id}'"),
        ))
        .await?
        .ok_or("ProductVariant revision row must exist")?;
    Ok(row.try_get("", "revision")?)
}

async fn assert_variant_membership_revision_contract(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    const TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";
    const PRODUCT_A: &str = "00000000-0000-0000-0000-000000000202";
    const PRODUCT_B: &str = "00000000-0000-0000-0000-000000000203";
    const EXHAUSTED_PRODUCT: &str = "00000000-0000-0000-0000-000000000204";
    const VARIANT_ID: &str = "00000000-0000-0000-0000-000000000223";
    const EXHAUSTED_VARIANT_ID: &str = "00000000-0000-0000-0000-000000000224";
    const PRODUCT_C: &str = "00000000-0000-0000-0000-000000000205";
    const AXIS_VARIANT_ID: &str = "00000000-0000-0000-0000-000000000225";

    db.execute_unprepared(&format!(
        "INSERT INTO products (id, tenant_id) VALUES
         ('{PRODUCT_A}', '{TENANT_ID}'),
         ('{PRODUCT_B}', '{TENANT_ID}')"
    ))
    .await?;
    if product_revision_pair(db, PRODUCT_A).await? != (1, 1)
        || product_revision_pair(db, PRODUCT_B).await? != (1, 1)
    {
        return Err("new Products must start with business and Index revisions at 1".into());
    }

    db.execute_unprepared(&format!(
        "INSERT INTO product_variants (id, product_id, tenant_id, position)
         VALUES ('{VARIANT_ID}', '{PRODUCT_A}', '{TENANT_ID}', 0)"
    ))
    .await?;
    if product_revision_pair(db, PRODUCT_A).await? != (2, 2) {
        return Err("variant insertion must advance the parent Product business and Index revisions once".into());
    }

    // Simulate the Product write path: it persists the full derived identity before COMMIT.
    // Deferred option-row triggers must not rewrite the Variant after its response snapshot.
    db.execute_unprepared(&format!(
        "INSERT INTO products (id, tenant_id) VALUES ('{PRODUCT_C}', '{TENANT_ID}')"
    )).await?;
    db.execute_unprepared(&format!(
        "INSERT INTO product_variants (id, product_id, tenant_id, position) VALUES ('{AXIS_VARIANT_ID}', '{PRODUCT_C}', '{TENANT_ID}', 0)"
    )).await?;
    if product_revision_pair(db, PRODUCT_C).await? != (2, 2) {
        return Err("axis test Variant insertion must advance its parent Product once".into());
    }
    const ATTRIBUTE_A: &str = "00000000-0000-0000-0000-000000000301";
    const ATTRIBUTE_B: &str = "00000000-0000-0000-0000-000000000302";
    const OPTION_A: &str = "00000000-0000-0000-0000-000000000311";
    const OPTION_B: &str = "00000000-0000-0000-0000-000000000312";
    const AXIS_A: &str = "00000000-0000-0000-0000-000000000321";
    const AXIS_B: &str = "00000000-0000-0000-0000-000000000322";
    const AXIS_VALUE_A: &str = "00000000-0000-0000-0000-000000000331";
    const AXIS_VALUE_B: &str = "00000000-0000-0000-0000-000000000332";
    const VALUE_A: &str = "00000000-0000-0000-0000-000000000341";
    const VALUE_B: &str = "00000000-0000-0000-0000-000000000342";
    let combination_identity = format!("{ATTRIBUTE_A}:{OPTION_A};{ATTRIBUTE_B}:{OPTION_B}");
    db.execute_unprepared(&format!(
        r#"
BEGIN;
INSERT INTO product_attributes (id, tenant_id, code, value_type, scope) VALUES
    ('{ATTRIBUTE_A}', '{TENANT_ID}', 'size_revision_test', 'select', 'variant'),
    ('{ATTRIBUTE_B}', '{TENANT_ID}', 'color_revision_test', 'select', 'variant');
INSERT INTO product_attribute_options (id, tenant_id, attribute_id, code) VALUES
    ('{OPTION_A}', '{TENANT_ID}', '{ATTRIBUTE_A}', 'small'),
    ('{OPTION_B}', '{TENANT_ID}', '{ATTRIBUTE_B}', 'blue');
INSERT INTO product_variant_axes (id, tenant_id, product_id, attribute_id, position) VALUES
    ('{AXIS_A}', '{TENANT_ID}', '{PRODUCT_C}', '{ATTRIBUTE_A}', 0),
    ('{AXIS_B}', '{TENANT_ID}', '{PRODUCT_C}', '{ATTRIBUTE_B}', 1);
INSERT INTO product_variant_axis_values (id, tenant_id, axis_id, option_id, position) VALUES
    ('{AXIS_VALUE_A}', '{TENANT_ID}', '{AXIS_A}', '{OPTION_A}', 0),
    ('{AXIS_VALUE_B}', '{TENANT_ID}', '{AXIS_B}', '{OPTION_B}', 0);
INSERT INTO product_variant_attribute_values (id, tenant_id, variant_id, attribute_id) VALUES
    ('{VALUE_A}', '{TENANT_ID}', '{AXIS_VARIANT_ID}', '{ATTRIBUTE_A}'),
    ('{VALUE_B}', '{TENANT_ID}', '{AXIS_VARIANT_ID}', '{ATTRIBUTE_B}');
INSERT INTO product_variant_attribute_value_options (tenant_id, value_id, option_id) VALUES
    ('{TENANT_ID}', '{VALUE_A}', '{OPTION_A}'),
    ('{TENANT_ID}', '{VALUE_B}', '{OPTION_B}');
UPDATE product_variants
SET combination_identity = '{combination_identity}'
WHERE tenant_id = '{TENANT_ID}' AND id = '{AXIS_VARIANT_ID}';
COMMIT;
"#
    )).await?;
    let revision_after_axis_assignment =
        product_variant_revision(db, TENANT_ID, AXIS_VARIANT_ID).await?;
    if revision_after_axis_assignment != 2 {
        return Err(format!(
            "deferred combination identity triggers must not bump an already-current Variant revision; got {revision_after_axis_assignment}, expected 2"
        ).into());
    }
    if product_revision_pair(db, PRODUCT_C).await? != (2, 2) {
        return Err("axis value maintenance must not advance the parent Product revision".into());
    }

    // Ordinary Variant field updates are not membership changes and must not advance Product CAS.
    db.execute_unprepared(&format!(
        "UPDATE product_variants
         SET inventory_quantity = inventory_quantity + 1
         WHERE id = '{VARIANT_ID}'"
    ))
    .await?;
    if product_revision_pair(db, PRODUCT_A).await? != (2, 2) {
        return Err("non-membership Variant updates must not advance the parent Product revision".into());
    }

    db.execute_unprepared(&format!(
        "DELETE FROM product_variants WHERE id = '{VARIANT_ID}'"
    ))
    .await?;
    if product_revision_pair(db, PRODUCT_A).await? != (3, 3) {
        return Err("variant deletion must advance the parent Product business and Index revisions once".into());
    }

    // Reusing a tombstoned Variant identity is a new membership event too.
    db.execute_unprepared(&format!(
        "INSERT INTO product_variants (id, product_id, tenant_id, position)
         VALUES ('{VARIANT_ID}', '{PRODUCT_A}', '{TENANT_ID}', 0)"
    ))
    .await?;
    if product_revision_pair(db, PRODUCT_A).await? != (4, 4) {
        return Err("recreated Variant membership must advance the parent Product revision".into());
    }

    db.execute_unprepared(&format!(
        "UPDATE product_variants
         SET product_id = '{PRODUCT_B}'
         WHERE id = '{VARIANT_ID}'"
    ))
    .await?;
    if product_revision_pair(db, PRODUCT_A).await? != (5, 5)
        || product_revision_pair(db, PRODUCT_B).await? != (2, 2)
    {
        return Err("moving a Variant must advance both affected Product revisions once".into());
    }

    db.execute_unprepared(&format!(
        "INSERT INTO products (id, tenant_id, revision)
         VALUES ('{EXHAUSTED_PRODUCT}', '{TENANT_ID}', 2147483647)"
    ))
    .await?;
    let error = db
        .execute_unprepared(&format!(
            "INSERT INTO product_variants (id, product_id, tenant_id, position)
             VALUES ('{EXHAUSTED_VARIANT_ID}', '{EXHAUSTED_PRODUCT}', '{TENANT_ID}', 0)"
        ))
        .await
        .expect_err("variant membership must refuse an exhausted Product revision");
    if !error
        .to_string()
        .contains("product revision exhausted for variant membership change")
    {
        return Err(format!("unexpected exhausted Product revision error: {error}").into());
    }
    let exhausted_variant_count: i64 = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT COUNT(*) AS count FROM product_variants WHERE id = '{EXHAUSTED_VARIANT_ID}'"
            ),
        ))
        .await?
        .ok_or("exhausted Variant count query must return one row")?
        .try_get("", "count")?;
    if exhausted_variant_count != 0 {
        return Err("an exhausted Product revision must not admit a new Variant".into());
    }

    Ok(())
}
