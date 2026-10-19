use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_media_releases"))
                    .col(
                        ColumnDef::new(Alias::new("image_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("media_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("context"))
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("attempts"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Alias::new("next_attempt_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("last_error_code")).string_len(96))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_image_media_coverage"))
                    .col(
                        ColumnDef::new(Alias::new("image_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_media_coverage_image")
                            .from(
                                Alias::new("product_image_media_coverage"),
                                Alias::new("image_id"),
                            )
                            .to(Alias::new("product_images"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("product_media_retention_readiness"))
                    .col(
                        ColumnDef::new(Alias::new("tenant_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_product_media_coverage_tenant")
                    .table(Alias::new("product_image_media_coverage"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("image_id"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_product_media_release_tenant")
                    .table(Alias::new("product_media_releases"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("image_id"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_product_media_release_due")
                    .table(Alias::new("product_media_releases"))
                    .col(Alias::new("next_attempt_at"))
                    .col(Alias::new("image_id"))
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            db.execute_unprepared("LOCK TABLE product_images, product_image_media_coverage, product_media_releases, product_media_retention_readiness IN ACCESS EXCLUSIVE MODE").await?;
        }
        // Rollback is permitted only after durable release work and protected
        // live relations have been drained explicitly. Never forget obligations.
        for table in ["product_media_releases", "product_image_media_coverage"] {
            if db
                .query_one_raw(Statement::from_string(
                    manager.get_database_backend(),
                    format!("SELECT 1 FROM {table} LIMIT 1"),
                ))
                .await?
                .is_some()
            {
                return Err(DbErr::Custom(
                    "Product Media retention requires an explicit drained-data retirement".into(),
                ));
            }
        }
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("product_media_retention_readiness"))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("product_image_media_coverage"))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("product_media_releases"))
                    .to_owned(),
            )
            .await
    }
}
