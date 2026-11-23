use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(Cursors::Table)
                .if_not_exists()
                .col(ColumnDef::new(Cursors::WorkflowId).uuid().not_null().primary_key())
                .col(ColumnDef::new(Cursors::TenantId).uuid().not_null())
                .col(ColumnDef::new(Cursors::ScheduleRevision).string_len(64).not_null())
                .col(ColumnDef::new(Cursors::LastEnqueuedAt).timestamp_with_time_zone().not_null())
                .col(ColumnDef::new(Cursors::Generation).big_integer().not_null().default(0))
                .col(ColumnDef::new(Cursors::UpdatedAt).timestamp_with_time_zone().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_workflow_cron_cursors_workflow")
                        .from(Cursors::Table, Cursors::WorkflowId)
                        .to(Workflows::Table, Workflows::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                )
                .to_owned()
        ).await?;

        manager.create_table(
            Table::create()
                .table(Firings::Table)
                .if_not_exists()
                .col(ColumnDef::new(Firings::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Firings::WorkflowId).uuid().not_null())
                .col(ColumnDef::new(Firings::TenantId).uuid().not_null())
                .col(ColumnDef::new(Firings::ScheduleRevision).string_len(64).not_null())
                .col(ColumnDef::new(Firings::ScheduledAt).timestamp_with_time_zone().not_null())
                .col(ColumnDef::new(Firings::Status).string_len(32).not_null().default("pending"))
                .col(ColumnDef::new(Firings::AttemptToken).uuid())
                .col(ColumnDef::new(Firings::ClaimedAt).timestamp_with_time_zone())
                .col(ColumnDef::new(Firings::ExecutionId).uuid())
                .col(ColumnDef::new(Firings::Reason).text())
                .col(ColumnDef::new(Firings::CreatedAt).timestamp_with_time_zone().not_null())
                .col(ColumnDef::new(Firings::CompletedAt).timestamp_with_time_zone())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_workflow_cron_firings_workflow")
                        .from(Firings::Table, Firings::WorkflowId)
                        .to(Workflows::Table, Workflows::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                )
                .to_owned()
        ).await?;

        manager.create_index(
            Index::create()
                .name("uidx_workflow_cron_slot")
                .table(Firings::Table)
                .col(Firings::WorkflowId)
                .col(Firings::ScheduleRevision)
                .col(Firings::ScheduledAt)
                .unique()
                .to_owned()
        ).await?;
        manager.create_index(
            Index::create()
                .name("idx_workflow_cron_pending")
                .table(Firings::Table)
                .col(Firings::Status)
                .col(Firings::ScheduledAt)
                .to_owned()
        ).await?;
        manager.create_index(
            Index::create()
                .name("idx_workflow_cron_stale")
                .table(Firings::Table)
                .col(Firings::Status)
                .col(Firings::ClaimedAt)
                .to_owned()
        ).await?;
        // Immutable operator decisions are recorded in the same transaction
        // as a review-state transition, never in a best-effort log alone.
        manager.create_table(
            Table::create()
                .table(Reviews::Table)
                .if_not_exists()
                .col(ColumnDef::new(Reviews::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Reviews::FiringId).uuid().not_null())
                .col(ColumnDef::new(Reviews::TenantId).uuid().not_null())
                .col(ColumnDef::new(Reviews::ActorId).uuid().not_null())
                .col(ColumnDef::new(Reviews::Decision).string_len(32).not_null())
                .col(ColumnDef::new(Reviews::Evidence).text().not_null())
                .col(ColumnDef::new(Reviews::PreviousAttemptToken).uuid())
                .col(ColumnDef::new(Reviews::CreatedAt).timestamp_with_time_zone().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_workflow_cron_review_firing")
                        .from(Reviews::Table, Reviews::FiringId)
                        .to(Firings::Table, Firings::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                )
                .to_owned()
        ).await?;
        manager.create_index(
            Index::create()
                .name("idx_workflow_cron_reviews_tenant_firing")
                .table(Reviews::Table)
                .col(Reviews::TenantId)
                .col(Reviews::FiringId)
                .to_owned()
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Reviews::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Firings::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Cursors::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Workflows { Table, Id }

#[derive(DeriveIden)]
enum Cursors {
    #[iden = "workflow_cron_cursors"]
    Table, WorkflowId, TenantId, ScheduleRevision, LastEnqueuedAt, Generation, UpdatedAt
}

#[derive(DeriveIden)]
enum Firings {
    #[iden = "workflow_cron_firings"]
    Table, Id, WorkflowId, TenantId, ScheduleRevision, ScheduledAt, Status,
    AttemptToken, ClaimedAt, ExecutionId, Reason, CreatedAt, CompletedAt
}

#[derive(DeriveIden)]
enum Reviews {
    #[iden = "workflow_cron_reconciliation_decisions"]
    Table, Id, FiringId, TenantId, ActorId, Decision, Evidence,
    PreviousAttemptToken, CreatedAt,
}
