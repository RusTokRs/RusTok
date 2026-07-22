use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_api::AuthContext;
use rustok_api::graphql::GraphQLError;

use super::{
    ensure_ai_overview_read, ensure_ai_provider_read, ensure_ai_session_read,
    ensure_ai_task_profile_read,
    types::{
        AiAgentDescriptorGql, AiAgentModelAssignmentGql, AiAgentPrincipalGql, AiAgentWorkflowGql,
        AiChatSessionDetailGql, AiChatSessionSummaryGql, AiProviderCatalogEntryGql,
        AiProviderProfileGql, AiProviderTargetGql, AiRecentRunGql, AiRunStreamEventGql,
        AiRuntimeMetricsGql, AiTaskProfileGql, AiTenantRbacPermissionGql, AiTenantRbacRoleGql,
        AiToolProfileGql, AiToolTraceGql,
    },
};

#[derive(Default)]
pub struct AiQuery;

fn require_auth_context<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    ctx.data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())
}

#[Object]
impl AiQuery {
    async fn ai_agent_catalog(&self, ctx: &Context<'_>) -> Result<Vec<AiAgentDescriptorGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        Ok(crate::agent_catalog()
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .descriptors()
            .iter()
            .map(Into::into)
            .collect())
    }

    async fn ai_agent_workflows(&self, ctx: &Context<'_>) -> Result<Vec<AiAgentWorkflowGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        Ok(crate::agent_catalog()
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .workflows()
            .iter()
            .map(Into::into)
            .collect())
    }

    async fn ai_agent_principals(&self, ctx: &Context<'_>) -> Result<Vec<AiAgentPrincipalGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        Ok(
            crate::AiManagementService::list_agent_principals(db, auth.tenant_id)
                .await
                .map_err(|error| async_graphql::Error::new(error.to_string()))?
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }

    async fn ai_agent_model_assignments(
        &self,
        ctx: &Context<'_>,
        agent_principal_id: Option<Uuid>,
    ) -> Result<Vec<AiAgentModelAssignmentGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let assignments = match agent_principal_id {
            Some(agent_principal_id) => {
                crate::AiManagementService::list_agent_model_assignments(
                    db,
                    auth.tenant_id,
                    agent_principal_id,
                )
                .await
            }
            None => {
                crate::AiManagementService::list_tenant_agent_model_assignments(db, auth.tenant_id)
                    .await
            }
        };
        Ok(assignments
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn ai_tenant_rbac_roles(&self, ctx: &Context<'_>) -> Result<Vec<AiTenantRbacRoleGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        let tenant_rbac_catalog = ctx
            .data::<crate::AiGraphqlRuntimeData>()?
            .tenant_rbac_catalog()
            .ok_or_else(|| async_graphql::Error::new("tenant RBAC catalog is unavailable"))?;
        Ok(tenant_rbac_catalog
            .0
            .roles(auth.tenant_id)
            .into_iter()
            .map(|role| AiTenantRbacRoleGql {
                slug: role.slug,
                display_name: role.display_name,
                permission_slugs: role.permission_slugs,
            })
            .collect())
    }

    async fn ai_tenant_rbac_permissions(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<AiTenantRbacPermissionGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        let tenant_rbac_catalog = ctx
            .data::<crate::AiGraphqlRuntimeData>()?
            .tenant_rbac_catalog()
            .ok_or_else(|| async_graphql::Error::new("tenant RBAC catalog is unavailable"))?;
        Ok(tenant_rbac_catalog
            .0
            .permissions(auth.tenant_id)
            .into_iter()
            .map(|permission| AiTenantRbacPermissionGql {
                slug: permission.slug,
                display_name: permission.display_name,
            })
            .collect())
    }

    async fn ai_provider_catalog(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<AiProviderCatalogEntryGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_read(auth)?;
        Ok(crate::provider_catalog().map(Into::into).collect())
    }

    async fn ai_provider_targets(&self, ctx: &Context<'_>) -> Result<Vec<AiProviderTargetGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_read(auth)?;
        let runtime = ctx.data::<crate::AiGraphqlRuntimeData>()?.runtime();
        Ok(runtime
            .provider_targets()
            .entries()
            .map(Into::into)
            .collect())
    }

    async fn ai_runtime_metrics(&self, ctx: &Context<'_>) -> Result<AiRuntimeMetricsGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        Ok(crate::AiManagementService::metrics_snapshot().into())
    }

    async fn ai_recent_run_stream_events(
        &self,
        ctx: &Context<'_>,
        session_id: Option<Uuid>,
        limit: Option<i32>,
    ) -> Result<Vec<AiRunStreamEventGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        let limit = limit.unwrap_or(20).max(1) as usize;
        Ok(
            crate::AiManagementService::recent_stream_events(session_id, limit)
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }

    async fn ai_recent_runs(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<AiRecentRunGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let limit = limit.unwrap_or(20).max(1) as usize;
        Ok(
            crate::AiManagementService::list_recent_runs(db, auth.tenant_id, limit)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }

    async fn ai_provider_profiles(&self, ctx: &Context<'_>) -> Result<Vec<AiProviderProfileGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;

        let items = crate::AiManagementService::list_provider_profiles(db, auth.tenant_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items.into_iter().map(Into::into).collect())
    }

    async fn ai_provider_profile(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<Option<AiProviderProfileGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_provider_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let item = crate::AiManagementService::get_provider_profile(db, auth.tenant_id, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.map(Into::into))
    }

    async fn ai_tool_profiles(&self, ctx: &Context<'_>) -> Result<Vec<AiToolProfileGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let items = crate::AiManagementService::list_tool_profiles(db, auth.tenant_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items.into_iter().map(Into::into).collect())
    }

    async fn ai_task_profiles(&self, ctx: &Context<'_>) -> Result<Vec<AiTaskProfileGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let items = crate::AiManagementService::list_task_profiles(db, auth.tenant_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items.into_iter().map(Into::into).collect())
    }

    async fn ai_chat_sessions(&self, ctx: &Context<'_>) -> Result<Vec<AiChatSessionSummaryGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_session_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let items = crate::AiManagementService::list_chat_sessions(db, auth.tenant_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items.into_iter().map(Into::into).collect())
    }

    async fn ai_chat_session(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<Option<AiChatSessionDetailGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_session_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let item = crate::AiManagementService::chat_session_detail(db, auth.tenant_id, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        item.map(TryInto::try_into).transpose()
    }

    async fn ai_tool_trace(
        &self,
        ctx: &Context<'_>,
        session_id: Option<Uuid>,
        run_id: Option<Uuid>,
    ) -> Result<Vec<AiToolTraceGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let items =
            crate::AiManagementService::list_tool_traces(db, auth.tenant_id, session_id, run_id)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items
            .into_iter()
            .map(super::types::AiToolTraceGql::from_record)
            .collect())
    }
}
