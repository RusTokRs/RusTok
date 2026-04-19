use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ShippingProfileTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ShippingProfileTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ShippingProfileTranslations::ShippingProfileId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ShippingProfileTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ShippingProfileTranslations::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ShippingProfileTranslations::Description).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shipping_profile_translations_profile")
                            .from(
                                ShippingProfileTranslations::Table,
                                ShippingProfileTranslations::ShippingProfileId,
                            )
                            .to(ShippingProfiles::Table, ShippingProfiles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shipping_profile_translations_unique")
                    .table(ShippingProfileTranslations::Table)
                    .col(ShippingProfileTranslations::ShippingProfileId)
                    .col(ShippingProfileTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        let backend = manager.get_connection().get_database_backend();
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                backend,
                "SELECT id, name, description FROM shipping_profiles".to_string(),
            ))
            .await?;

        for row in rows {
            let shipping_profile_id: Uuid = row.try_get("", "id")?;
            let name: String = row.try_get("", "name")?;
            let description: Option<String> = row.try_get("", "description")?;
            manager
                .get_connection()
                .execute(Statement::from_sql_and_values(
                    backend,
                    "INSERT INTO shipping_profile_translations (id, shipping_profile_id, locale, name, description)
                     VALUES (?, ?, ?, ?, ?)"
                        .to_string(),
                    vec![
                        Uuid::new_v4().into(),
                        shipping_profile_id.into(),
                        "en".into(),
                        name.into(),
                        description.into(),
                    ],
                ))
                .await?;
        }

        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(ShippingProfiles::Table)
                        .drop_column(ShippingProfiles::Name)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(ShippingProfiles::Table)
                        .drop_column(ShippingProfiles::Description)
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(ShippingProfiles::Table)
                        .drop_column(ShippingProfiles::Name)
                        .drop_column(ShippingProfiles::Description)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(ShippingProfiles::Table)
                        .add_column(
                            ColumnDef::new(ShippingProfiles::Name)
                                .string_len(255)
                                .not_null()
                                .default(""),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(ShippingProfiles::Table)
                        .add_column(ColumnDef::new(ShippingProfiles::Description).text())
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(ShippingProfiles::Table)
                        .add_column(
                            ColumnDef::new(ShippingProfiles::Name)
                                .string_len(255)
                                .not_null()
                                .default(""),
                        )
                        .add_column(ColumnDef::new(ShippingProfiles::Description).text())
                        .to_owned(),
                )
                .await?;
        }

        let backend = manager.get_connection().get_database_backend();
        manager
            .get_connection()
            .execute(Statement::from_string(
                backend,
                "UPDATE shipping_profiles
                 SET name = COALESCE((
                        SELECT name
                        FROM shipping_profile_translations
                        WHERE shipping_profile_id = shipping_profiles.id
                        ORDER BY locale
                        LIMIT 1
                    ), ''),
                    description = (
                        SELECT description
                        FROM shipping_profile_translations
                        WHERE shipping_profile_id = shipping_profiles.id
                        ORDER BY locale
                        LIMIT 1
                    )"
                .to_string(),
            ))
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(ShippingProfileTranslations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ShippingProfiles {
    Table,
    Id,
    Name,
    Description,
}

#[derive(DeriveIden)]
enum ShippingProfileTranslations {
    Table,
    Id,
    ShippingProfileId,
    Locale,
    Name,
    Description,
}
