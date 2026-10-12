//! Composer write transport for the GraphQL storefront path.
//!
//! The create mutations go through the shared `super::graphql_adapter::request`
//! helper so error redaction, tenant-slug resolution, and the GraphQL endpoint
//! selection stay defined once for the whole storefront transport.

use rustok_api::RichTextDocument;
use serde::{Deserialize, Serialize};

use super::graphql_adapter::{ApiError, request};
use crate::core::forum_storefront_quote_target_kind;
use crate::model::{
    CreateStorefrontForumReplyDraft, CreateStorefrontForumTopicDraft, StorefrontForumComposerQuote,
    StorefrontForumCreatedReply, StorefrontForumCreatedTopic,
};

const CREATE_STOREFRONT_FORUM_TOPIC_MUTATION: &str = "mutation CreateStorefrontForumTopic($tenantId: UUID, $input: CreateForumTopicWithQuotesInput!) { createForumTopicWithQuotes(tenantId: $tenantId, input: $input) { id slug } }";
const CREATE_STOREFRONT_FORUM_REPLY_MUTATION: &str = "mutation CreateStorefrontForumReply($tenantId: UUID, $topicId: UUID!, $input: CreateForumReplyWithQuotesInput!) { createForumReplyWithQuotes(tenantId: $tenantId, topicId: $topicId, input: $input) { id topicId } }";

#[derive(Debug, Serialize)]
struct CreateTopicVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    input: CreateTopicGraphqlInput,
}

#[derive(Debug, Serialize)]
struct CreateTopicGraphqlInput {
    locale: String,
    #[serde(rename = "categoryId")]
    category_id: String,
    title: String,
    body: RichTextDocument,
    tags: Vec<String>,
    quotes: Vec<QuoteGraphqlInput>,
}

#[derive(Debug, Serialize)]
struct CreateReplyVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "topicId")]
    topic_id: String,
    input: CreateReplyGraphqlInput,
}

#[derive(Debug, Serialize)]
struct CreateReplyGraphqlInput {
    locale: String,
    content: RichTextDocument,
    #[serde(rename = "parentReplyId")]
    parent_reply_id: Option<String>,
    quotes: Vec<QuoteGraphqlInput>,
}

#[derive(Debug, Serialize)]
struct QuoteGraphqlInput {
    #[serde(rename = "targetKind")]
    target_kind: &'static str,
    #[serde(rename = "targetId")]
    target_id: String,
    #[serde(rename = "revisionId")]
    revision_id: i64,
}

#[derive(Debug, Deserialize)]
struct CreateStorefrontForumTopicResponse {
    #[serde(rename = "createForumTopicWithQuotes")]
    created: CreatedTopicPayload,
}

#[derive(Debug, Deserialize)]
struct CreatedTopicPayload {
    id: String,
    slug: String,
}

#[derive(Debug, Deserialize)]
struct CreateStorefrontForumReplyResponse {
    #[serde(rename = "createForumReplyWithQuotes")]
    created: CreatedReplyPayload,
}

#[derive(Debug, Deserialize)]
struct CreatedReplyPayload {
    id: String,
    #[serde(rename = "topicId")]
    topic_id: String,
}

fn composer_quote_inputs(
    quotes: &[StorefrontForumComposerQuote],
) -> Result<Vec<QuoteGraphqlInput>, ApiError> {
    quotes
        .iter()
        .map(|quote| {
            let target_kind = forum_storefront_quote_target_kind(quote.target_kind.as_str())
                .ok_or_else(|| {
                    ApiError::Graphql(
                        "Forum composer quote target kind is not supported".to_string(),
                    )
                })?;
            Ok(QuoteGraphqlInput {
                target_kind,
                target_id: quote.target_id.clone(),
                revision_id: quote.revision_id,
            })
        })
        .collect()
}

pub async fn create_storefront_topic_graphql(
    draft: CreateStorefrontForumTopicDraft,
) -> Result<StorefrontForumCreatedTopic, ApiError> {
    let body = RichTextDocument::from_plain_text(draft.body_text.as_str());
    if !body.has_text() {
        return Err(ApiError::Graphql(
            "Forum composer body must carry text".to_string(),
        ));
    }
    let response: CreateStorefrontForumTopicResponse = request(
        CREATE_STOREFRONT_FORUM_TOPIC_MUTATION,
        CreateTopicVariables {
            tenant_id: None,
            input: CreateTopicGraphqlInput {
                locale: draft.locale,
                category_id: draft.category_id,
                title: draft.title,
                body,
                tags: Vec::new(),
                quotes: composer_quote_inputs(&draft.quotes)?,
            },
        },
    )
    .await?;

    Ok(StorefrontForumCreatedTopic {
        topic_id: response.created.id,
        slug: response.created.slug,
    })
}

pub async fn create_storefront_reply_graphql(
    draft: CreateStorefrontForumReplyDraft,
) -> Result<StorefrontForumCreatedReply, ApiError> {
    let content = RichTextDocument::from_plain_text(draft.body_text.as_str());
    if !content.has_text() {
        return Err(ApiError::Graphql(
            "Forum composer body must carry text".to_string(),
        ));
    }
    let response: CreateStorefrontForumReplyResponse = request(
        CREATE_STOREFRONT_FORUM_REPLY_MUTATION,
        CreateReplyVariables {
            tenant_id: None,
            topic_id: draft.topic_id,
            input: CreateReplyGraphqlInput {
                locale: draft.locale,
                content,
                parent_reply_id: draft.parent_reply_id,
                quotes: composer_quote_inputs(&draft.quotes)?,
            },
        },
    )
    .await?;

    Ok(StorefrontForumCreatedReply {
        reply_id: response.created.id,
        topic_id: response.created.topic_id,
    })
}
