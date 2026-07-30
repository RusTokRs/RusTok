use super::{
    graphql_adapter,
    graphql_error_safety::{graphql_correlation_id, map_graphql_error},
    native_server_adapter,
    order_change_client_error_safety::OrderChangeClientErrorContext,
};
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
        let operation = "fetch_order_changes";
        let correlation_id = graphql_correlation_id(operation);
        let diagnostic_tenant_id = tenant_id.clone();
        let tenant_slug_length = tenant_slug.as_deref().map(str::len);
        graphql_adapter::fetch_order_changes(token, tenant_slug, tenant_id, order_id, status)
            .await
            .map_err(|error| {
                map_graphql_error(
                    error,
                    operation,
                    &correlation_id,
                    Some(diagnostic_tenant_id.as_str()),
                    tenant_slug_length,
                )
            })
    } else {
        let context = OrderChangeClientErrorContext::for_fetch(
            token.as_deref(),
            tenant_slug.as_deref(),
            tenant_id.as_str(),
            order_id.as_deref(),
            status.as_deref(),
        );
        native_server_adapter::fetch_order_changes(token, tenant_slug, tenant_id, order_id, status)
            .await
            .map_err(|order_change_error| context.map_error(order_change_error))
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
        let operation = "apply_order_change";
        let correlation_id = graphql_correlation_id(operation);
        let diagnostic_tenant_id = tenant_id.clone();
        let tenant_slug_length = tenant_slug.as_deref().map(str::len);
        graphql_adapter::apply_order_change(token, tenant_slug, tenant_id, order_change_id, draft)
            .await
            .map_err(|error| {
                map_graphql_error(
                    error,
                    operation,
                    &correlation_id,
                    Some(diagnostic_tenant_id.as_str()),
                    tenant_slug_length,
                )
            })
    } else {
        let context = OrderChangeClientErrorContext::for_apply(
            token.as_deref(),
            tenant_slug.as_deref(),
            tenant_id.as_str(),
            order_change_id.as_str(),
        );
        native_server_adapter::apply_order_change(
            token,
            tenant_slug,
            tenant_id,
            order_change_id,
            draft,
        )
        .await
        .map_err(|order_change_error| context.map_error(order_change_error))
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
        let operation = "cancel_order_change";
        let correlation_id = graphql_correlation_id(operation);
        let diagnostic_tenant_id = tenant_id.clone();
        let tenant_slug_length = tenant_slug.as_deref().map(str::len);
        graphql_adapter::cancel_order_change(token, tenant_slug, tenant_id, order_change_id, draft)
            .await
            .map_err(|error| {
                map_graphql_error(
                    error,
                    operation,
                    &correlation_id,
                    Some(diagnostic_tenant_id.as_str()),
                    tenant_slug_length,
                )
            })
    } else {
        let context = OrderChangeClientErrorContext::for_cancel(
            token.as_deref(),
            tenant_slug.as_deref(),
            tenant_id.as_str(),
            order_change_id.as_str(),
        );
        native_server_adapter::cancel_order_change(
            token,
            tenant_slug,
            tenant_id,
            order_change_id,
            draft,
        )
        .await
        .map_err(|order_change_error| context.map_error(order_change_error))
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
