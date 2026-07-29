mod graphql_adapter;
mod native_server_adapter;

use crate::model::StorefrontForumData;

pub type TransportError = graphql_adapter::ApiError;

fn use_native_transport() -> bool {
    cfg!(any(feature = "ssr", feature = "hydrate"))
}

pub async fn fetch_storefront_forum(
    selected_category_id: Option<String>,
    selected_topic_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontForumData, TransportError> {
    if use_native_transport() {
        native_server_adapter::fetch_storefront_forum_server(
            selected_category_id,
            selected_topic_id,
            locale,
        )
        .await
    } else {
        graphql_adapter::fetch_storefront_forum_graphql(
            selected_category_id,
            selected_topic_id,
            locale,
        )
        .await
    }
}

pub async fn mark_storefront_topic_read(
    topic_id: String,
    locale: Option<String>,
) -> Result<(), TransportError> {
    if use_native_transport() {
        native_server_adapter::mark_storefront_topic_read_server(topic_id, locale).await
    } else {
        graphql_adapter::mark_storefront_topic_read_graphql(topic_id, locale).await
    }
}
