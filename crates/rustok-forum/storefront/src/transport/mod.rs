mod graphql_adapter;

use crate::model::StorefrontForumData;

pub type TransportError = graphql_adapter::ApiError;

pub async fn fetch_storefront_forum(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, TransportError> {
    graphql_adapter::fetch_storefront_forum(selected_category_id, selected_topic_id, locale).await
}
