use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::{CreateOAuthAppCommand, UpdateOAuthAppCommand};

use super::{
    CreateOAuthAppInput, CreateOAuthAppResultGql, OAuthAppGql, RotateSecretResultGql,
    UpdateOAuthAppInput, map_error, mutation_context, require_auth_context, runtime,
};

#[derive(Default)]
pub struct OAuthMutation;

#[Object]
impl OAuthMutation {
    async fn create_oauth_app(
        &self,
        ctx: &Context<'_>,
        input: CreateOAuthAppInput,
    ) -> Result<CreateOAuthAppResultGql> {
        let auth = require_auth_context(ctx)?;
        let result = runtime(ctx)?
            .port()
            .create_oauth_app(
                &mutation_context(ctx, auth),
                CreateOAuthAppCommand {
                    name: input.name,
                    slug: input.slug,
                    description: input.description,
                    app_type: input.app_type.as_str().to_string(),
                    icon_url: input.icon_url,
                    redirect_uris: input.redirect_uris.unwrap_or_default(),
                    scopes: input.scopes,
                    grant_types: input.grant_types,
                    granted_permissions: input.granted_permissions,
                },
            )
            .await
            .map_err(map_error)?;
        Ok(CreateOAuthAppResultGql {
            app: OAuthAppGql(result.app),
            client_secret: result.client_secret,
        })
    }

    async fn update_oauth_app(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateOAuthAppInput,
    ) -> Result<OAuthAppGql> {
        let auth = require_auth_context(ctx)?;
        runtime(ctx)?
            .port()
            .update_oauth_app(
                &mutation_context(ctx, auth),
                UpdateOAuthAppCommand {
                    id,
                    name: input.name,
                    description: input.description,
                    icon_url: input.icon_url,
                    redirect_uris: input.redirect_uris,
                    scopes: input.scopes,
                    grant_types: input.grant_types,
                    granted_permissions: input.granted_permissions,
                },
            )
            .await
            .map(OAuthAppGql)
            .map_err(map_error)
    }

    async fn rotate_oauth_app_secret(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<RotateSecretResultGql> {
        let auth = require_auth_context(ctx)?;
        let result = runtime(ctx)?
            .port()
            .rotate_oauth_app_secret(&mutation_context(ctx, auth), id)
            .await
            .map_err(map_error)?;
        Ok(RotateSecretResultGql {
            app: OAuthAppGql(result.app),
            client_secret: result.client_secret,
        })
    }

    async fn revoke_oauth_app(&self, ctx: &Context<'_>, id: Uuid) -> Result<OAuthAppGql> {
        let auth = require_auth_context(ctx)?;
        runtime(ctx)?
            .port()
            .revoke_oauth_app(&mutation_context(ctx, auth), id)
            .await
            .map(OAuthAppGql)
            .map_err(map_error)
    }

    async fn grant_app_consent(
        &self,
        ctx: &Context<'_>,
        app_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<bool> {
        let auth = require_auth_context(ctx)?;
        runtime(ctx)?
            .port()
            .grant_oauth_app_consent(&mutation_context(ctx, auth), app_id, scopes)
            .await
            .map_err(map_error)?;
        Ok(true)
    }

    async fn revoke_app_consent(&self, ctx: &Context<'_>, app_id: Uuid) -> Result<bool> {
        let auth = require_auth_context(ctx)?;
        runtime(ctx)?
            .port()
            .revoke_oauth_app_consent(&mutation_context(ctx, auth), app_id)
            .await
            .map_err(map_error)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptySubscription, Object, Request, Schema, Value};

    use super::OAuthMutation;

    struct TestQueryRoot;

    #[Object]
    impl TestQueryRoot {
        async fn health(&self) -> &str {
            "ok"
        }
    }

    #[tokio::test]
    async fn revoke_consent_requires_auth_context() {
        let schema = Schema::build(TestQueryRoot, OAuthMutation, EmptySubscription).finish();
        let response = schema
            .execute(Request::new(
                "mutation { revokeAppConsent(appId: \"550e8400-e29b-41d4-a716-446655440000\") }",
            ))
            .await;

        let code = response.errors.first().and_then(|error| {
            error.extensions.as_ref()?.get("code").and_then(|value| {
                if let Value::String(code) = value {
                    Some(code.as_str())
                } else {
                    None
                }
            })
        });
        assert_eq!(code, Some("UNAUTHENTICATED"));
    }
}
