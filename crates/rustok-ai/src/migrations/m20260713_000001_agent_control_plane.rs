use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let json = match manager.get_database_backend() {
            DbBackend::Postgres => "JSONB",
            DbBackend::Sqlite => "JSON",
            other => {
                return Err(DbErr::Migration(format!(
                    "AI agent control-plane migration does not support database backend {other:?}"
                )));
            }
        };
        for statement in [
            format!(
                "CREATE TABLE ai_agent_principals (\
                    id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL,\
                    descriptor_owner TEXT NOT NULL, descriptor_slug TEXT NOT NULL,\
                    role_slugs {json} NOT NULL, permission_slugs {json} NOT NULL,\
                    is_active BOOLEAN NOT NULL, metadata {json} NOT NULL,\
                    created_by UUID NULL, updated_by UUID NULL,\
                    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, slug))"
            ),
            "CREATE INDEX ai_agent_principals_tenant_descriptor_idx ON ai_agent_principals (tenant_id, descriptor_owner, descriptor_slug)".to_string(),
            format!(
                "CREATE TABLE ai_agent_model_assignments (\
                    id UUID PRIMARY KEY, tenant_id UUID NOT NULL, agent_principal_id UUID NOT NULL,\
                    provider_profile_id UUID NOT NULL, model_override TEXT NULL,\
                    execution_mode TEXT NOT NULL, is_active BOOLEAN NOT NULL, metadata {json} NOT NULL,\
                    created_by UUID NULL, updated_by UUID NULL,\
                    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, agent_principal_id, provider_profile_id))"
            ),
            "CREATE INDEX ai_agent_model_assignments_principal_idx ON ai_agent_model_assignments (tenant_id, agent_principal_id, is_active)".to_string(),
            format!(
                "CREATE TABLE ai_agent_workflow_runs (\
                    id UUID PRIMARY KEY, tenant_id UUID NOT NULL, workflow_owner TEXT NOT NULL,\
                    workflow_slug TEXT NOT NULL, initiator_id UUID NOT NULL, status TEXT NOT NULL,\
                    input_payload {json} NOT NULL, output_payload {json} NULL, metadata {json} NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL, started_at TIMESTAMPTZ NULL,\
                    completed_at TIMESTAMPTZ NULL, updated_at TIMESTAMPTZ NOT NULL)"
            ),
            "CREATE INDEX ai_agent_workflow_runs_tenant_status_idx ON ai_agent_workflow_runs (tenant_id, status, created_at)".to_string(),
            format!(
                "CREATE TABLE ai_agent_workflow_stages (\
                    id UUID PRIMARY KEY, tenant_id UUID NOT NULL, workflow_run_id UUID NOT NULL,\
                    stage_id TEXT NOT NULL, agent_principal_id UUID NOT NULL,\
                    model_assignment_id UUID NULL, run_id UUID NULL, status TEXT NOT NULL,\
                    requires_approval BOOLEAN NOT NULL, input_payload {json} NOT NULL,\
                    output_payload {json} NULL, error_message TEXT NULL, metadata {json} NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL, started_at TIMESTAMPTZ NULL,\
                    completed_at TIMESTAMPTZ NULL, updated_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, workflow_run_id, stage_id))"
            ),
            "CREATE INDEX ai_agent_workflow_stages_ready_idx ON ai_agent_workflow_stages (tenant_id, status, created_at)".to_string(),
        ] {
            connection.execute_unprepared(&statement).await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "AI agent control-plane migration is intentionally irreversible".to_string(),
        ))
    }
}
