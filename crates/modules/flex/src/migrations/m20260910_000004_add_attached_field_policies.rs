use sea_orm_migration::prelude::*;

const POLICIES_TABLE: &str = "flex_attached_field_policies";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Deliberately no FK to flex_attached_field_definitions: historical donors may keep
        // owner-specific definition storage while sharing this Flex-owned governance plane.
        manager
            .create_table(
                Table::create()
                    .table(Alias::new(POLICIES_TABLE))
                    .if_not_exists()
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
                        ColumnDef::new(Alias::new("classification"))
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("ai_export_allowed"))
                            .boolean()
                            .not_null(),
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
                    .primary_key(
                        Index::create()
                            .name("pk_flex_attached_field_policies")
                            .col(Alias::new("tenant_id"))
                            .col(Alias::new("entity_type"))
                            .col(Alias::new("field_key")),
                    )
                    .check(Expr::cust(
                        "classification IN ('public', 'tenant_private', 'personal', 'sensitive', 'secret', 'immutable_transaction')",
                    ))
                    .check(Expr::cust(
                        "ai_export_allowed = FALSE OR classification NOT IN ('secret', 'immutable_transaction')",
                    ))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new(POLICIES_TABLE))
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
