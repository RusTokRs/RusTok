use crate::api::{self, ApiError};
use crate::model::StorefrontPagesData;

pub async fn fetch_pages(
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    api::fetch_storefront_pages(page_slug, locale).await
}
