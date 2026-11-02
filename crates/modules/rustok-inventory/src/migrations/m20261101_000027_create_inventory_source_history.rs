//! Empty owner tables only. No implicit baseline, historical backfill or Index storage writes.
use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create().table(Alias::new("inventory_source_baselines"))
                .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null().primary_key())
                .col(ColumnDef::new(Alias::new("baseline_id")).uuid().not_null().unique_key())
                .col(ColumnDef::new(Alias::new("actor_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("active")).boolean().not_null())
                .col(ColumnDef::new(Alias::new("last_variant_id")).uuid())
                .col(ColumnDef::new(Alias::new("record_count")).big_integer().not_null())
                .col(ColumnDef::new(Alias::new("last_page_id")).uuid())
                .col(ColumnDef::new(Alias::new("last_page_after")).uuid())
                .col(ColumnDef::new(Alias::new("last_page_limit")).integer())
                .col(ColumnDef::new(Alias::new("created_at")).timestamp_with_time_zone().not_null())
                .col(ColumnDef::new(Alias::new("updated_at")).timestamp_with_time_zone().not_null())
                .check(Expr::cust("record_count >= 0 AND ((last_page_id IS NULL AND last_page_after IS NULL AND last_page_limit IS NULL) OR (last_page_id IS NOT NULL AND last_page_limit BETWEEN 1 AND 128))"))
                .to_owned()
        ).await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("inventory_source_baseline_pages"))
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("page_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("baseline_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("after_variant_id")).uuid())
                    .col(
                        ColumnDef::new(Alias::new("page_limit"))
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("next_after")).uuid())
                    .col(ColumnDef::new(Alias::new("active")).boolean().not_null())
                    .col(
                        ColumnDef::new(Alias::new("record_count"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(Alias::new("tenant_id"))
                            .col(Alias::new("page_id")),
                    )
                    .check(Expr::cust(
                        "page_limit BETWEEN 1 AND 128 AND record_count >= 0",
                    ))
                    .to_owned(),
            )
            .await?;
        manager.create_table(
            Table::create().table(Alias::new("inventory_source_records"))
                .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("variant_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("baseline_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("inventory_item_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("product_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("source_version")).big_integer().not_null())
                .col(ColumnDef::new(Alias::new("deleted")).boolean().not_null())
                .col(ColumnDef::new(Alias::new("stocked")).big_integer().not_null())
                .col(ColumnDef::new(Alias::new("reserved")).big_integer().not_null())
                .col(ColumnDef::new(Alias::new("incoming")).big_integer().not_null())
                .col(ColumnDef::new(Alias::new("binding_digest")).string_len(64).not_null())
                .col(ColumnDef::new(Alias::new("last_change_id")).uuid().not_null().unique_key())
                .col(ColumnDef::new(Alias::new("updated_at")).timestamp_with_time_zone().not_null())
                .primary_key(Index::create().col(Alias::new("tenant_id")).col(Alias::new("variant_id")))
                .check(Expr::cust("source_version > 0 AND stocked >= 0 AND reserved >= 0 AND incoming >= 0 AND (NOT deleted OR (stocked = 0 AND reserved = 0 AND incoming = 0))"))
                .to_owned()
        ).await?;
        manager.create_table(
            Table::create().table(Alias::new("inventory_source_changes"))
                .col(ColumnDef::new(Alias::new("change_id")).uuid().not_null().primary_key())
                .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("variant_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("baseline_id")).uuid().not_null())
                .col(ColumnDef::new(Alias::new("source_version")).big_integer().not_null())
                .col(ColumnDef::new(Alias::new("kind")).string_len(16).not_null())
                .col(ColumnDef::new(Alias::new("created_at")).timestamp_with_time_zone().not_null())
                .check(Expr::cust("source_version > 0 AND kind IN ('baseline', 'created', 'changed', 'deleted')"))
                .to_owned()
        ).await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_source_changes_identity_version")
                    .table(Alias::new("inventory_source_changes"))
                    .unique()
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("variant_id"))
                    .col(Alias::new("source_version"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let tables = [
            "inventory_source_changes",
            "inventory_source_records",
            "inventory_source_baseline_pages",
            "inventory_source_baselines",
        ];
        // Verify every table before dropping any: retained baselines, history and tombstones survive.
        for table in tables {
            if manager.has_table(table).await?
                && manager
                    .get_connection()
                    .query_one_raw(Statement::from_string(
                        manager.get_database_backend(),
                        format!("SELECT 1 FROM {table} LIMIT 1"),
                    ))
                    .await?
                    .is_some()
            {
                return Err(DbErr::Custom("Inventory source history is retained; downgrade cannot discard nonempty owner tables".into()));
            }
        }
        for table in tables {
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new(table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
