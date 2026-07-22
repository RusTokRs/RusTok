use async_graphql::{Context, Object, Result};
use rustok_telemetry::metrics;
use uuid::Uuid;

use super::{
    AppType, AuthorizedAppGql, OAuthAppGql, map_error, mutation_context, require_auth_context,
    runtime,
};

#[derive(Default)]
pub struct OAuthQuery;

#[Object]
impl OAuthQuery {
    async fn oauth_apps(
        &self,
        ctx: &Context<'_>,
        app_type: Option<AppType>,
        limit: Option<i32>,
    ) -> Result<Vec<OAuthAppGql>> {
        let auth = require_auth_context(ctx)?;
        let requested_limit = requested_limit(limit);
        let limit = clamp_limit(limit);
        let apps = runtime(ctx)?
            .port()
            .list_oauth_apps(
                &mutation_context(auth),
                app_type.map(|value| value.as_str().to_string()),
                limit,
            )
            .await
            .map_err(map_error)?;
        metrics::record_read_path_budget(
            "graphql",
            "oauth.oauth_apps",
            requested_limit,
            limit,
            apps.len(),
        );
        Ok(apps.into_iter().map(OAuthAppGql).collect())
    }

    async fn oauth_app(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<OAuthAppGql>> {
        let auth = require_auth_context(ctx)?;
        runtime(ctx)?
            .port()
            .get_oauth_app(&mutation_context(auth), id)
            .await
            .map(|app| app.map(OAuthAppGql))
            .map_err(map_error)
    }

    async fn my_authorized_apps(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<AuthorizedAppGql>> {
        let auth = require_auth_context(ctx)?;
        let requested_limit = requested_limit(limit);
        let limit = clamp_limit(limit);
        let apps = runtime(ctx)?
            .port()
            .list_authorized_oauth_apps(&mutation_context(auth), limit)
            .await
            .map_err(map_error)?;
        metrics::record_read_path_budget(
            "graphql",
            "oauth.my_authorized_apps",
            requested_limit,
            limit,
            apps.len(),
        );
        Ok(apps.into_iter().map(AuthorizedAppGql).collect())
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

    use super::OAuthQuery;

    #[tokio::test]
    async fn authorized_apps_require_auth_context() {
        let schema = Schema::build(OAuthQuery, EmptyMutation, EmptySubscription).finish();
        let response = schema
            .execute(Request::new("{ myAuthorizedApps { scopes } }"))
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
