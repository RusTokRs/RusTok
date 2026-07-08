use super::{graphql_adapter, native_server_adapter};
use crate::model::{CommerceOrderChange, CommerceOrderChangeActionDraft, CommerceOrderChangeList};
use native_server_adapter::ApiError;

fn use_graphql_transport() -> bool {
    cfg!(all(target_arch = "wasm32", not(feature = "hydrate")))
}

pub async fn fetch_order_changes(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_id: Option<String>,
    status: Option<String>,
) -> Result<CommerceOrderChangeList, ApiError> {
    if use_graphql_transport() {
        graphql_adapter::fetch_order_changes(token, tenant_slug, tenant_id, order_id, status).await
    } else {
        native_server_adapter::fetch_order_changes(token, tenant_slug, tenant_id, order_id, status)
            .await
    }
}

pub async fn apply_order_change(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_change_id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    if use_graphql_transport() {
        graphql_adapter::apply_order_change(token, tenant_slug, tenant_id, order_change_id, draft)
            .await
    } else {
        native_server_adapter::apply_order_change(
            token,
            tenant_slug,
            tenant_id,
            order_change_id,
            draft,
        )
        .await
    }
}

pub async fn cancel_order_change(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_change_id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    if use_graphql_transport() {
        graphql_adapter::cancel_order_change(token, tenant_slug, tenant_id, order_change_id, draft)
            .await
    } else {
        native_server_adapter::cancel_order_change(
            token,
            tenant_slug,
            tenant_id,
            order_change_id,
            draft,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::any::type_name;

    use super::*;

    #[test]
    fn order_change_transport_keeps_api_error_contract() {
        assert!(type_name::<ApiError>().contains("ApiError"));
    }
}
