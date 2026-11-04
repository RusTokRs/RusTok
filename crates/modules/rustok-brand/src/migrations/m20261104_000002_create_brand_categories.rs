use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let is_postgres = manager.get_database_backend() == DatabaseBackend::Postgres;

        if is_postgres {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS brand_categories (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    brand_id UUID NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    category_id UUID NOT NULL,
    is_featured BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_brand_categories UNIQUE (tenant_id, brand_id, category_id)
);

CREATE INDEX IF NOT EXISTS idx_brand_categories_lookup
    ON brand_categories (tenant_id, category_id, brand_id);

CREATE INDEX IF NOT EXISTS idx_brand_categories_brand
    ON brand_categories (tenant_id, brand_id);
"#,
            )
            .await?;
        } else {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS brand_categories (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    brand_id TEXT NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL,
    is_featured INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, brand_id, category_id)
);

CREATE INDEX IF NOT EXISTS idx_brand_categories_lookup
    ON brand_categories (tenant_id, category_id, brand_id);

CREATE INDEX IF NOT EXISTS idx_brand_categories_brand
    ON brand_categories (tenant_id, brand_id);
"#,
            )
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE IF EXISTS brand_categories;")
            .await?;
        Ok(())
    }
}
