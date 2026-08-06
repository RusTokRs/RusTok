use crate::model::StorefrontForumCategoryRouteResolution;

const STOREFRONT_FORUM_CATEGORY_ROUTE_QUERY: &str = "query StorefrontForumCategoryRoute($tenantId: UUID, $locale: String!, $slug: String!) { forumStorefrontCategoryRoute(tenantId: $tenantId, locale: $locale, slug: $slug) { requestedLocale requestedSlug disposition canonical { categoryId locale slug path } } }";

#[derive(Debug, Deserialize)]
struct StorefrontForumCategoryRouteResponse {
    #[serde(rename = "forumStorefrontCategoryRoute")]
    forum_storefront_category_route: Option<StorefrontForumCategoryRouteResolution>,
}

#[derive(Debug, Serialize)]
struct StorefrontForumCategoryRouteVariables {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    locale: String,
    slug: String,
}

pub async fn resolve_storefront_category_route_graphql(
    locale: String,
    slug: String,
) -> Result<Option<StorefrontForumCategoryRouteResolution>, ApiError> {
    let response: StorefrontForumCategoryRouteResponse = request(
        STOREFRONT_FORUM_CATEGORY_ROUTE_QUERY,
        StorefrontForumCategoryRouteVariables {
            tenant_id: None,
            locale,
            slug,
        },
    )
    .await?;
    Ok(response.forum_storefront_category_route)
}
