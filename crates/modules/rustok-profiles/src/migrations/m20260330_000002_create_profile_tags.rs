use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProfileTags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ProfileTags::ProfileUserId).uuid().not_null())
                    .col(ColumnDef::new(ProfileTags::TermId).uuid().not_null())
                    .col(ColumnDef::new(ProfileTags::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(ProfileTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ProfileTags::ProfileUserId)
                            .col(ProfileTags::TermId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_profile_tags_profile")
                            .from(ProfileTags::Table, ProfileTags::ProfileUserId)
                            .to(Profiles::Table, Profiles::UserId)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_profile_tags_term")
                            .from(ProfileTags::Table, ProfileTags::TermId)
                            .to(TaxonomyTerms::Table, TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_profile_tags_tenant_term")
                    .table(ProfileTags::Table)
                    .col(ProfileTags::TenantId)
                    .col(ProfileTags::TermId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProfileTags::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ProfileTags {
    Table,
    ProfileUserId,
    TermId,
    TenantId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Profiles {
    Table,
    UserId,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    Id,
}
