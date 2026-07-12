use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegistryGovernanceEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::Id)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::Slug)
                            .string_len(96)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::RequestId)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::ReleaseId)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::EventType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::Actor)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::Publisher)
                            .string_len(128)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::Details)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryGovernanceEvents::CreatedAt)
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
                    .name("idx_registry_governance_events_slug_created")
                    .table(RegistryGovernanceEvents::Table)
                    .col(RegistryGovernanceEvents::Slug)
                    .col(RegistryGovernanceEvents::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RegistryGovernanceEvents::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegistryGovernanceEvents {
    Table,
    Id,
    Slug,
    RequestId,
    ReleaseId,
    EventType,
    Actor,
    Publisher,
    Details,
    CreatedAt,
}
