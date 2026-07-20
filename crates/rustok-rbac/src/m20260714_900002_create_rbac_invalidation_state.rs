//! Creates the durable RBAC permission invalidation generation store.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

const RBAC_PERMISSION_SCOPE: &str = "permissions";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RbacInvalidationState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RbacInvalidationState::Scope)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RbacInvalidationState::Generation)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RbacInvalidationState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(&format!(
                "INSERT INTO rbac_invalidation_state (scope, generation, updated_at) \
                 SELECT '{RBAC_PERMISSION_SCOPE}', 0, CURRENT_TIMESTAMP \
                 WHERE NOT EXISTS ( \
                     SELECT 1 FROM rbac_invalidation_state \
                     WHERE scope = '{RBAC_PERMISSION_SCOPE}' \
                 )"
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RbacInvalidationState::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum RbacInvalidationState {
    Table,
    Scope,
    Generation,
    UpdatedAt,
}

#[cfg(test)]
mod tests {
    use super::Migration;
    use sea_orm_migration::MigrationTrait;
    use sea_orm_migration::prelude::SchemaManager;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    #[tokio::test]
    async fn replay_keeps_one_row_and_preserves_advanced_generation() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let manager = SchemaManager::new(&db);
        let migration = Migration;
        migration.up(&manager).await.unwrap();
        db.execute_unprepared(
            "UPDATE rbac_invalidation_state SET generation = 9 WHERE scope = 'permissions'",
        )
        .await
        .unwrap();

        migration.up(&manager).await.unwrap();

        let row = db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS row_count, MAX(generation) AS generation FROM rbac_invalidation_state WHERE scope = 'permissions'".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<i64>("", "row_count").unwrap(), 1);
        assert_eq!(row.try_get::<i64>("", "generation").unwrap(), 9);
    }
}
