use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_reservation_items_external")
                    .table(ReservationItems::Table)
                    .col(ReservationItems::ExternalId)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE UNIQUE INDEX IF NOT EXISTS ux_reservation_items_active_external
                        ON reservation_items (inventory_item_id, external_id)
                        WHERE external_id IS NOT NULL AND deleted_at IS NULL
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                // MySQL needs a generated active-key column to emulate a partial
                // unique index. The owner service must still use external_id and
                // row locking there; do not add a misleading nullable unique key.
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !matches!(manager.get_database_backend(), DatabaseBackend::MySql) {
            manager
                .drop_index(
                    Index::drop()
                        .name("ux_reservation_items_active_external")
                        .table(ReservationItems::Table)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .drop_index(
                Index::drop()
                    .name("idx_reservation_items_external")
                    .table(ReservationItems::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ReservationItems {
    Table,
    ExternalId,
}
