pub mod graphql_adapter;
pub mod native_server_adapter;

pub use graphql_adapter::ApiError;
use uuid::Uuid;

use crate::model::*;

pub async fn fetch_conversations(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ConversationItemPayload>, ApiError> {
    #[cfg(feature = "ssr")]
    {
        native_server_adapter::fetch_conversations(token, tenant_slug)
            .await
            .map_err(ApiError::Network)
    }
    #[cfg(not(feature = "ssr"))]
    {
        graphql_adapter::fetch_conversations(token, tenant_slug).await
    }
}

pub async fn fetch_messages(
    token: Option<String>,
    tenant_slug: Option<String>,
    conversation_id: Uuid,
    since_seq: Option<i64>,
    limit: Option<i32>,
) -> Result<Vec<MessageItemPayload>, ApiError> {
    #[cfg(feature = "ssr")]
    {
        native_server_adapter::fetch_messages(token, tenant_slug, conversation_id, since_seq, limit)
            .await
            .map_err(ApiError::Network)
    }
    #[cfg(not(feature = "ssr"))]
    {
        graphql_adapter::fetch_messages(token, tenant_slug, conversation_id, since_seq, limit).await
    }
}

pub async fn send_message(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: SendMessagePayload,
) -> Result<MessageItemPayload, ApiError> {
    #[cfg(feature = "ssr")]
    {
        native_server_adapter::send_message(token, tenant_slug, input)
            .await
            .map_err(ApiError::Network)
    }
    #[cfg(not(feature = "ssr"))]
    {
        graphql_adapter::send_message(token, tenant_slug, input).await
    }
}

pub async fn update_cursor(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: UpdateCursorPayload,
) -> Result<(), ApiError> {
    #[cfg(feature = "ssr")]
    {
        native_server_adapter::update_cursor(token, tenant_slug, input)
            .await
            .map_err(ApiError::Network)
    }
    #[cfg(not(feature = "ssr"))]
    {
        graphql_adapter::update_cursor(token, tenant_slug, input).await
    }
}
