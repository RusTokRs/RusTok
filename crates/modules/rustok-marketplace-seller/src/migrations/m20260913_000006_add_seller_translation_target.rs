use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder, Statement,
};
use sea_orm_migration::prelude::*;

use crate::entities::{seller, seller_translation};
use crate::translation::resource_revision;
use crate::translation_changes::translation_lifecycle_for_status;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut table = Table::create();
        table
            .table(MarketplaceSellerTranslationChangeJournal::Table)
            .if_not_exists();
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            table.col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::ChangeSeq)
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        } else {
            table.col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::ChangeSeq)
                    .big_integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        }
        table
            .col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::OperationId).uuid(),
            )
            .col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::TenantId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::SellerId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::ResourceRevision)
                    .string_len(96)
                    .not_null(),
            )
            .col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::Lifecycle)
                    .string_len(16)
                    .not_null(),
            )
            .col(
                ColumnDef::new(MarketplaceSellerTranslationChangeJournal::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null()
                    .default(Expr::current_timestamp()),
            );
        manager.create_table(table.to_owned()).await?;

        for index in [
            Index::create()
                .name("ux_marketplace_seller_translation_change_operation")
                .table(MarketplaceSellerTranslationChangeJournal::Table)
                .col(MarketplaceSellerTranslationChangeJournal::OperationId)
                .col(MarketplaceSellerTranslationChangeJournal::SellerId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_marketplace_seller_translation_change_tenant_seq")
                .table(MarketplaceSellerTranslationChangeJournal::Table)
                .col(MarketplaceSellerTranslationChangeJournal::TenantId)
                .col(MarketplaceSellerTranslationChangeJournal::ChangeSeq)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_seller_translation_change_seller_seq")
                .table(MarketplaceSellerTranslationChangeJournal::Table)
                .col(MarketplaceSellerTranslationChangeJournal::TenantId)
                .col(MarketplaceSellerTranslationChangeJournal::SellerId)
                .col(MarketplaceSellerTranslationChangeJournal::ChangeSeq)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        backfill_existing_resources(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplaceSellerTranslationChangeJournal::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn backfill_existing_resources(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let sellers = seller::Entity::find()
        .order_by_asc(seller::Column::TenantId)
        .order_by_asc(seller::Column::Id)
        .all(connection)
        .await?;
    let backend = connection.get_database_backend();

    for seller in sellers {
        let translations = seller_translation::Entity::find()
            .filter(seller_translation::Column::TenantId.eq(seller.tenant_id))
            .filter(seller_translation::Column::SellerId.eq(seller.id))
            .order_by_asc(seller_translation::Column::Locale)
            .all(connection)
            .await?;
        if translations.is_empty() {
            continue;
        }
        let lifecycle = translation_lifecycle_for_status(&seller.status)
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        let revision = resource_revision(&seller, &translations);
        let sql = match backend {
            DatabaseBackend::Postgres => {
                r#"
INSERT INTO marketplace_seller_translation_change_journal (
    tenant_id, seller_id, resource_revision, lifecycle
) VALUES ($1, $2, $3, $4)
"#
            }
            _ => {
                r#"
INSERT INTO marketplace_seller_translation_change_journal (
    tenant_id, seller_id, resource_revision, lifecycle
) VALUES (?, ?, ?, ?)
"#
            }
        };
        connection
            .execute_raw(Statement::from_sql_and_values(
                backend,
                sql,
                vec![
                    seller.tenant_id.into(),
                    seller.id.into(),
                    revision.into(),
                    lifecycle.as_str().into(),
                ],
            ))
            .await?;
    }
    Ok(())
}

#[derive(DeriveIden)]
enum MarketplaceSellerTranslationChangeJournal {
    Table,
    ChangeSeq,
    OperationId,
    TenantId,
    SellerId,
    ResourceRevision,
    Lifecycle,
    CreatedAt,
}
