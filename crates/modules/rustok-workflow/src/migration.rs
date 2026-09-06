use sea_orm_migration::prelude::*;

/// Migration: create workflows, workflow_steps, workflow_executions,
/// workflow_step_executions tables.
#[derive(DeriveMigrationName)]
pub struct WorkflowsMigration;

#[async_trait::async_trait]
impl MigrationTrait for WorkflowsMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // --- workflows ---
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
                    .col(ColumnDef::new(Workflows::Name).string_len(255).not_null())
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
                        ColumnDef::new(Workflows::FailureCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Workflows::AutoDisabledAt).timestamp_with_time_zone())
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
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_workflows_tenant_name")
                            .col(Workflows::TenantId)
                            .col(Workflows::Name),
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

        // --- workflow_steps ---
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

        // --- workflow_executions ---
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
                    .name("idx_workflow_executions_tenant_workflow")
                    .table(WorkflowExecutions::Table)
                    .col(WorkflowExecutions::TenantId)
                    .col(WorkflowExecutions::WorkflowId)
                    .col(WorkflowExecutions::StartedAt)
                    .to_owned(),
            )
            .await?;

        // --- workflow_step_executions ---
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
                            .name("fk_wse_execution_id")
                            .from(
                                WorkflowStepExecutions::Table,
                                WorkflowStepExecutions::ExecutionId,
                            )
                            .to(WorkflowExecutions::Table, WorkflowExecutions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_wse_execution_id")
                    .table(WorkflowStepExecutions::Table)
                    .col(WorkflowStepExecutions::ExecutionId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(WorkflowStepExecutions::Table)
                    .to_owned(),
            )
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
    FailureCount,
    AutoDisabledAt,
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

/// Migration: add webhook_slug/webhook_secret to workflows + create workflow_versions.
#[derive(DeriveMigrationName)]
pub struct WorkflowPhase4Migration;

#[async_trait::async_trait]
impl MigrationTrait for WorkflowPhase4Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add webhook_slug + webhook_secret to workflows
        manager
            .alter_table(
                Table::alter()
                    .table(WorkflowsV2::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(WorkflowsV2::WebhookSlug).string_len(128),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(WorkflowsV2::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(WorkflowsV2::WebhookSecret).string_len(128),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_workflows_tenant_webhook_slug")
                    .table(WorkflowsV2::Table)
                    .col(WorkflowsV2::TenantId)
                    .col(WorkflowsV2::WebhookSlug)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Create workflow_versions table
        manager
            .create_table(
                Table::create()
                    .table(WorkflowVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WorkflowVersions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WorkflowVersions::WorkflowId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowVersions::Version)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowVersions::Snapshot)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WorkflowVersions::CreatedBy).uuid())
                    .col(
                        ColumnDef::new(WorkflowVersions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workflow_versions_workflow_id")
                            .from(WorkflowVersions::Table, WorkflowVersions::WorkflowId)
                            .to(WorkflowsV2::Table, Workflows::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_workflow_versions_workflow_version")
                            .col(WorkflowVersions::WorkflowId)
                            .col(WorkflowVersions::Version),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workflow_versions_workflow_id")
                    .table(WorkflowVersions::Table)
                    .col(WorkflowVersions::WorkflowId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WorkflowVersions::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(WorkflowsV2::Table)
                    .drop_column(WorkflowsV2::WebhookSlug)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(WorkflowsV2::Table)
                    .drop_column(WorkflowsV2::WebhookSecret)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum WorkflowsV2 {
    #[sea_orm(iden = "workflows")]
    Table,
    TenantId,
    WebhookSlug,
    WebhookSecret,
}

#[derive(DeriveIden)]
enum WorkflowVersions {
    Table,
    Id,
    WorkflowId,
    Version,
    Snapshot,
    CreatedBy,
    CreatedAt,
}
