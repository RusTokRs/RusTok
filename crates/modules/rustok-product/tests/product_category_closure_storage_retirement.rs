use rustok_test_utils::{
    assert_postgres_url, assert_valid_postgres_database_name, connect_postgres,
    create_postgres_database, drop_postgres_database_if_exists, postgres_database_url,
    unique_postgres_database_name,
};
use sea_orm_migration::{
    MigrationTrait, SchemaManager,
    prelude::MigratorTrait,
    sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait},
};

struct ProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn product_category_closure_storage_supports_up_down_up() {
    if let Err(error) = run_product_category_closure_storage_lifecycle().await {
        panic!("Product Category closure storage lifecycle failed: {error}");
    }
}

async fn run_product_category_closure_storage_lifecycle() -> Result<(), Box<dyn std::error::Error>>
{
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = std::env::var("RUSTOK_MIGRATION_SMOKE_DB_NAME")
        .unwrap_or_else(|_| unique_postgres_database_name("rustok_cat34_product"));
    assert_valid_postgres_database_name(&database_name);
    let target_url = postgres_database_url(&admin_url, &database_name);
    let keep_database = matches!(
        std::env::var("RUSTOK_MIGRATION_SMOKE_KEEP_DB")
            .ok()
            .as_deref(),
        Some("1")
    );

    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let lifecycle_result = async {
        let db = connect_postgres(&target_url).await?;
        create_product_migration_prerequisites(&db).await?;

        let migration_count = rustok_product::migrations::migrations().len();
        if migration_count < 2 {
            return Err("Product migration plan must contain CAT-33 and CAT-34".into());
        }
        ProductMigrator::up(&db, Some((migration_count - 1) as u32))
            .await
            .map_err(|error| format!("Product migrations through CAT-33 failed: {error}"))?;
        assert_cat33_storage(&db).await?;
        seed_cat33_category_tree(&db).await?;

        ProductMigrator::up(&db, Some(1))
            .await
            .map_err(|error| format!("CAT-34 apply failed: {error}"))?;
        assert_cat34_head(&db).await?;

        ProductMigrator::down(&db, Some(1))
            .await
            .map_err(|error| format!("CAT-34 rollback failed: {error}"))?;
        assert_cat33_storage(&db).await?;
        assert_reconstructed_closure(&db).await?;
        let pending = ProductMigrator::get_pending_migrations(&db).await?;
        if pending.len() != 1
            || pending[0].name() != "m20260829_000020_retire_product_category_closure_storage"
        {
            let names = pending
                .iter()
                .map(|migration| migration.name().to_string())
                .collect::<Vec<_>>();
            return Err(
                format!("CAT-34 rollback must expose only CAT-34 as pending: {names:?}").into(),
            );
        }

        ProductMigrator::up(&db, Some(1))
            .await
            .map_err(|error| format!("CAT-34 reapply failed: {error}"))?;
        assert_cat34_head(&db).await?;
        let pending = ProductMigrator::get_pending_migrations(&db).await?;
        if !pending.is_empty() {
            let names = pending
                .iter()
                .map(|migration| migration.name().to_string())
                .collect::<Vec<_>>();
            return Err(format!(
                "CAT-34 reapply must leave no pending Product migrations: {names:?}"
            )
            .into());
        }

        db.close().await?;
        Ok(())
    }
    .await;

    if keep_database {
        eprintln!("Keeping CAT-34 Product migration database '{database_name}' at {target_url}");
    } else {
        drop_postgres_database_if_exists(&admin, &database_name).await?;
    }
    lifecycle_result
}

async fn create_product_migration_prerequisites(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
CREATE TABLE tenants (
    id UUID PRIMARY KEY
);
"#,
    )
    .await?;

    let manager = SchemaManager::new(db);
    let required_taxonomy_migration = "m20260822_000010_create_taxonomy_category_hierarchy";
    let mut taxonomy_hierarchy_ready = false;
    for migration in rustok_taxonomy::migrations::migrations() {
        let migration_name = migration.name().to_string();
        migration.up(&manager).await.map_err(|error| {
            format!("Taxonomy prerequisite migration {migration_name} failed: {error}")
        })?;
        if migration_name == required_taxonomy_migration {
            taxonomy_hierarchy_ready = true;
            break;
        }
    }
    if !taxonomy_hierarchy_ready {
        return Err(format!(
            "Product CAT-16 requires Taxonomy migration {required_taxonomy_migration}"
        )
        .into());
    }

    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

async fn seed_cat33_category_tree(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let transaction = db.begin().await?;
    transaction
        .execute_unprepared(
            r#"
INSERT INTO tenants (id)
VALUES ('41000000-0000-0000-0000-000000000001');

INSERT INTO catalog_categories (
    id, tenant_id, parent_id, code, slug, kind, path, level, position,
    is_active, rule_config, metadata
) VALUES
    (
        '41000000-0000-0000-0000-000000000010',
        '41000000-0000-0000-0000-000000000001',
        NULL,
        'cat34-root',
        'cat34-root',
        'structural',
        'cat34-root',
        0,
        0,
        TRUE,
        '{}'::jsonb,
        '{}'::jsonb
    ),
    (
        '41000000-0000-0000-0000-000000000011',
        '41000000-0000-0000-0000-000000000001',
        '41000000-0000-0000-0000-000000000010',
        'cat34-child',
        'cat34-child',
        'structural',
        'cat34-root/cat34-child',
        1,
        0,
        TRUE,
        '{}'::jsonb,
        '{}'::jsonb
    );

INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth) VALUES
    (
        '41000000-0000-0000-0000-000000000001',
        '41000000-0000-0000-0000-000000000010',
        '41000000-0000-0000-0000-000000000010',
        0
    ),
    (
        '41000000-0000-0000-0000-000000000001',
        '41000000-0000-0000-0000-000000000011',
        '41000000-0000-0000-0000-000000000011',
        0
    ),
    (
        '41000000-0000-0000-0000-000000000001',
        '41000000-0000-0000-0000-000000000010',
        '41000000-0000-0000-0000-000000000011',
        1
    );
"#,
        )
        .await?;
    transaction.commit().await?;
    Ok(())
}

async fn assert_cat33_storage(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
DO $$
BEGIN
    IF to_regclass('public.catalog_category_closure') IS NULL THEN
        RAISE EXCEPTION 'CAT-33 must expose catalog_category_closure';
    END IF;
    IF NOT EXISTS (
        SELECT 1
        FROM pg_trigger trigger
        JOIN pg_class relation ON relation.oid = trigger.tgrelid
        WHERE relation.relname = 'catalog_category_closure'
          AND trigger.tgname = 'trg_catalog_category_closure_validate_tree'
          AND NOT trigger.tgisinternal
    ) THEN
        RAISE EXCEPTION 'CAT-33 closure compatibility trigger is missing';
    END IF;
END;
$$;
"#,
    )
    .await?;
    Ok(())
}

async fn assert_cat34_head(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
DO $$
BEGIN
    IF to_regclass('public.catalog_category_closure') IS NOT NULL THEN
        RAISE EXCEPTION 'CAT-34 must retire catalog_category_closure';
    END IF;
    IF NOT EXISTS (
        SELECT 1
        FROM pg_trigger trigger
        JOIN pg_class relation ON relation.oid = trigger.tgrelid
        WHERE relation.relname = 'catalog_categories'
          AND trigger.tgname = 'trg_catalog_categories_validate_tree'
          AND NOT trigger.tgisinternal
    ) THEN
        RAISE EXCEPTION 'CAT-34 retained category cycle trigger is missing';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM pg_get_functiondef('rustok_product_assert_category_tree()'::regprocedure) definition
        WHERE definition LIKE '%catalog_category_closure%'
    ) THEN
        RAISE EXCEPTION 'CAT-34 retained category-tree assertion must be closure-independent';
    END IF;
END;
$$;
"#,
    )
    .await?;
    Ok(())
}

async fn assert_reconstructed_closure(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
DO $$
BEGIN
    IF EXISTS (
        WITH expected(ancestor_id, descendant_id, depth) AS (
            VALUES
                ('41000000-0000-0000-0000-000000000010'::uuid, '41000000-0000-0000-0000-000000000010'::uuid, 0),
                ('41000000-0000-0000-0000-000000000011'::uuid, '41000000-0000-0000-0000-000000000011'::uuid, 0),
                ('41000000-0000-0000-0000-000000000010'::uuid, '41000000-0000-0000-0000-000000000011'::uuid, 1)
        ),
        actual AS (
            SELECT ancestor_id, descendant_id, depth
            FROM catalog_category_closure
            WHERE tenant_id = '41000000-0000-0000-0000-000000000001'::uuid
        )
        (SELECT * FROM expected EXCEPT SELECT * FROM actual)
        UNION ALL
        (SELECT * FROM actual EXCEPT SELECT * FROM expected)
    ) THEN
        RAISE EXCEPTION 'CAT-34 rollback did not reconstruct exact CAT-33 closure rows';
    END IF;
END;
$$;
"#,
    )
    .await?;
    Ok(())
}
