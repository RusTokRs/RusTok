use async_graphql::{Context, Object, Result};
use rustok_telemetry::metrics;
use uuid::Uuid;

use alloy_scripting::storage::ScriptQuery;
use alloy_scripting::ScriptRegistry;

use super::{
    require_admin, AlloyState, GqlEventType, GqlScript, GqlScriptConnection, GqlScriptStatus,
};
use crate::graphql::common::require_module_enabled;
use crate::graphql::common::PaginationInput;
use crate::graphql::schema::module_slug;

#[derive(Default)]
pub struct AlloyQuery;

#[Object]
impl AlloyQuery {
    async fn scripts(
        &self,
        ctx: &Context<'_>,
        status: Option<GqlScriptStatus>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<GqlScriptConnection> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let requested_limit = pagination.requested_limit();
        let query = match status {
            Some(status) => ScriptQuery::ByStatus(status.into()),
            None => ScriptQuery::All,
        };

        let (offset, limit) = pagination.normalize()?;
        let page = state
            .storage
            .find_paginated(query, offset as u64, limit as u64)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        let items = page
            .items
            .into_iter()
            .map(GqlScript::from)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "alloy.scripts",
            Some(requested_limit),
            limit as u64,
            items.len(),
        );

        Ok(GqlScriptConnection {
            items,
            page_info: crate::graphql::common::PageInfo::new(page.total as i64, offset, limit),
        })
    }

    async fn script(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<GqlScript>> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        match state.storage.get(id).await {
            Ok(script) => Ok(Some(script.into())),
            Err(_) => Ok(None),
        }
    }

    async fn script_by_name(&self, ctx: &Context<'_>, name: String) -> Result<Option<GqlScript>> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        match state.storage.get_by_name(&name).await {
            Ok(script) => Ok(Some(script.into())),
            Err(_) => Ok(None),
        }
    }

    async fn scripts_for_event(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        event: GqlEventType,
        limit: Option<i32>,
    ) -> Result<Vec<GqlScript>> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let requested_limit = limit.map(|value| value.max(0) as u64);
        let limit = limit.unwrap_or(50).clamp(1, 100) as u64;
        let page = state
            .storage
            .find_paginated(
                ScriptQuery::ByEvent {
                    entity_type,
                    event: event.into(),
                },
                0,
                limit,
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        let scripts = page
            .items
            .into_iter()
            .map(GqlScript::from)
            .collect::<Vec<_>>();
        metrics::record_read_path_budget(
            "graphql",
            "alloy.scripts_for_event",
            requested_limit,
            limit,
            scripts.len(),
        );

        Ok(scripts)
    }
}
