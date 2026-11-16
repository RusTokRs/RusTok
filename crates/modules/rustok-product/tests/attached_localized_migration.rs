use rustok_product::migrations::m20261116_000041_move_product_attached_localized_values::Migration;
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};

const SETUP: &str = r#"
CREATE TABLE products (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    UNIQUE (tenant_id, id)
);
CREATE TABLE flex_attached_localized_values (
    id UUID PRIMARY KEY, tenant_id UUID NOT NULL, entity_type VARCHAR(64) NOT NULL,
    entity_id UUID NOT NULL, field_key VARCHAR(128) NOT NULL, locale VARCHAR(32) NOT NULL,
    value JSONB NOT NULL, created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, entity_type, entity_id, field_key, locale)
);
INSERT INTO products VALUES (
    '11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222'
);
INSERT INTO flex_attached_localized_values VALUES
('33333333-3333-4333-8333-333333333333', '22222222-2222-4222-8222-222222222222',
 'product', '11111111-1111-4111-8111-111111111111', 'title', 'en', '"Hello"', '2026-01-01 10:00:00+00', '2026-01-02 11:00:00+00'),
('77777777-7777-4777-8777-777777777777', '22222222-2222-4222-8222-222222222222',
 'product', '11111111-1111-4111-8111-111111111111', 'rating', 'en', '42', '2026-01-01 10:00:00+00', '2026-01-02 11:00:00+00'),
('44444444-4444-4444-8444-444444444444', '22222222-2222-4222-8222-222222222222',
 'taxonomy.category', '55555555-5555-4555-8555-555555555555', 'title', 'en', '"Category"', '2026-01-01 10:00:00+00', '2026-01-02 11:00:00+00');
"#;

async fn count(db: &DatabaseConnection, query: &str) -> Result<i64, DbErr> {
    db.query_one_raw(Statement::from_string(db.get_database_backend(), query))
        .await?
        .ok_or_else(|| DbErr::Custom("COUNT did not return a row".into()))?
        .try_get("", "total")
}

async fn verify_transfer(db: DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    if db.get_database_backend() == DatabaseBackend::Sqlite {
        db.execute_unprepared(&SETUP.replace("value JSONB", "value JSONB_TEXT"))
            .await?;
    } else {
        db.execute_unprepared(SETUP).await?;
    }
    let manager = SchemaManager::new(&db);

    db.execute_unprepared(
        "UPDATE flex_attached_localized_values SET locale = 'EN' WHERE entity_type = 'product'",
    )
    .await?;
    assert!(Migration.up(&manager).await.is_err());
    assert!(
        !manager
            .has_table("product_attached_localized_values")
            .await?
    );
    db.execute_unprepared(
        "UPDATE flex_attached_localized_values SET locale = 'en',
         tenant_id = '66666666-6666-4666-8666-666666666666' WHERE entity_type = 'product'",
    )
    .await?;
    assert!(Migration.up(&manager).await.is_err());
    assert!(
        !manager
            .has_table("product_attached_localized_values")
            .await?
    );
    db.execute_unprepared(
        "UPDATE flex_attached_localized_values SET tenant_id = '22222222-2222-4222-8222-222222222222'
         WHERE entity_type = 'product'",
    ).await?;
    db.execute_unprepared(
        "CREATE TABLE expected_values AS SELECT id, tenant_id, entity_id AS product_id, field_key,
            locale, value, created_at, updated_at FROM flex_attached_localized_values
         WHERE entity_type = 'product'",
    )
    .await?;

    Migration.up(&manager).await?;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM (
            SELECT id, tenant_id, product_id, field_key, locale, value, created_at, updated_at
                FROM product_attached_localized_values
            INTERSECT
            SELECT id, tenant_id, product_id, field_key, locale, value, created_at, updated_at
                FROM expected_values
         ) matched"
        )
        .await?,
        2,
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM product_attached_localized_values"
        )
        .await?,
        2
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM flex_attached_localized_values"
        )
        .await?,
        1
    );
    assert!(
        db.execute_unprepared("UPDATE flex_attached_localized_values SET entity_type = 'product'")
            .await
            .is_err()
    );
    assert!(db.execute_unprepared(
        "INSERT INTO flex_attached_localized_values
         SELECT id, tenant_id, 'product', product_id, field_key, locale, value, created_at, updated_at
         FROM expected_values"
    ).await.is_err());
    assert!(
        db.execute_unprepared(
            "UPDATE product_attached_localized_values
         SET tenant_id = '66666666-6666-4666-8666-666666666666'"
        )
        .await
        .is_err()
    );
    db.execute_unprepared(
        r#"UPDATE product_attached_localized_values SET value = '"Changed"' WHERE field_key = 'title'"#,
    )
    .await?;
    db.execute_unprepared("UPDATE flex_attached_localized_values SET field_key = 'tagline' WHERE entity_type = 'taxonomy.category'").await?;

    Migration.down(&manager).await?;
    assert!(
        !manager
            .has_table("product_attached_localized_values")
            .await?
    );
    assert_eq!(
        count(
            &db,
            r#"SELECT COUNT(*) AS total FROM flex_attached_localized_values
            WHERE entity_type = 'product' AND value = '"Changed"'"#
        )
        .await?,
        1,
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM flex_attached_localized_values
            WHERE entity_type = 'product' AND field_key = 'rating' AND value = '42'"
        )
        .await?,
        1,
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM flex_attached_localized_values
            WHERE entity_type = 'taxonomy.category' AND field_key = 'tagline'"
        )
        .await?,
        1,
    );

    Migration.up(&manager).await?;
    db.execute_unprepared("DELETE FROM products").await?;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM product_attached_localized_values"
        )
        .await?,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS total FROM flex_attached_localized_values"
        )
        .await?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn sqlite_moves_fences_restores_and_cascades_product_values()
-> Result<(), Box<dyn std::error::Error>> {
    let db = Database::connect("sqlite::memory:").await?;
    db.execute_unprepared("PRAGMA foreign_keys = ON").await?;
    verify_transfer(db).await
}

#[tokio::test]
#[ignore = "requires RUSTOK_MIGRATION_SMOKE_ADMIN_URL for disposable PostgreSQL database creation"]
async fn postgres_moves_fences_restores_and_cascades_product_values()
-> Result<(), Box<dyn std::error::Error>> {
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    assert_postgres_url(&admin_url);
    let admin = connect_postgres(&admin_url).await?;
    let name = unique_postgres_database_name("product_attached_move");
    create_postgres_database(&admin, &name).await?;
    let result = async {
        let db = connect_postgres(&postgres_database_url(&admin_url, &name)).await?;
        verify_transfer(db).await
    }
    .await;
    let cleanup = drop_postgres_database_if_exists(&admin, &name).await;
    result?;
    cleanup?;
    Ok(())
}
