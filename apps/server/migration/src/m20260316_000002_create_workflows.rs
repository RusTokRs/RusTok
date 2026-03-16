use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // workflows table
        manager
            .create_table(
                Table::create()
                    .table(Workflows::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Workflows::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Workflows::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(Workflows::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Workflows::Description).text())
                    .col(
                        ColumnDef::new(Workflows::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(Workflows::TriggerConfig)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Workflows::CreatedBy).uuid())
                    .col(
                        ColumnDef::new(Workflows::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Workflows::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workflows_tenant_status")
                    .table(Workflows::Table)
                    .col(Workflows::TenantId)
                    .col(Workflows::Status)
                    .to_owned(),
            )
            .await?;

        // workflow_steps table
        manager
            .create_table(
                Table::create()
                    .table(WorkflowSteps::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WorkflowSteps::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(WorkflowSteps::WorkflowId).uuid().not_null())
                    .col(ColumnDef::new(WorkflowSteps::Position).integer().not_null())
                    .col(
                        ColumnDef::new(WorkflowSteps::StepType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowSteps::Config)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowSteps::OnError)
                            .string_len(32)
                            .not_null()
                            .default("stop"),
                    )
                    .col(ColumnDef::new(WorkflowSteps::TimeoutMs).big_integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workflow_steps_workflow_id")
                            .from(WorkflowSteps::Table, WorkflowSteps::WorkflowId)
                            .to(Workflows::Table, Workflows::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workflow_steps_workflow_position")
                    .table(WorkflowSteps::Table)
                    .col(WorkflowSteps::WorkflowId)
                    .col(WorkflowSteps::Position)
                    .to_owned(),
            )
            .await?;

        // workflow_executions table
        manager
            .create_table(
                Table::create()
                    .table(WorkflowExecutions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WorkflowExecutions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WorkflowExecutions::WorkflowId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowExecutions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WorkflowExecutions::TriggerEventId).uuid())
                    .col(
                        ColumnDef::new(WorkflowExecutions::Status)
                            .string_len(32)
                            .not_null()
                            .default("running"),
                    )
                    .col(
                        ColumnDef::new(WorkflowExecutions::Context)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WorkflowExecutions::Error).text())
                    .col(
                        ColumnDef::new(WorkflowExecutions::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WorkflowExecutions::CompletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workflow_executions_workflow_id")
                            .from(WorkflowExecutions::Table, WorkflowExecutions::WorkflowId)
                            .to(Workflows::Table, Workflows::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workflow_executions_tenant_status")
                    .table(WorkflowExecutions::Table)
                    .col(WorkflowExecutions::TenantId)
                    .col(WorkflowExecutions::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workflow_executions_workflow_id")
                    .table(WorkflowExecutions::Table)
                    .col(WorkflowExecutions::WorkflowId)
                    .to_owned(),
            )
            .await?;

        // workflow_step_executions table
        manager
            .create_table(
                Table::create()
                    .table(WorkflowStepExecutions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::ExecutionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::StepId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::Input)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::Output)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WorkflowStepExecutions::Error).text())
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowStepExecutions::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workflow_step_executions_execution_id")
                            .from(
                                WorkflowStepExecutions::Table,
                                WorkflowStepExecutions::ExecutionId,
                            )
                            .to(WorkflowExecutions::Table, WorkflowExecutions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workflow_step_executions_step_id")
                            .from(WorkflowStepExecutions::Table, WorkflowStepExecutions::StepId)
                            .to(WorkflowSteps::Table, WorkflowSteps::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workflow_step_executions_execution_id")
                    .table(WorkflowStepExecutions::Table)
                    .col(WorkflowStepExecutions::ExecutionId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WorkflowStepExecutions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkflowExecutions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WorkflowSteps::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Workflows::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Workflows {
    Table,
    Id,
    TenantId,
    Name,
    Description,
    Status,
    TriggerConfig,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WorkflowSteps {
    Table,
    Id,
    WorkflowId,
    Position,
    StepType,
    Config,
    OnError,
    TimeoutMs,
}

#[derive(DeriveIden)]
enum WorkflowExecutions {
    Table,
    Id,
    WorkflowId,
    TenantId,
    TriggerEventId,
    Status,
    Context,
    Error,
    StartedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum WorkflowStepExecutions {
    Table,
    Id,
    ExecutionId,
    StepId,
    Status,
    Input,
    Output,
    Error,
    StartedAt,
    CompletedAt,
}
