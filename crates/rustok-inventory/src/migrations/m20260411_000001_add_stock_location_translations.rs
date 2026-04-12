use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StockLocationTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StockLocationTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(StockLocationTranslations::StockLocationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StockLocationTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StockLocationTranslations::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_stock_location_translations_location")
                            .from(
                                StockLocationTranslations::Table,
                                StockLocationTranslations::StockLocationId,
                            )
                            .to(StockLocations::Table, StockLocations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_stock_location_translations_unique")
                    .table(StockLocationTranslations::Table)
                    .col(StockLocationTranslations::StockLocationId)
                    .col(StockLocationTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        let backend = manager.get_connection().get_database_backend();
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                backend,
                "SELECT id, name FROM stock_locations".to_string(),
            ))
            .await?;

        for row in rows {
            let location_id: Uuid = row.try_get("", "id")?;
            let name: String = row.try_get("", "name")?;
            manager
                .get_connection()
                .execute(Statement::from_sql_and_values(
                    backend,
                    "INSERT INTO stock_location_translations (id, stock_location_id, locale, name)
                     VALUES (?, ?, ?, ?)"
                        .to_string(),
                    vec![
                        Uuid::new_v4().into(),
                        location_id.into(),
                        "en".into(),
                        name.into(),
                    ],
                ))
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(StockLocations::Table)
                    .drop_column(StockLocations::Name)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(StockLocations::Table)
                    .add_column(
                        ColumnDef::new(StockLocations::Name)
                            .string_len(100)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        let backend = manager.get_connection().get_database_backend();
        manager
            .get_connection()
            .execute(Statement::from_string(
                backend,
                "UPDATE stock_locations
                 SET name = COALESCE((
                        SELECT name
                        FROM stock_location_translations
                        WHERE stock_location_id = stock_locations.id
                        ORDER BY locale
                        LIMIT 1
                    ), '')"
                .to_string(),
            ))
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(StockLocationTranslations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum StockLocations {
    Table,
    Id,
    Name,
}

#[derive(DeriveIden)]
enum StockLocationTranslations {
    Table,
    Id,
    StockLocationId,
    Locale,
    Name,
}
