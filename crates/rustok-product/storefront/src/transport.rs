use crate::api::{self, ApiError};
use crate::model::StorefrontProductsData;

#[allow(clippy::too_many_arguments)]
pub async fn fetch_products(
    selected_handle: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<StorefrontProductsData, ApiError> {
    api::fetch_storefront_products(
        selected_handle,
        locale,
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity,
    )
    .await
}
