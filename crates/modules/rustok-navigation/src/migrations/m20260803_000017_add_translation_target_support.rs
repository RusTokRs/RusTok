use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Menus::Table)
                    .add_column(
                        ColumnDef::new(Menus::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(MenuTranslations::Table)
                    .add_column(
                        ColumnDef::new(MenuTranslations::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(NavigationTranslationChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::ResourceKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::ResourceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::ResourceRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::TargetRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::Lifecycle)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NavigationTranslationChanges::CreatedAt)
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
                    .if_not_exists()
                    .name("idx_navigation_translation_changes_tenant_id")
                    .table(NavigationTranslationChanges::Table)
                    .col(NavigationTranslationChanges::TenantId)
                    .col(NavigationTranslationChanges::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(NavigationTranslationChanges::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(MenuTranslations::Table)
                    .drop_column(MenuTranslations::Revision)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Menus::Table)
                    .drop_column(Menus::Revision)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Menus {
    #[iden = "menus"]
    Table,
    Revision,
}

#[derive(Iden)]
enum MenuTranslations {
    #[iden = "menu_translations"]
    Table,
    Revision,
}

#[derive(Iden)]
enum NavigationTranslationChanges {
    #[iden = "navigation_translation_changes"]
    Table,
    Id,
    TenantId,
    ResourceKind,
    ResourceId,
    Locale,
    ResourceRevision,
    TargetRevision,
    Operation,
    Lifecycle,
    CreatedAt,
}
