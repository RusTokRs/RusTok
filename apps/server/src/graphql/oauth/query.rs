//! GraphQL queries for OAuth App management

use async_graphql::{Context, FieldError, Object, Result};
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::oauth_app::OAuthAppService;
use sea_orm::{DatabaseConnection, EntityTrait};

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
    ) -> Result<Vec<OAuthAppGql>> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;

        // Require admin permissions
        ensure_oauth_admin(auth, db).await?;

        let apps = OAuthAppService::list_by_tenant(db, auth.tenant_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to list apps: {e}")))?;

        let apps = if let Some(app_type) = app_type {
            apps.into_iter()
                .filter(|a| a.app_type == app_type.as_str())
                .map(OAuthAppGql)
                .collect()
        } else {
            apps.into_iter().map(OAuthAppGql).collect()
        };

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
    ) -> Result<Vec<super::types::AuthorizedAppGql>> {
        let auth = require_auth_context(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let user_id = auth.user_id;

        // Find active consents joined with apps
        use crate::models::oauth_apps;
        use crate::models::oauth_consents;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let active_consents: Vec<(oauth_consents::Model, Option<oauth_apps::Model>)> =
            oauth_consents::Entity::find()
                .filter(oauth_consents::Column::UserId.eq(user_id))
                .filter(oauth_consents::Column::TenantId.eq(auth.tenant_id))
                .filter(oauth_consents::Column::RevokedAt.is_null())
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

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
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

    #[tokio::test]
    async fn my_authorized_apps_requires_auth_context() {
        let schema =
            Schema::build(OAuthQuery::default(), EmptyMutation, EmptySubscription).finish();

        let response = schema
            .execute(Request::new("{ myAuthorizedApps { scopes } }"))
            .await;

        let code = response.errors[0]
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .and_then(|value| value.as_str());

        assert_eq!(code, Some("UNAUTHENTICATED"));
    }

    #[tokio::test]
    async fn my_authorized_apps_with_auth_context_is_not_unauthenticated() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = Schema::build(OAuthQuery::default(), EmptyMutation, EmptySubscription)
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
                .and_then(|value| value.as_str());
            assert_ne!(code, Some("UNAUTHENTICATED"));
        } else {
            assert!(response.errors.is_empty());
        }
    }
}
