use sea_orm::{ConnectionTrait, DatabaseBackend, TransactionTrait};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        let backend = txn.get_database_backend();
        let json_type = match backend {
            DatabaseBackend::Postgres => "JSONB",
            DatabaseBackend::Sqlite => "JSONB_TEXT",
            _ => {
                return Err(DbErr::Custom(
                    "Product Flex schema projection requires PostgreSQL or SQLite".into(),
                ));
            }
        };
        txn.execute_unprepared(&format!(
            "CREATE TABLE product_flex_schema_projections (
                tenant_id UUID PRIMARY KEY NOT NULL,
                revision BIGINT NOT NULL CHECK (revision >= 0),
                schema_json {json_type} NOT NULL
            )"
        ))
        .await?;
        // This part tracks the existing Flex definition source in shared-schema installations.
        if SchemaManager::new(&txn)
            .has_table("product_field_definitions")
            .await?
        {
            txn.execute_unprepared(
                "CREATE TABLE flex_product_attached_schema_revisions (
                    tenant_id UUID PRIMARY KEY NOT NULL,
                    revision BIGINT NOT NULL CHECK (revision > 0)
                )",
            )
            .await?;
            match backend {
                DatabaseBackend::Postgres => {
                    txn.execute_unprepared(POSTGRES_TRIGGER).await.map(|_| ())?
                }
                _ => install_sqlite_triggers(&txn).await?,
            }
        }
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        if txn.get_database_backend() == DatabaseBackend::Postgres {
            if SchemaManager::new(&txn)
                .has_table("product_field_definitions")
                .await?
            {
                txn.execute_unprepared("DROP TRIGGER IF EXISTS flex_product_schema_revision ON product_field_definitions").await?;
                txn.execute_unprepared("DROP TRIGGER IF EXISTS flex_product_schema_revision_truncate ON product_field_definitions").await?;
            }
            txn.execute_unprepared("DROP FUNCTION IF EXISTS flex_bump_product_schema_revision()")
                .await?;
        } else {
            for suffix in ["insert", "delete", "update", "update_old"] {
                txn.execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS flex_product_schema_revision_{suffix}"
                ))
                .await?;
            }
        }
        txn.execute_unprepared("DROP TABLE IF EXISTS flex_product_attached_schema_revisions")
            .await?;
        txn.execute_unprepared("DROP TABLE product_flex_schema_projections")
            .await?;
        txn.commit().await
    }
}

const POSTGRES_TRIGGER: &str = "
CREATE FUNCTION flex_bump_product_schema_revision() RETURNS trigger AS $$
DECLARE affected_tenant UUID;
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        INSERT INTO flex_product_attached_schema_revisions (tenant_id, revision)
        SELECT DISTINCT tenant_id, 1 FROM product_field_definitions ORDER BY tenant_id
        ON CONFLICT (tenant_id) DO UPDATE SET revision = flex_product_attached_schema_revisions.revision + 1;
        RETURN NULL;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id THEN
            INSERT INTO flex_product_attached_schema_revisions (tenant_id, revision)
            VALUES (OLD.tenant_id, 1)
            ON CONFLICT (tenant_id) DO UPDATE SET revision = flex_product_attached_schema_revisions.revision + 1;
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN affected_tenant := OLD.tenant_id;
    ELSE affected_tenant := NEW.tenant_id;
    END IF;
    INSERT INTO flex_product_attached_schema_revisions (tenant_id, revision)
    VALUES (affected_tenant, 1)
    ON CONFLICT (tenant_id) DO UPDATE SET revision = flex_product_attached_schema_revisions.revision + 1;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER flex_product_schema_revision AFTER INSERT OR UPDATE OR DELETE ON product_field_definitions
FOR EACH ROW EXECUTE FUNCTION flex_bump_product_schema_revision();
CREATE TRIGGER flex_product_schema_revision_truncate BEFORE TRUNCATE ON product_field_definitions
FOR EACH STATEMENT EXECUTE FUNCTION flex_bump_product_schema_revision();";

async fn install_sqlite_triggers(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    for (suffix, operation, tenant, condition) in [
        ("insert", "INSERT", "NEW.tenant_id", ""),
        ("delete", "DELETE", "OLD.tenant_id", ""),
        ("update", "UPDATE", "NEW.tenant_id", ""),
        (
            "update_old",
            "UPDATE",
            "OLD.tenant_id",
            "WHEN OLD.tenant_id <> NEW.tenant_id",
        ),
    ] {
        db.execute_unprepared(&format!(
            "CREATE TRIGGER flex_product_schema_revision_{suffix} AFTER {operation} ON product_field_definitions
             {condition}
             BEGIN
                 INSERT INTO flex_product_attached_schema_revisions (tenant_id, revision) VALUES ({tenant}, 1)
                 ON CONFLICT (tenant_id) DO UPDATE SET revision = flex_product_attached_schema_revisions.revision + 1;
             END;"
        )).await?;
    }
    Ok(())
}
