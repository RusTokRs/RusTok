use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

const UNIQUE_DEFAULT_INDEX: &str = "uq_channels_one_default_per_tenant";
const MYSQL_DEFAULT_GUARD_COLUMN: &str = "default_channel_guard";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        reject_existing_duplicate_defaults(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: removing this invariant would reopen a cross-replica
        // resolution ambiguity and allow multiple defaults for one tenant.
        Ok(())
    }
}

async fn reject_existing_duplicate_defaults(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            backend,
            r#"
            SELECT tenant_id
            FROM channels
            WHERE is_default = TRUE
            GROUP BY tenant_id
            HAVING COUNT(*) > 1
            LIMIT 1
            "#
            .to_string(),
        ))
        .await?;

    if let Some(row) = duplicate {
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        return Err(DbErr::Custom(format!(
            "cannot enforce one default channel per tenant: tenant {tenant_id} has multiple default channels"
        )));
    }

    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            CREATE UNIQUE INDEX {UNIQUE_DEFAULT_INDEX}
                ON channels (tenant_id)
                WHERE is_default;
            "#
        ))
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS {UNIQUE_DEFAULT_INDEX}
                ON channels (tenant_id)
                WHERE is_default = 1;
            "#
        ))
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            ALTER TABLE channels
                ADD COLUMN {MYSQL_DEFAULT_GUARD_COLUMN} TINYINT
                    GENERATED ALWAYS AS (
                        CASE WHEN is_default THEN 1 ELSE NULL END
                    ) STORED,
                ADD UNIQUE INDEX {UNIQUE_DEFAULT_INDEX} (
                    tenant_id,
                    {MYSQL_DEFAULT_GUARD_COLUMN}
                );
            "#
        ))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn sqlite_channels_schema() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            r#"
            CREATE TABLE channels (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                is_default INTEGER NOT NULL
            );
            "#,
        )
        .await
        .expect("channels table");
        db
    }

    async fn insert_channel(
        db: &sea_orm::DatabaseConnection,
        id: Uuid,
        tenant_id: Uuid,
        is_default: bool,
    ) -> Result<(), sea_orm::DbErr> {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id, is_default) VALUES (?1, ?2, ?3)",
            vec![id.into(), tenant_id.into(), is_default.into()],
        ))
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_rejects_a_second_default_for_the_same_tenant() {
        let db = sqlite_channels_schema().await;
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("single-default invariant");

        let tenant_id = Uuid::new_v4();
        insert_channel(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("first default");
        assert!(
            insert_channel(&db, Uuid::new_v4(), tenant_id, true)
                .await
                .is_err()
        );
        insert_channel(&db, Uuid::new_v4(), tenant_id, false)
            .await
            .expect("non-default channel");
        insert_channel(&db, Uuid::new_v4(), Uuid::new_v4(), true)
            .await
            .expect("other tenant default");
    }

    #[tokio::test]
    async fn migration_rejects_preexisting_duplicate_defaults() {
        let db = sqlite_channels_schema().await;
        let tenant_id = Uuid::new_v4();
        insert_channel(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("first historical default");
        insert_channel(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("second historical default");

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("duplicate defaults must block migration");
        assert!(error.to_string().contains(&tenant_id.to_string()));
    }
}
