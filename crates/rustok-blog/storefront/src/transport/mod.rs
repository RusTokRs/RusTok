pub mod graphql_adapter;
pub mod native_server_adapter;

use crate::api::ApiError;
use crate::core::BlogStorefrontFetchRequest;
use crate::model::StorefrontBlogData;

pub async fn fetch_blog(
    request: BlogStorefrontFetchRequest,
) -> Result<StorefrontBlogData, ApiError> {
    match native_server_adapter::fetch_blog(request.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => graphql_adapter::fetch_blog(request).await,
    }
}
