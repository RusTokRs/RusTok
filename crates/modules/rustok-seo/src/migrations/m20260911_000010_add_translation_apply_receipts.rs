use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("seo_translation_apply_receipts"))
                    .if_not_exists()
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("idempotency_key")).string_len(160).not_null())
                    .col(ColumnDef::new(Alias::new("actor_user_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("target_kind")).string_len(128).not_null())
                    .col(ColumnDef::new(Alias::new("target_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("request_fingerprint")).string_len(96).not_null())
                    .col(ColumnDef::new(Alias::new("operation_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("resource_revision")).string_len(96).not_null())
                    .col(ColumnDef::new(Alias::new("source_revision")).string_len(96).not_null())
                    .col(ColumnDef::new(Alias::new("target_revision")).string_len(96).not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_seo_translation_apply_receipts")
                            .col(Alias::new("tenant_id"))
                            .col(Alias::new("idempotency_key")),
                    )
                    .check(Expr::cust("length(trim(idempotency_key)) > 0"))
                    .check(Expr::cust("length(trim(target_kind)) > 0"))
                    .check(Expr::cust("length(trim(request_fingerprint)) > 0"))
                    .check(Expr::cust("length(trim(resource_revision)) > 0"))
                    .check(Expr::cust("length(trim(source_revision)) > 0"))
                    .check(Expr::cust("length(trim(target_revision)) > 0"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_seo_translation_apply_receipts_resource")
                    .table(Alias::new("seo_translation_apply_receipts"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("target_kind"))
                    .col(Alias::new("target_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("seo_translation_apply_receipts"))
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
