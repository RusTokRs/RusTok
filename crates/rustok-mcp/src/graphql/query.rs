use async_graphql::{Context, Object, Result};
use rustok_api::Permission;
use uuid::Uuid;

use super::{
    McpAuditEventGql, McpClientDetailsGql, McpClientGql, McpModuleScaffoldDraftGql,
    ensure_permission, management_context, map_error, require_auth_context, runtime,
};

#[derive(Default)]
pub struct McpQuery;

#[Object]
impl McpQuery {
    async fn mcp_clients(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<McpClientGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_permission(
            auth,
            Permission::MCP_READ,
            "Permission denied: mcp:read required",
        )?;
        runtime(ctx)?
            .port()
            .list_clients(&management_context(auth), limit.map(non_negative_limit))
            .await
            .map(|items| items.into_iter().map(Into::into).collect())
            .map_err(map_error)
    }

    async fn mcp_client(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<McpClientDetailsGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_permission(
            auth,
            Permission::MCP_READ,
            "Permission denied: mcp:read required",
        )?;
        runtime(ctx)?
            .port()
            .get_client(&management_context(auth), id)
            .await
            .map_err(map_error)?
            .map(TryInto::try_into)
            .transpose()
    }

    async fn mcp_audit_events(
        &self,
        ctx: &Context<'_>,
        client_id: Option<Uuid>,
        outcome: Option<String>,
        limit: Option<i32>,
    ) -> Result<Vec<McpAuditEventGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_permission(
            auth,
            Permission::MCP_READ,
            "Permission denied: mcp:read required",
        )?;
        runtime(ctx)?
            .port()
            .list_audit_events(
                &management_context(auth),
                client_id,
                outcome,
                limit.map(non_negative_limit),
            )
            .await
            .map(|items| items.into_iter().map(Into::into).collect())
            .map_err(map_error)
    }

    async fn mcp_module_scaffold_drafts(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<McpModuleScaffoldDraftGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_permission(
            auth,
            Permission::MCP_MANAGE,
            "Permission denied: mcp:manage required",
        )?;
        runtime(ctx)?
            .port()
            .list_scaffold_drafts(&management_context(auth), limit.map(non_negative_limit))
            .await
            .map_err(map_error)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    async fn mcp_module_scaffold_draft(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<Option<McpModuleScaffoldDraftGql>> {
        let auth = require_auth_context(ctx)?;
        ensure_permission(
            auth,
            Permission::MCP_MANAGE,
            "Permission denied: mcp:manage required",
        )?;
        runtime(ctx)?
            .port()
            .get_scaffold_draft(&management_context(auth), id)
            .await
            .map_err(map_error)?
            .map(TryInto::try_into)
            .transpose()
    }
}

fn non_negative_limit(value: i32) -> u64 {
    value.max(0) as u64
}
