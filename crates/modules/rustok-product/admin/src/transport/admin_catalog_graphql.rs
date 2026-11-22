use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::catalog_controls::ProductAdminListInput;
use crate::model::{AdminCatalogFacet, ProductList};

/// Admin facet counts: the same admin filter set as the list, deliberately without pagination
/// because the owner counts the whole filtered catalog.
const ADMIN_PRODUCT_CATALOG_FACETS_QUERY: &str = "query ProductAdminCatalogFacets($tenantId: UUID!, $locale: String, $filter: AdminProductCatalogFilter, $facetCodes: [String!]!) { adminProductCatalogFacets(tenantId: $tenantId, locale: $locale, filter: $filter, facetCodes: $facetCodes) { code label valueType isLocalized isEnumerable isTruncated totalProducts values { value label count } } }";

const ADMIN_PRODUCT_CATALOG_QUERY: &str = "query ProductAdminCatalog($tenantId: UUID!, $locale: String, $filter: AdminProductCatalogFilter) { adminProductCatalog(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt publishedAt } } }";

#[derive(Debug, Deserialize)]
struct AdminProductCatalogResponse {
    #[serde(rename = "adminProductCatalog")]
    admin_product_catalog: ProductList,
}

#[derive(Debug, Serialize)]
struct AdminProductCatalogVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    locale: Option<String>,
    filter: AdminProductCatalogFilter,
}

#[derive(Debug, Deserialize)]
struct AdminProductCatalogFacetsResponse {
    #[serde(rename = "adminProductCatalogFacets")]
    admin_product_catalog_facets: Vec<AdminCatalogFacet>,
}

#[derive(Debug, Serialize)]
struct AdminProductCatalogFacetsVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    locale: Option<String>,
    filter: AdminProductCatalogFilter,
    #[serde(rename = "facetCodes")]
    facet_codes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AdminProductCatalogFilter {
    search: Option<String>,
    status: Option<String>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    #[serde(rename = "sortBy")]
    sort_by: Option<String>,
    #[serde(rename = "sortDirection")]
    sort_direction: Option<String>,
    #[serde(rename = "attributeFilters")]
    attribute_filters: Vec<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

fn build_admin_product_catalog_filter(
    controls: ProductAdminListInput,
    paginated: bool,
) -> AdminProductCatalogFilter {
    let (page, per_page) = if paginated {
        (Some(controls.page), Some(controls.per_page))
    } else {
        (None, None)
    };

    AdminProductCatalogFilter {
        search: controls.search,
        status: controls.status,
        category_id: controls.category_id,
        sort_by: controls.sort_by,
        sort_direction: controls.sort_direction,
        attribute_filters: controls.attribute_filters,
        page,
        per_page,
    }
}

pub(crate) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
) -> Result<ProductList, GraphqlHttpError> {
    let response: AdminProductCatalogResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            ADMIN_PRODUCT_CATALOG_QUERY,
            Some(AdminProductCatalogVariables {
                tenant_id,
                locale,
                filter: build_admin_product_catalog_filter(controls, true),
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await?;
    Ok(response.admin_product_catalog)
}

pub(crate) async fn fetch_facets(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
    facet_codes: Vec<String>,
) -> Result<Vec<AdminCatalogFacet>, GraphqlHttpError> {
    let response: AdminProductCatalogFacetsResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            ADMIN_PRODUCT_CATALOG_FACETS_QUERY,
            Some(AdminProductCatalogFacetsVariables {
                tenant_id,
                locale,
                filter: build_admin_product_catalog_filter(controls, false),
                facet_codes,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await?;
    Ok(response.admin_product_catalog_facets)
}

#[cfg(test)]
mod tests {
    use super::build_admin_product_catalog_filter;
    use crate::catalog_controls::ProductAdminListInput;

    #[test]
    fn admin_catalog_graphql_forwards_requested_server_page() {
        let filter = build_admin_product_catalog_filter(
            ProductAdminListInput {
                page: 3,
                per_page: 50,
                ..ProductAdminListInput::default()
            },
            true,
        );

        assert_eq!(filter.page, Some(3));
        assert_eq!(filter.per_page, Some(50));
    }

    #[test]
    fn admin_catalog_facets_are_not_limited_to_the_current_page() {
        let filter = build_admin_product_catalog_filter(ProductAdminListInput::default(), false);

        assert_eq!(filter.page, None);
        assert_eq!(filter.per_page, None);
    }
}
