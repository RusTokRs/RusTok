use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;

use super::{
    ensure_ai_overview_read, ensure_ai_provider_read, ensure_ai_session_read,
    ensure_ai_task_profile_read,
    types::{
        AiChatSessionDetailGql, AiChatSessionSummaryGql, AiProviderProfileGql, AiRecentRunGql,
        AiRunStreamEventGql, AiRuntimeMetricsGql, AiTaskProfileGql, AiToolProfileGql,
        AiToolTraceGql,
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
    async fn ai_runtime_metrics(&self, ctx: &Context<'_>) -> Result<AiRuntimeMetricsGql> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_overview_read(auth)?;
        Ok(rustok_ai::AiManagementService::metrics_snapshot().into())
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
            rustok_ai::AiManagementService::recent_stream_events(session_id, limit)
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
            rustok_ai::AiManagementService::list_recent_runs(db, auth.tenant_id, limit)
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

        let items = rustok_ai::AiManagementService::list_provider_profiles(db, auth.tenant_id)
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
        let item = rustok_ai::AiManagementService::get_provider_profile(db, auth.tenant_id, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(item.map(Into::into))
    }

    async fn ai_tool_profiles(&self, ctx: &Context<'_>) -> Result<Vec<AiToolProfileGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let items = rustok_ai::AiManagementService::list_tool_profiles(db, auth.tenant_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items.into_iter().map(Into::into).collect())
    }

    async fn ai_task_profiles(&self, ctx: &Context<'_>) -> Result<Vec<AiTaskProfileGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_task_profile_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let items = rustok_ai::AiManagementService::list_task_profiles(db, auth.tenant_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items.into_iter().map(Into::into).collect())
    }

    async fn ai_chat_sessions(&self, ctx: &Context<'_>) -> Result<Vec<AiChatSessionSummaryGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_ai_session_read(auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let items = rustok_ai::AiManagementService::list_chat_sessions(db, auth.tenant_id)
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
        let item = rustok_ai::AiManagementService::chat_session_detail(db, auth.tenant_id, id)
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
        let items = rustok_ai::AiManagementService::list_tool_traces(
            db,
            auth.tenant_id,
            session_id,
            run_id,
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        Ok(items
            .into_iter()
            .map(super::types::AiToolTraceGql::from_record)
            .collect())
    }
}
