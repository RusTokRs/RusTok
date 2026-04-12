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
                    .table(ShippingOptionTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ShippingOptionTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ShippingOptionTranslations::ShippingOptionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ShippingOptionTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ShippingOptionTranslations::Name)
                            .string_len(120)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shipping_option_translations_option")
                            .from(
                                ShippingOptionTranslations::Table,
                                ShippingOptionTranslations::ShippingOptionId,
                            )
                            .to(ShippingOptions::Table, ShippingOptions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shipping_option_translations_unique")
                    .table(ShippingOptionTranslations::Table)
                    .col(ShippingOptionTranslations::ShippingOptionId)
                    .col(ShippingOptionTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        let backend = manager.get_connection().get_database_backend();
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                backend,
                "SELECT id, name FROM shipping_options".to_string(),
            ))
            .await?;

        for row in rows {
            let shipping_option_id: Uuid = row.try_get("", "id")?;
            let name: String = row.try_get("", "name")?;
            manager
                .get_connection()
                .execute(Statement::from_sql_and_values(
                    backend,
                    "INSERT INTO shipping_option_translations (id, shipping_option_id, locale, name)
                     VALUES (?, ?, ?, ?)"
                        .to_string(),
                    vec![
                        Uuid::new_v4().into(),
                        shipping_option_id.into(),
                        "en".into(),
                        name.into(),
                    ],
                ))
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ShippingOptions::Table)
                    .drop_column(ShippingOptions::Name)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ShippingOptions::Table)
                    .add_column(
                        ColumnDef::new(ShippingOptions::Name)
                            .string_len(120)
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
                "UPDATE shipping_options
                 SET name = COALESCE((
                        SELECT name
                        FROM shipping_option_translations
                        WHERE shipping_option_id = shipping_options.id
                        ORDER BY locale
                        LIMIT 1
                    ), '')"
                .to_string(),
            ))
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(ShippingOptionTranslations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ShippingOptions {
    Table,
    Id,
    Name,
}

#[derive(DeriveIden)]
enum ShippingOptionTranslations {
    Table,
    Id,
    ShippingOptionId,
    Locale,
    Name,
}
