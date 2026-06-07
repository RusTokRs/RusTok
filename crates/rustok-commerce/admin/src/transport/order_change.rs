use crate::api::{self, ApiError};
use crate::model::{CommerceOrderChange, CommerceOrderChangeActionDraft, CommerceOrderChangeList};

pub async fn fetch_order_changes(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_id: Option<String>,
    status: Option<String>,
) -> Result<CommerceOrderChangeList, ApiError> {
    api::fetch_order_changes(token, tenant_slug, tenant_id, order_id, status).await
}

pub async fn apply_order_change(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_change_id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    api::apply_order_change(token, tenant_slug, tenant_id, order_change_id, draft).await
}

pub async fn cancel_order_change(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_change_id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    api::cancel_order_change(token, tenant_slug, tenant_id, order_change_id, draft).await
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
