use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const OWNER_TABLE: &str = "product_attached_localized_values";
const SHARED_TABLE: &str = "flex_attached_localized_values";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        let manager = SchemaManager::new(&txn);
        let shared_exists = manager.has_table(SHARED_TABLE).await?;
        let backend = txn.get_database_backend();
        require_supported_database(backend)?;
        if shared_exists {
            lock_legacy_tables(&txn).await?;
            verify_legacy_rows(&txn).await?;
        }
        // Match SeaQuery's SQLite JSON type so numeric scalars keep text affinity.
        let value_type = if backend == DatabaseBackend::Sqlite {
            "JSONB_TEXT"
        } else {
            "JSONB"
        };
        txn.execute_unprepared(&format!(
            "CREATE TABLE product_attached_localized_values (
                id UUID PRIMARY KEY NOT NULL,
                tenant_id UUID NOT NULL,
                product_id UUID NOT NULL,
                field_key VARCHAR(128) NOT NULL,
                locale VARCHAR(32) NOT NULL,
                value {value_type} NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL,
                CONSTRAINT uq_product_attached_localized_value
                    UNIQUE (tenant_id, product_id, field_key, locale),
                CONSTRAINT fk_product_attached_localized_value_owner
                    FOREIGN KEY (tenant_id, product_id) REFERENCES products (tenant_id, id)
                    ON DELETE CASCADE
            )"
        ))
        .await?;

        if shared_exists {
            let copied = txn
                .execute_unprepared(
                    "INSERT INTO product_attached_localized_values
                    (id, tenant_id, product_id, field_key, locale, value, created_at, updated_at)
                 SELECT id, tenant_id, entity_id, field_key, locale, value, created_at, updated_at
                 FROM flex_attached_localized_values WHERE entity_type = 'product'",
                )
                .await?;
            let deleted = txn
                .execute_unprepared(
                    "DELETE FROM flex_attached_localized_values WHERE entity_type = 'product'",
                )
                .await?;
            if copied.rows_affected() != deleted.rows_affected() {
                return Err(DbErr::Custom(
                    "Product attached value transfer count mismatch".into(),
                ));
            }
            install_legacy_fence(&txn).await?;
        }
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        let manager = SchemaManager::new(&txn);
        let backend = txn.get_database_backend();
        require_supported_database(backend)?;
        let shared_exists = manager.has_table(SHARED_TABLE).await?;
        if shared_exists {
            lock_legacy_tables(&txn).await?;
        }
        if backend == DatabaseBackend::Postgres {
            txn.execute_unprepared(
                "LOCK TABLE product_attached_localized_values IN ACCESS EXCLUSIVE MODE",
            )
            .await?;
        }
        if shared_exists {
            remove_legacy_fence(&txn).await?;
            // A competing legacy row is an invariant failure. Do not overwrite it.
            txn.execute_unprepared(
                "INSERT INTO flex_attached_localized_values
                    (id, tenant_id, entity_type, entity_id, field_key, locale, value, created_at, updated_at)
                 SELECT id, tenant_id, 'product', product_id, field_key, locale, value, created_at, updated_at
                 FROM product_attached_localized_values",
            ).await?;
        } else if txn
            .query_one_raw(Statement::from_string(
                backend,
                format!("SELECT 1 FROM {OWNER_TABLE} LIMIT 1"),
            ))
            .await?
            .is_some()
        {
            return Err(DbErr::Custom(
                "Cannot roll back Product attached values without a legacy destination".into(),
            ));
        }
        txn.execute_unprepared("DROP TABLE product_attached_localized_values")
            .await?;
        txn.commit().await
    }
}

fn require_supported_database(backend: DatabaseBackend) -> Result<(), DbErr> {
    match backend {
        DatabaseBackend::Postgres | DatabaseBackend::Sqlite => Ok(()),
        _ => Err(DbErr::Custom(
            "Product attached values require PostgreSQL or SQLite".into(),
        )),
    }
}

async fn lock_legacy_tables(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    if db.get_database_backend() == DatabaseBackend::Postgres {
        // Product mutations lock the aggregate before touching localized values.
        db.execute_unprepared("LOCK TABLE products IN SHARE ROW EXCLUSIVE MODE")
            .await?;
        db.execute_unprepared("LOCK TABLE flex_attached_localized_values IN ACCESS EXCLUSIVE MODE")
            .await?;
    }
    Ok(())
}

async fn verify_legacy_rows(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    if db
        .query_one_raw(Statement::from_string(
            backend,
            "SELECT 1 FROM flex_attached_localized_values v
         WHERE v.entity_type = 'product' AND NOT EXISTS (
            SELECT 1 FROM products p WHERE p.tenant_id = v.tenant_id AND p.id = v.entity_id
         ) LIMIT 1",
        ))
        .await?
        .is_some()
    {
        return Err(DbErr::Custom(
            "Legacy Product attached values contain rows without a matching tenant and product"
                .into(),
        ));
    }
    let locales = db.query_all_raw(Statement::from_string(backend,
        "SELECT DISTINCT locale FROM flex_attached_localized_values WHERE entity_type = 'product'",
    )).await?;
    for row in locales {
        let locale: String = row.try_get("", "locale")?;
        if normalize_locale_tag(&locale).as_deref() != Some(locale.as_str()) {
            return Err(DbErr::Custom(
                "Legacy Product attached values contain a noncanonical locale".into(),
            ));
        }
    }
    Ok(())
}

async fn install_legacy_fence(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    if db.get_database_backend() == DatabaseBackend::Postgres {
        db.execute_unprepared(
            "ALTER TABLE flex_attached_localized_values ADD CONSTRAINT chk_flex_values_product_owner
             CHECK (entity_type <> 'product')",
        ).await?;
    } else {
        for operation in ["INSERT", "UPDATE"] {
            db.execute_unprepared(&format!(
                "CREATE TRIGGER flex_values_product_owner_{operation}
                 BEFORE {operation} ON flex_attached_localized_values
                 WHEN NEW.entity_type = 'product'
                 BEGIN SELECT RAISE(ABORT, 'Product attached values belong to Product'); END",
            ))
            .await?;
        }
    }
    Ok(())
}

async fn remove_legacy_fence(db: &impl ConnectionTrait) -> Result<(), DbErr> {
    if db.get_database_backend() == DatabaseBackend::Postgres {
        db.execute_unprepared(
            "ALTER TABLE flex_attached_localized_values DROP CONSTRAINT IF EXISTS chk_flex_values_product_owner",
        ).await?;
    } else {
        db.execute_unprepared("DROP TRIGGER IF EXISTS flex_values_product_owner_INSERT")
            .await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS flex_values_product_owner_UPDATE")
            .await?;
    }
    Ok(())
}
