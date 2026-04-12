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
                    .table(PriceListTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PriceListTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PriceListTranslations::PriceListId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PriceListTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PriceListTranslations::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PriceListTranslations::Description).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_price_list_translations_list")
                            .from(
                                PriceListTranslations::Table,
                                PriceListTranslations::PriceListId,
                            )
                            .to(PriceLists::Table, PriceLists::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_price_list_translations_unique")
                    .table(PriceListTranslations::Table)
                    .col(PriceListTranslations::PriceListId)
                    .col(PriceListTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        let backend = manager.get_connection().get_database_backend();
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                backend,
                "SELECT id, name, description FROM price_lists".to_string(),
            ))
            .await?;

        for row in rows {
            let price_list_id: Uuid = row.try_get("", "id")?;
            let name: String = row.try_get("", "name")?;
            let description: Option<String> = row.try_get("", "description")?;
            manager
                .get_connection()
                .execute(Statement::from_sql_and_values(
                    backend,
                    "INSERT INTO price_list_translations (id, price_list_id, locale, name, description)
                     VALUES (?, ?, ?, ?, ?)"
                        .to_string(),
                    vec![
                        Uuid::new_v4().into(),
                        price_list_id.into(),
                        "en".into(),
                        name.into(),
                        description.into(),
                    ],
                ))
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(PriceLists::Table)
                    .drop_column(PriceLists::Name)
                    .drop_column(PriceLists::Description)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PriceLists::Table)
                    .add_column(
                        ColumnDef::new(PriceLists::Name)
                            .string_len(100)
                            .not_null()
                            .default(""),
                    )
                    .add_column(ColumnDef::new(PriceLists::Description).text())
                    .to_owned(),
            )
            .await?;

        let backend = manager.get_connection().get_database_backend();
        manager
            .get_connection()
            .execute(Statement::from_string(
                backend,
                "UPDATE price_lists
                 SET name = COALESCE((
                        SELECT name
                        FROM price_list_translations
                        WHERE price_list_id = price_lists.id
                        ORDER BY locale
                        LIMIT 1
                    ), ''),
                    description = (
                        SELECT description
                        FROM price_list_translations
                        WHERE price_list_id = price_lists.id
                        ORDER BY locale
                        LIMIT 1
                    )"
                .to_string(),
            ))
            .await?;

        manager
            .drop_table(Table::drop().table(PriceListTranslations::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PriceLists {
    Table,
    Id,
    Name,
    Description,
}

#[derive(DeriveIden)]
enum PriceListTranslations {
    Table,
    Id,
    PriceListId,
    Locale,
    Name,
    Description,
}
