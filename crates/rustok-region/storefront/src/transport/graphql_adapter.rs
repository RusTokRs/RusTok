use crate::api::{self, ApiError};
use crate::model::StorefrontRegionsData;

pub async fn fetch_regions(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ApiError> {
    api::fetch_storefront_regions_graphql(selected_region_id, locale).await
}
