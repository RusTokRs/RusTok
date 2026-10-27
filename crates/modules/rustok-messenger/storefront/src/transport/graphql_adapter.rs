use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::Deserialize;
use uuid::Uuid;

use crate::model::*;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiError {
    #[error("GraphQL error: {0}")]
    Graphql(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Network error: {0}")]
    Network(String),
}

impl From<rustok_graphql::GraphqlHttpError> for ApiError {
    fn from(err: rustok_graphql::GraphqlHttpError) -> Self {
        Self::Graphql(err.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

fn graphql_url() -> String {
    "/api/graphql".to_string()
}

fn sanitize_token(token: Option<String>) -> Option<String> {
    token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

fn sanitize_tenant_slug(tenant_slug: Option<String>) -> Option<String> {
    tenant_slug.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

const LIST_CONVERSATIONS_QUERY: &str = r#"
query ListStorefrontConversations {
    messengerConversations {
        id
        conversationType
        title
        topic
        lastMessageContent
        lastMessageAt
        unreadCount
        isEncrypted
        entityType
        entityId
    }
}
"#;

#[derive(Deserialize)]
struct ListConversationsResponse {
    #[serde(rename = "messengerConversations")]
    conversations: Option<Vec<ConversationItemPayload>>,
}

pub async fn fetch_conversations(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ConversationItemPayload>, ApiError> {
    let response: ListConversationsResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(LIST_CONVERSATIONS_QUERY, None::<serde_json::Value>),
        sanitize_token(token),
        sanitize_tenant_slug(tenant_slug),
        None::<String>,
    )
    .await?;

    Ok(response.conversations.unwrap_or_default())
}

const LIST_MESSAGES_QUERY: &str = r#"
query ListConversationMessages($conversationId: UUID!, $sinceSeq: Long, $limit: Int) {
    conversationMessages(conversationId: $conversationId, sinceSeq: $sinceSeq, limit: $limit) {
        id
        conversationId
        senderId
        senderName
        messageType
        content
        blocksJson
        replyToId
        isEncrypted
        createdAt
    }
}
"#;

#[derive(Deserialize)]
struct ListMessagesResponse {
    #[serde(rename = "conversationMessages")]
    messages: Option<Vec<MessageItemPayload>>,
}

pub async fn fetch_messages(
    token: Option<String>,
    tenant_slug: Option<String>,
    conversation_id: Uuid,
    since_seq: Option<i64>,
    limit: Option<i32>,
) -> Result<Vec<MessageItemPayload>, ApiError> {
    if conversation_id.is_nil() {
        return Err(ApiError::Graphql("conversation_id cannot be nil UUID".into()));
    }
    if let Some(seq) = since_seq {
        if seq < 0 {
            return Err(ApiError::Graphql("since_seq cannot be negative".into()));
        }
    }
    if let Some(l) = limit {
        if l <= 0 {
            return Err(ApiError::Graphql("limit must be positive".into()));
        }
    }

    let variables = serde_json::json!({
        "conversationId": conversation_id,
        "sinceSeq": since_seq,
        "limit": limit.unwrap_or(50),
    });

    let response: ListMessagesResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(LIST_MESSAGES_QUERY, Some(variables)),
        sanitize_token(token),
        sanitize_tenant_slug(tenant_slug),
        None::<String>,
    )
    .await?;

    Ok(response.messages.unwrap_or_default())
}

const SEND_MESSAGE_MUTATION: &str = r#"
mutation SendChatMessage($input: SendMessageInputGql!) {
    sendChatMessage(input: $input) {
        id
        conversationId
        senderId
        content
        createdAt
    }
}
"#;

#[derive(Deserialize)]
struct SendMessageResponse {
    #[serde(rename = "sendChatMessage")]
    message: Option<MessageItemPayload>,
}

pub async fn send_message(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: SendMessagePayload,
) -> Result<MessageItemPayload, ApiError> {
    if input.conversation_id.is_nil() {
        return Err(ApiError::Graphql("conversation_id cannot be nil UUID".into()));
    }
    let trimmed_content = input.content.trim();
    if trimmed_content.is_empty() {
        return Err(ApiError::Graphql("content cannot be empty".into()));
    }

    let variables = serde_json::json!({
        "input": {
            "conversationId": input.conversation_id,
            "content": trimmed_content,
            "replyToId": input.reply_to_id,
        }
    });

    let response: SendMessageResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(SEND_MESSAGE_MUTATION, Some(variables)),
        sanitize_token(token),
        sanitize_tenant_slug(tenant_slug),
        None::<String>,
    )
    .await?;

    response
        .message
        .ok_or_else(|| ApiError::Graphql("Failed to send message: empty response".into()))
}

const UPDATE_CURSOR_MUTATION: &str = r#"
mutation UpdateReadCursor($input: UpdateCursorInputGql!) {
    updateReadCursor(input: $input) {
        conversationId
        lastReadMessageId
    }
}
"#;

pub async fn update_cursor(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: UpdateCursorPayload,
) -> Result<(), ApiError> {
    if input.conversation_id.is_nil() {
        return Err(ApiError::Graphql("conversation_id cannot be nil UUID".into()));
    }
    if input.last_read_message_id < 0 {
        return Err(ApiError::Graphql("last_read_message_id cannot be negative".into()));
    }

    let variables = serde_json::json!({
        "input": {
            "conversationId": input.conversation_id,
            "lastReadMessageId": input.last_read_message_id,
        }
    });

    let _response: serde_json::Value = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(UPDATE_CURSOR_MUTATION, Some(variables)),
        sanitize_token(token),
        sanitize_tenant_slug(tenant_slug),
        None::<String>,
    )
    .await?;

    Ok(())
}
