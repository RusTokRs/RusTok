use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

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
                    .table(Alias::new("product_brands"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("product_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("brand_id")).uuid().not_null())
                    .col(
                        ColumnDef::new(Alias::new("is_primary"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Alias::new("role"))
                            .string_len(32)
                            .not_null()
                            .default("manufacturer"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("position"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_brands_product")
                            .from(Alias::new("product_brands"), Alias::new("product_id"))
                            .to(Alias::new("products"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_product_brands")
                    .table(Alias::new("product_brands"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("product_id"))
                    .col(Alias::new("brand_id"))
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_product_brands_lookup")
                    .table(Alias::new("product_brands"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("brand_id"))
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
CREATE UNIQUE INDEX IF NOT EXISTS uq_product_brands_primary
    ON product_brands (tenant_id, product_id)
    WHERE is_primary;
"#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
CREATE UNIQUE INDEX IF NOT EXISTS uq_product_brands_primary
    ON product_brands (tenant_id, product_id)
    WHERE is_primary = 1;
"#,
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared("DROP INDEX IF EXISTS uq_product_brands_primary;")
                    .await?;
            }
            _ => {}
        }

        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("product_brands"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
