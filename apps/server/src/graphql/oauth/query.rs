//! GraphQL queries for OAuth App management

use async_graphql::{Context, FieldError, Object, Result};
use rustok_telemetry::metrics;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use sea_orm::DatabaseConnection;

use super::{
    ensure_oauth_admin,
    types::{AppType, OAuthAppGql},
};

#[derive(Default)]
pub struct OAuthQuery;

fn require_auth_context<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    ctx.data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())
}

#[Object]
impl OAuthQuery {
    /// List OAuth apps for the current tenant (admin only)
    async fn oauth_apps(
        &self,
        ctx: &Context<'_>,
        app_type: Option<AppType>,
        limit: Option<i32>,
    ) -> Result<Vec<OAuthAppGql>> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let requested_limit = requested_limit(limit);
        let limit = clamp_limit(limit);

        // Require admin permissions
        ensure_oauth_admin(auth, db).await?;

        use crate::models::oauth_apps;

        let mut query = oauth_apps::Entity::find()
            .filter(oauth_apps::Column::TenantId.eq(auth.tenant_id))
            .filter(oauth_apps::Column::IsActive.eq(true))
            .filter(oauth_apps::Column::RevokedAt.is_null())
            .order_by_desc(oauth_apps::Column::CreatedAt)
            .limit(limit);

        if let Some(app_type) = app_type {
            query = query.filter(oauth_apps::Column::AppType.eq(app_type.as_str()));
        }

        let apps = query
            .all(db)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to list apps: {e}")))?;

        let apps = apps.into_iter().map(OAuthAppGql).collect::<Vec<_>>();
        metrics::record_read_path_budget(
            "graphql",
            "oauth.oauth_apps",
            requested_limit,
            limit,
            apps.len(),
        );

        Ok(apps)
    }

    /// Get a specific OAuth app by ID (admin only)
    async fn oauth_app(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<OAuthAppGql>> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;

        ensure_oauth_admin(auth, db).await?;

        let app = crate::models::oauth_apps::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {e}")))?;

        // Verify tenant ownership
        let app = app.filter(|a| a.tenant_id == auth.tenant_id);

        Ok(app.map(OAuthAppGql))
    }

    /// List apps the current user has authorized
    async fn my_authorized_apps(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<super::types::AuthorizedAppGql>> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let user_id = auth.user_id;
        let requested_limit = requested_limit(limit);
        let limit = clamp_limit(limit);

        // Find active consents joined with apps
        use crate::models::oauth_apps;
        use crate::models::oauth_consents;

        let active_consents: Vec<(oauth_consents::Model, Option<oauth_apps::Model>)> =
            oauth_consents::Entity::find()
                .filter(oauth_consents::Column::UserId.eq(user_id))
                .filter(oauth_consents::Column::TenantId.eq(auth.tenant_id))
                .filter(oauth_consents::Column::RevokedAt.is_null())
                .order_by_desc(oauth_consents::Column::GrantedAt)
                .limit(limit)
                .find_also_related(oauth_apps::Entity)
                .all(db)
                .await
                .map_err(|e| {
                    async_graphql::Error::new(format!("Failed to fetch authorizations: {e}"))
                })?;

        let mut results = Vec::new();
        for (consent, app) in active_consents {
            if let Some(app_model) = app {
                // Ensure app is still active
                if app_model.is_active {
                    results.push(super::types::AuthorizedAppGql {
                        app: app_model,
                        scopes: consent.scopes_list(),
                        granted_at: consent.granted_at.into(),
                    });
                }
            }
        }

        metrics::record_read_path_budget(
            "graphql",
            "oauth.my_authorized_apps",
            requested_limit,
            limit,
            results.len(),
        );

        Ok(results)
    }
}

fn clamp_limit(limit: Option<i32>) -> u64 {
    limit.unwrap_or(50).clamp(1, 100) as u64
}

fn requested_limit(limit: Option<i32>) -> Option<u64> {
    limit.map(|value| value.max(0) as u64)
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema, Value};
    use sea_orm::Database;

    use super::OAuthQuery;
    use crate::context::AuthContext;

    fn auth_context() -> AuthContext {
        AuthContext {
            user_id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        }
    }

    fn error_code(response: &async_graphql::Response) -> Option<&str> {
        response.errors.first().and_then(|error| {
            error
                .extensions
                .as_ref()
                .and_then(|ext| ext.get("code"))
                .and_then(|value| match value {
                    Value::String(code) => Some(code.as_str()),
                    _ => None,
                })
        })
    }

    #[tokio::test]
    async fn my_authorized_apps_requires_auth_context() {
        let schema = Schema::build(OAuthQuery, EmptyMutation, EmptySubscription).finish();

        let response = schema
            .execute(Request::new("{ myAuthorizedApps { scopes } }"))
            .await;

        assert_eq!(error_code(&response), Some("UNAUTHENTICATED"));
    }

    #[tokio::test]
    async fn my_authorized_apps_with_auth_context_is_not_unauthenticated() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = Schema::build(OAuthQuery, EmptyMutation, EmptySubscription)
            .data(db)
            .finish();

        let response = schema
            .execute(Request::new("{ myAuthorizedApps { scopes } }").data(auth_context()))
            .await;

        if let Some(err) = response.errors.first() {
            let code = err
                .extensions
                .as_ref()
                .and_then(|ext| ext.get("code"))
                .and_then(|value| match value {
                    Value::String(code) => Some(code.as_str()),
                    _ => None,
                });
            assert_ne!(code, Some("UNAUTHENTICATED"));
        } else {
            assert!(response.errors.is_empty());
        }
    }
}
