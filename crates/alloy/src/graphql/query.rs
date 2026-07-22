use async_graphql::{Context, Object, Result};
use rustok_api::graphql::{PageInfo, PaginationInput};
use rustok_telemetry::metrics;
use uuid::Uuid;

use crate::{ScriptRegistry, storage::ScriptQuery};

use super::{
    GqlEventType, GqlExecutionLogConnection, GqlExecutionLogEntry, GqlReviewDecision, GqlScript,
    GqlScriptConnection, GqlScriptStatus, require_admin, runtime_from_graphql_ctx,
};

pub const EXECUTION_HISTORY_GRAPHQL_FIELDS: &[&str] = &[
    "scriptExecutions",
    "scriptExecutionHistory",
    "recentScriptExecutions",
];

#[derive(Default)]
pub struct AlloyQuery;

#[Object]
impl AlloyQuery {
    async fn script_reviews(
        &self,
        ctx: &Context<'_>,
        script_id: Uuid,
        revision: u32,
    ) -> Result<Vec<GqlReviewDecision>> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        runtime
            .storage
            .list_reviews(script_id, revision)
            .await
            .map(|decisions| decisions.into_iter().map(Into::into).collect())
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }

    async fn scripts(
        &self,
        ctx: &Context<'_>,
        status: Option<GqlScriptStatus>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<GqlScriptConnection> {
        require_admin(ctx).await?;
        let state = runtime_from_graphql_ctx(ctx)?;
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

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
            page_info: PageInfo::new(page.total as i64, offset, limit),
        })
    }

    async fn script(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<GqlScript>> {
        require_admin(ctx).await?;
        let state = runtime_from_graphql_ctx(ctx)?;

        match state.storage.get(id).await {
            Ok(script) => Ok(Some(script.into())),
            Err(_) => Ok(None),
        }
    }

    async fn script_by_name(&self, ctx: &Context<'_>, name: String) -> Result<Option<GqlScript>> {
        require_admin(ctx).await?;
        let state = runtime_from_graphql_ctx(ctx)?;

        match state.storage.get_by_name(&name).await {
            Ok(script) => Ok(Some(script.into())),
            Err(_) => Ok(None),
        }
    }

    async fn script_executions(
        &self,
        ctx: &Context<'_>,
        script_id: Option<Uuid>,
        limit: Option<i32>,
    ) -> Result<Vec<GqlExecutionLogEntry>> {
        require_admin(ctx).await?;
        let state = runtime_from_graphql_ctx(ctx)?;
        let requested_limit = limit.map(|value| value.max(0) as u64);
        let limit = limit.unwrap_or(50).clamp(1, 100) as u64;

        let entries = match script_id {
            Some(script_id) => {
                state
                    .execution_log
                    .list_for_script_for_tenant(script_id, state.tenant_id, limit)
                    .await
            }
            None => {
                state
                    .execution_log
                    .list_recent_for_tenant(state.tenant_id, limit)
                    .await
            }
        }
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        metrics::record_read_path_budget(
            "graphql",
            "alloy.script_executions",
            requested_limit,
            limit,
            entries.len(),
        );

        Ok(entries
            .into_iter()
            .map(GqlExecutionLogEntry::from)
            .collect())
    }

    async fn scripts_for_event(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        event: GqlEventType,
        limit: Option<i32>,
    ) -> Result<Vec<GqlScript>> {
        require_admin(ctx).await?;
        let state = runtime_from_graphql_ctx(ctx)?;
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
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

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

    async fn script_execution_history(
        &self,
        ctx: &Context<'_>,
        script_id: Uuid,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<GqlExecutionLogConnection> {
        require_admin(ctx).await?;
        let state = runtime_from_graphql_ctx(ctx)?;
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;

        let entries = state
            .execution_log
            .list_for_script_for_tenant_paginated(
                script_id,
                state.tenant_id,
                offset as u64,
                limit as u64,
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .into_iter()
            .map(GqlExecutionLogEntry::from)
            .collect::<Vec<_>>();
        let total = state
            .execution_log
            .count_for_script_for_tenant(script_id, state.tenant_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            as i64;

        metrics::record_read_path_budget(
            "graphql",
            "alloy.script_execution_history",
            Some(requested_limit),
            limit as u64,
            entries.len(),
        );

        Ok(GqlExecutionLogConnection {
            page_info: PageInfo::new(total, offset, limit),
            items: entries,
        })
    }

    async fn recent_script_executions(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<GqlExecutionLogConnection> {
        require_admin(ctx).await?;
        let state = runtime_from_graphql_ctx(ctx)?;
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;

        let entries = state
            .execution_log
            .list_recent_for_tenant_paginated(state.tenant_id, offset as u64, limit as u64)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .into_iter()
            .map(GqlExecutionLogEntry::from)
            .collect::<Vec<_>>();
        let total = state
            .execution_log
            .count_recent_for_tenant(state.tenant_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            as i64;

        metrics::record_read_path_budget(
            "graphql",
            "alloy.recent_script_executions",
            Some(requested_limit),
            limit as u64,
            entries.len(),
        );

        Ok(GqlExecutionLogConnection {
            page_info: PageInfo::new(total, offset, limit),
            items: entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::EXECUTION_HISTORY_GRAPHQL_FIELDS;

    #[test]
    fn execution_history_graphql_fields_match_public_schema_contract() {
        assert_eq!(
            EXECUTION_HISTORY_GRAPHQL_FIELDS,
            &[
                "scriptExecutions",
                "scriptExecutionHistory",
                "recentScriptExecutions",
            ]
        );
    }
}
