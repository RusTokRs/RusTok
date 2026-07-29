use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

const UNIQUE_ACTIVE_INDEX: &str = "uq_channel_policy_sets_one_active_per_tenant";
const MYSQL_ACTIVE_GUARD_COLUMN: &str = "active_policy_set_guard";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        reject_existing_duplicate_active_sets(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: multiple active policy sets make channel resolution
        // nondeterministic and must not be re-enabled by rollback.
        Ok(())
    }
}

async fn reject_existing_duplicate_active_sets(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            r#"
            SELECT tenant_id
            FROM channel_resolution_policy_sets
            WHERE is_active = TRUE
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
            "cannot enforce one active channel policy set per tenant: tenant {tenant_id} has multiple active policy sets"
        )));
    }
    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "CREATE UNIQUE INDEX {UNIQUE_ACTIVE_INDEX} ON channel_resolution_policy_sets (tenant_id) WHERE is_active"
        ))
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS {UNIQUE_ACTIVE_INDEX} ON channel_resolution_policy_sets (tenant_id) WHERE is_active = 1"
        ))
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            ALTER TABLE channel_resolution_policy_sets
                ADD COLUMN {MYSQL_ACTIVE_GUARD_COLUMN} TINYINT
                    GENERATED ALWAYS AS (
                        CASE WHEN is_active THEN 1 ELSE NULL END
                    ) STORED,
                ADD UNIQUE INDEX {UNIQUE_ACTIVE_INDEX} (
                    tenant_id,
                    {MYSQL_ACTIVE_GUARD_COLUMN}
                )
            "#
        ))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn sqlite_policy_schema() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            r#"
            CREATE TABLE channel_resolution_policy_sets (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                is_active INTEGER NOT NULL
            );
            "#,
        )
        .await
        .expect("policy-set table");
        db
    }

    async fn insert_policy_set(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        is_active: bool,
    ) -> Result<(), sea_orm::DbErr> {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_resolution_policy_sets (id, tenant_id, is_active) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), tenant_id.into(), is_active.into()],
        ))
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_rejects_a_second_active_policy_set_for_the_same_tenant() {
        let db = sqlite_policy_schema().await;
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("single-active invariant");

        let tenant_id = Uuid::new_v4();
        insert_policy_set(&db, tenant_id, true)
            .await
            .expect("first active set");
        assert!(insert_policy_set(&db, tenant_id, true).await.is_err());
        insert_policy_set(&db, tenant_id, false)
            .await
            .expect("inactive set");
        insert_policy_set(&db, Uuid::new_v4(), true)
            .await
            .expect("other tenant active set");
    }

    #[tokio::test]
    async fn migration_rejects_preexisting_duplicate_active_sets() {
        let db = sqlite_policy_schema().await;
        let tenant_id = Uuid::new_v4();
        insert_policy_set(&db, tenant_id, true)
            .await
            .expect("first historical active set");
        insert_policy_set(&db, tenant_id, true)
            .await
            .expect("second historical active set");

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("duplicate active sets must block migration");
        assert!(error.to_string().contains(&tenant_id.to_string()));
    }
}
