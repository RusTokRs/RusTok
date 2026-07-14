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
            .drop_table(
                Table::drop()
                    .table(RbacInvalidationState::Table)
                    .to_owned(),
            )
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
