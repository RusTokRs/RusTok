use crate::api::{ApiError, fetch_storefront_forum as fetch_via_api};
use crate::model::StorefrontForumData;

pub async fn fetch_storefront_forum(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, ApiError> {
    fetch_via_api(selected_category_id, selected_topic_id, locale).await
}
