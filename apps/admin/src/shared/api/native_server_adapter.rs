use leptos::prelude::*;
use serde_json::Value;

use super::ServerGraphqlRequest;

#[server(prefix = "/api/fn", endpoint = "admin/graphql")]
pub(super) async fn admin_graphql(request: ServerGraphqlRequest) -> Result<Value, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use axum::http::header::AUTHORIZATION;

        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|error| ServerFnError::new(error.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| ServerFnError::new(error.to_string()))?;
        let headers = leptos_axum::extract::<axum::http::HeaderMap>()
            .await
            .map_err(|error| ServerFnError::new(error.to_string()))?;

        let token = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                value
                    .strip_prefix("Bearer ")
                    .or_else(|| value.strip_prefix("bearer "))
            })
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ServerFnError::new("Authenticated GraphQL bearer token is unavailable"))?;

        let request = ServerGraphqlRequest {
            query: request.query,
            variables: request.variables,
            persisted_query_sha256: request.persisted_query_sha256,
            context: super::ApiRequestContext {
                token: Some(token),
                tenant_slug: Some(tenant.slug),
                locale: request.context.locale,
            },
        };

        super::execute_server_graphql(request)
            .await
            .map_err(|err| ServerFnError::ServerError(err.to_string()))
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "admin/graphql server adapter requires the `ssr` feature",
        ))
    }
}
