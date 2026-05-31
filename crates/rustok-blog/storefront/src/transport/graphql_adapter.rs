use crate::api::{self, ApiError};
use crate::core::BlogStorefrontFetchRequest;
use crate::model::StorefrontBlogData;

pub async fn fetch_blog(
    request: BlogStorefrontFetchRequest,
) -> Result<StorefrontBlogData, ApiError> {
    api::fetch_storefront_blog_graphql(request.post_slug, request.locale).await
}
