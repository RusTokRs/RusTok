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
                    .table(RegionTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegionTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslations::RegionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionTranslations::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_region_translations_region")
                            .from(RegionTranslations::Table, RegionTranslations::RegionId)
                            .to(Regions::Table, Regions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_region_translations_unique")
                    .table(RegionTranslations::Table)
                    .col(RegionTranslations::RegionId)
                    .col(RegionTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        let backend = manager.get_connection().get_database_backend();
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                backend,
                "SELECT id, name FROM regions".to_string(),
            ))
            .await?;

        for row in rows {
            let region_id: Uuid = row.try_get("", "id")?;
            let name: String = row.try_get("", "name")?;
            manager
                .get_connection()
                .execute(Statement::from_sql_and_values(
                    backend,
                    "INSERT INTO region_translations (id, region_id, locale, name)
                     VALUES (?, ?, ?, ?)"
                        .to_string(),
                    vec![
                        Uuid::new_v4().into(),
                        region_id.into(),
                        "en".into(),
                        name.into(),
                    ],
                ))
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Regions::Table)
                    .drop_column(Regions::Name)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Regions::Table)
                    .add_column(
                        ColumnDef::new(Regions::Name)
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
                "UPDATE regions
                 SET name = COALESCE((
                        SELECT name
                        FROM region_translations
                        WHERE region_id = regions.id
                        ORDER BY locale
                        LIMIT 1
                    ), '')"
                    .to_string(),
            ))
            .await?;

        manager
            .drop_table(
                Table::drop().table(RegionTranslations::Table).to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Regions {
    Table,
    Id,
    Name,
}

#[derive(DeriveIden)]
enum RegionTranslations {
    Table,
    Id,
    RegionId,
    Locale,
    Name,
}
