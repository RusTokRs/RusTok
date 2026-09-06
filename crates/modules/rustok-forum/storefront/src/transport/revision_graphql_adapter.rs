const STOREFRONT_FORUM_TOPIC_CURRENT_REVISION_QUERY: &str = "query StorefrontForumTopicCurrentRevision($tenantId: UUID, $id: UUID!, $locale: String) { forumStorefrontTopicCurrentRevision(tenantId: $tenantId, id: $id, locale: $locale) }";
const STOREFRONT_FORUM_REPLY_CURRENT_REVISION_QUERY: &str = "query StorefrontForumReplyCurrentRevision($tenantId: UUID, $id: UUID!, $locale: String) { forumStorefrontReplyCurrentRevision(tenantId: $tenantId, id: $id, locale: $locale) }";

#[derive(Debug, Serialize)]
struct StorefrontForumRevisionVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumTopicCurrentRevisionResponse {
    #[serde(rename = "forumStorefrontTopicCurrentRevision")]
    revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StorefrontForumReplyCurrentRevisionResponse {
    #[serde(rename = "forumStorefrontReplyCurrentRevision")]
    revision: Option<String>,
}

pub async fn fetch_storefront_topic_current_revision_graphql(
    topic_id: String,
    locale: Option<String>,
) -> Result<Option<String>, ApiError> {
    let response: StorefrontForumTopicCurrentRevisionResponse = request(
        STOREFRONT_FORUM_TOPIC_CURRENT_REVISION_QUERY,
        StorefrontForumRevisionVariables {
            tenant_id: None,
            id: topic_id,
            locale,
        },
    )
    .await?;
    Ok(response.revision)
}

pub async fn fetch_storefront_reply_current_revision_graphql(
    reply_id: String,
    locale: Option<String>,
) -> Result<Option<String>, ApiError> {
    let response: StorefrontForumReplyCurrentRevisionResponse = request(
        STOREFRONT_FORUM_REPLY_CURRENT_REVISION_QUERY,
        StorefrontForumRevisionVariables {
            tenant_id: None,
            id: reply_id,
            locale,
        },
    )
    .await?;
    Ok(response.revision)
}

#[cfg(test)]
mod revision_tests {
    use super::{
        STOREFRONT_FORUM_REPLY_CURRENT_REVISION_QUERY,
        STOREFRONT_FORUM_TOPIC_CURRENT_REVISION_QUERY,
    };

    #[test]
    fn current_revision_queries_stay_generic_forum_owner_facts() {
        for query in [
            STOREFRONT_FORUM_TOPIC_CURRENT_REVISION_QUERY,
            STOREFRONT_FORUM_REPLY_CURRENT_REVISION_QUERY,
        ] {
            assert!(query.contains("CurrentRevision"));
            assert!(!query.to_lowercase().contains("reaction"));
            assert!(!query.contains("actorId"));
        }
    }
}
