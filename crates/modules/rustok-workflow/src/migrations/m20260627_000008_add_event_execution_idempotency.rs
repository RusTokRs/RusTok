use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("uidx_workflow_executions_trigger_event")
                    .table(WorkflowExecutions::Table)
                    .col(WorkflowExecutions::WorkflowId)
                    .col(WorkflowExecutions::TriggerEventId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uidx_workflow_executions_trigger_event")
                    .table(WorkflowExecutions::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum WorkflowExecutions {
    Table,
    WorkflowId,
    TriggerEventId,
}
