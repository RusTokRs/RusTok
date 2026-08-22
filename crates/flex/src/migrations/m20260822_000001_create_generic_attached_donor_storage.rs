use sea_orm_migration::prelude::*;

use crate::cache_generation::{
    create_field_definition_cache_generation_trigger,
    drop_field_definition_cache_generation_trigger,
};

const DEFINITIONS_TABLE: &str = "flex_attached_field_definitions";
const VALUES_TABLE: &str = "flex_attached_values";
const CACHE_TRIGGER: &str = "flex_attached_fd_cache_generation";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new(DEFINITIONS_TABLE))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("entity_type"))
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("field_key"))
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("field_type"))
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("label")).json_binary().not_null())
                    .col(ColumnDef::new(Alias::new("description")).json_binary().null())
                    .col(
                        ColumnDef::new(Alias::new("is_localized"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_required"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Alias::new("default_value")).json_binary().null())
                    .col(ColumnDef::new(Alias::new("validation")).json_binary().null())
                    .col(
                        ColumnDef::new(Alias::new("position"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Alias::new("is_active"))
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_flex_attached_fd_scope_key")
                    .table(Alias::new(DEFINITIONS_TABLE))
                    .unique()
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("entity_type"))
                    .col(Alias::new("field_key"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_flex_attached_fd_scope_position")
                    .table(Alias::new(DEFINITIONS_TABLE))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("entity_type"))
                    .col(Alias::new("position"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new(VALUES_TABLE))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("entity_type"))
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("entity_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("data")).json_binary().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_flex_attached_values_owner")
                    .table(Alias::new(VALUES_TABLE))
                    .unique()
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("entity_type"))
                    .col(Alias::new("entity_id"))
                    .to_owned(),
            )
            .await?;

        create_field_definition_cache_generation_trigger(
            manager,
            DEFINITIONS_TABLE,
            CACHE_TRIGGER,
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_field_definition_cache_generation_trigger(manager, DEFINITIONS_TABLE, CACHE_TRIGGER)
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new(VALUES_TABLE)).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new(DEFINITIONS_TABLE)).to_owned())
            .await
    }
}
