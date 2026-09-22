use super::shared_adapter::{self, ApiError};
use crate::core::FetchCommerceRequest;
use crate::model::StorefrontCommerceData;

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    shared_adapter::fetch_storefront_commerce_graphql(request.selected_cart_id, request.locale)
        .await
}
