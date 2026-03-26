use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Profiles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Profiles::UserId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Profiles::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Profiles::Handle).string_len(64).not_null())
                    .col(
                        ColumnDef::new(Profiles::DisplayName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Profiles::AvatarMediaId).uuid())
                    .col(ColumnDef::new(Profiles::BannerMediaId).uuid())
                    .col(ColumnDef::new(Profiles::PreferredLocale).string_len(16))
                    .col(
                        ColumnDef::new(Profiles::Visibility)
                            .string_len(32)
                            .not_null()
                            .default("public"),
                    )
                    .col(
                        ColumnDef::new(Profiles::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Profiles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Profiles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_profiles_tenant_handle")
                    .table(Profiles::Table)
                    .col(Profiles::TenantId)
                    .col(Profiles::Handle)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProfileTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProfileTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProfileTranslations::ProfileUserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProfileTranslations::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProfileTranslations::DisplayName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProfileTranslations::Bio).text())
                    .col(
                        ColumnDef::new(ProfileTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ProfileTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_profile_translations_profile")
                            .from(
                                ProfileTranslations::Table,
                                ProfileTranslations::ProfileUserId,
                            )
                            .to(Profiles::Table, Profiles::UserId)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_profile_translations_profile_locale")
                    .table(ProfileTranslations::Table)
                    .col(ProfileTranslations::ProfileUserId)
                    .col(ProfileTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProfileTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Profiles::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Profiles {
    Table,
    UserId,
    TenantId,
    Handle,
    DisplayName,
    AvatarMediaId,
    BannerMediaId,
    PreferredLocale,
    Visibility,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ProfileTranslations {
    Table,
    Id,
    ProfileUserId,
    Locale,
    DisplayName,
    Bio,
    CreatedAt,
    UpdatedAt,
}
