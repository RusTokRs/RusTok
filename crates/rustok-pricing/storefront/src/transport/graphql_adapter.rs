use crate::api::{self, ApiError};
use crate::core::StorefrontPricingQuery;
use crate::model::StorefrontPricingData;

pub(crate) async fn fetch_storefront_pricing(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    api::fetch_storefront_pricing_graphql(query).await
}
