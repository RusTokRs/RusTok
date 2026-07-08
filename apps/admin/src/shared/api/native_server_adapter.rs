use leptos::prelude::*;
use serde_json::Value;

use super::ServerGraphqlRequest;

#[server(prefix = "/api/fn", endpoint = "admin/graphql")]
pub(super) async fn admin_graphql(request: ServerGraphqlRequest) -> Result<Value, ServerFnError> {
    super::execute_server_graphql(request)
        .await
        .map_err(|err| ServerFnError::ServerError(err.to_string()))
}
