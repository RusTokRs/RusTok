use crate::model::StorefrontForumTopicRouteResolution;

const STOREFRONT_FORUM_TOPIC_ROUTE_QUERY: &str = "query StorefrontForumTopicRouteDecision($tenantId: UUID, $locale: String!, $shortId: String!, $slug: String!) { forumStorefrontTopicRouteDecision(tenantId: $tenantId, locale: $locale, shortId: $shortId, slug: $slug) { requestedLocale requestedShortId requestedSlug disposition canonical { topicId locale shortId slug path } } }";

#[derive(Debug, Deserialize)]
struct StorefrontForumTopicRouteResponse {
    #[serde(rename = "forumStorefrontTopicRouteDecision")]
    forum_storefront_topic_route_decision: Option<StorefrontForumTopicRouteResolution>,
}

#[derive(Debug, Serialize)]
struct StorefrontForumTopicRouteVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    locale: String,
    #[serde(rename = "shortId")]
    short_id: String,
    slug: String,
}

pub async fn resolve_storefront_topic_route_graphql(
    locale: String,
    short_id: String,
    slug: String,
) -> Result<Option<StorefrontForumTopicRouteResolution>, ApiError> {
    let response: StorefrontForumTopicRouteResponse = request(
        STOREFRONT_FORUM_TOPIC_ROUTE_QUERY,
        StorefrontForumTopicRouteVariables {
            tenant_id: None,
            locale,
            short_id,
            slug,
        },
    )
    .await?;
    Ok(response.forum_storefront_topic_route_decision)
}
