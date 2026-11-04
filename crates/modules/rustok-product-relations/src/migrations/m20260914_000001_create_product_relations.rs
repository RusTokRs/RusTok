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
CREATE TABLE IF NOT EXISTS product_relations (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    related_product_id UUID NOT NULL,
    relation_type VARCHAR(32) NOT NULL,
    position INT NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_product_relations_no_self_relation CHECK (product_id != related_product_id),
    CONSTRAINT uq_product_relations_tenant_pair_type UNIQUE (tenant_id, product_id, related_product_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_product_relations_lookup
    ON product_relations (tenant_id, product_id, relation_type, position ASC);

CREATE INDEX IF NOT EXISTS idx_product_relations_reverse
    ON product_relations (tenant_id, related_product_id, relation_type);
"#,
            )
            .await?;
        } else {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS product_relations (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    related_product_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, related_product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    CHECK (product_id <> related_product_id),
    UNIQUE (tenant_id, product_id, related_product_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_product_relations_lookup
    ON product_relations (tenant_id, product_id, relation_type, position ASC);

CREATE INDEX IF NOT EXISTS idx_product_relations_reverse
    ON product_relations (tenant_id, related_product_id, relation_type);
"#,
            )
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS product_relations;")
            .await?;
        Ok(())
    }
}
