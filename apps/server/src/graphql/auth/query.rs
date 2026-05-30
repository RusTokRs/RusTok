use async_graphql::{Context, FieldError, Object, Result};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::context::{AuthContext, TenantContext, infer_user_role_from_permissions};
use crate::graphql::errors::GraphQLError;
use crate::models::users;
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};

use super::types::{AuthUser, SessionItem, SessionsPayload, auth_permission_strings};

const DEFAULT_SESSION_LIMIT: u64 = 50;

#[derive(Default)]
pub struct AuthQuery;

#[Object]
impl AuthQuery {
    /// Health check for auth module
    async fn auth_health(&self) -> &str {
        "Auth module is working!"
    }

    /// Return the currently authenticated user.
    async fn me(&self, ctx: &Context<'_>) -> Result<AuthUser> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let user = users::Entity::find_by_id(auth.user_id)
            .filter(users::Column::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?
            .ok_or_else(<FieldError as GraphQLError>::unauthenticated)?;

        Ok(AuthUser {
            id: user.id.to_string(),
            email: user.email,
            name: user.name,
            role: infer_user_role_from_permissions(&auth.permissions).to_string(),
            status: user.status.to_string(),
            permissions: auth_permission_strings(&auth.permissions),
        })
    }

    /// List active sessions for the currently authenticated user.
    async fn sessions(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<SessionsPayload> {
        let app_ctx = ctx.data::<AppContext>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let cap = limit
            .map(|l| l.clamp(1, 100) as u64)
            .unwrap_or(DEFAULT_SESSION_LIMIT);

        let rows = AuthLifecycleService::list_sessions(app_ctx, tenant.id, auth.user_id, cap)
            .await
            .map_err(|e| match e {
                AuthLifecycleError::Internal(err) => {
                    <FieldError as GraphQLError>::internal_error(&err.to_string())
                }
                other => <FieldError as GraphQLError>::internal_error(&format!("{other:?}")),
            })?;

        let sessions = rows
            .into_iter()
            .map(|s| SessionItem {
                id: s.id.to_string(),
                ip_address: s.ip_address,
                user_agent: s.user_agent,
                last_used_at: s.last_used_at.map(|t| t.to_rfc3339()),
                expires_at: s.expires_at.to_rfc3339(),
                created_at: s.created_at.to_rfc3339(),
                current: s.id == auth.session_id,
            })
            .collect();

        Ok(SessionsPayload { sessions })
    }
}
