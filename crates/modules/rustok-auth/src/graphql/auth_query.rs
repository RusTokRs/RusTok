use async_graphql::{Context, Object, Result};

use super::{
    AuthUser, SessionItem, SessionsPayload, auth_lifecycle_context, auth_runtime,
    require_auth_context,
};

const DEFAULT_SESSION_LIMIT: u64 = 50;

#[derive(Default)]
pub struct AuthQuery;

#[Object]
impl AuthQuery {
    async fn auth_health(&self) -> &str {
        "Auth module is working!"
    }

    async fn me(&self, ctx: &Context<'_>) -> Result<AuthUser> {
        let auth = require_auth_context(ctx)?;
        auth_runtime(ctx)?
            .port()
            .current_user(&auth_lifecycle_context(ctx, Some(auth))?)
            .await
            .map(AuthUser::from)
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))
    }

    async fn sessions(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<SessionsPayload> {
        let auth = require_auth_context(ctx)?;
        let cap = limit
            .map(|value| value.clamp(1, 100) as u64)
            .unwrap_or(DEFAULT_SESSION_LIMIT);
        let sessions = auth_runtime(ctx)?
            .port()
            .list_sessions(&auth_lifecycle_context(ctx, Some(auth))?, cap)
            .await
            .map_err(|error| super::map_auth_lifecycle_error(ctx, error))?
            .into_iter()
            .map(|record| SessionItem::from_record(record, Some(auth.session_id)))
            .collect();

        Ok(SessionsPayload { sessions })
    }
}
