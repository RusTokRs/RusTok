use crate::core::{InventoryProductRequest, InventoryProductsRequest};
use crate::model::{InventoryAdminBootstrap, InventoryProductDetail, InventoryProductList};
use crate::transport::{
    CommerceGraphqlInventoryReadAdapter, InventoryReadTransport, InventoryTransportError,
};

pub type ApiError = InventoryTransportError;

fn read_transport() -> impl InventoryReadTransport {
    CommerceGraphqlInventoryReadAdapter
}

fn products_request(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> InventoryProductsRequest {
    InventoryProductsRequest {
        token,
        tenant_slug,
        tenant_id,
        locale,
        search,
        status,
    }
}

fn product_request(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> InventoryProductRequest {
    InventoryProductRequest {
        token,
        tenant_slug,
        tenant_id,
        id,
        locale,
    }
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<InventoryAdminBootstrap, ApiError> {
    read_transport().fetch_bootstrap(token, tenant_slug).await
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<InventoryProductList, ApiError> {
    read_transport()
        .fetch_products(products_request(
            token,
            tenant_slug,
            tenant_id,
            locale,
            search,
            status,
        ))
        .await
}

pub async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<InventoryProductDetail>, ApiError> {
    read_transport()
        .fetch_product(product_request(token, tenant_slug, tenant_id, id, locale))
        .await
}

#[cfg(test)]
mod tests {
    use super::{product_request, products_request};

    #[test]
    fn products_request_preserves_inventory_facade_context() {
        let request = products_request(
            Some("token".to_string()),
            Some("tenant-slug".to_string()),
            "tenant-id".to_string(),
            Some("ru".to_string()),
            Some("coat".to_string()),
            Some("active".to_string()),
        );

        assert_eq!(request.token.as_deref(), Some("token"));
        assert_eq!(request.tenant_slug.as_deref(), Some("tenant-slug"));
        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.locale.as_deref(), Some("ru"));
        assert_eq!(request.search.as_deref(), Some("coat"));
        assert_eq!(request.status.as_deref(), Some("active"));
    }

    #[test]
    fn product_request_preserves_inventory_facade_context() {
        let request = product_request(
            Some("token".to_string()),
            Some("tenant-slug".to_string()),
            "tenant-id".to_string(),
            "product-id".to_string(),
            Some("en".to_string()),
        );

        assert_eq!(request.token.as_deref(), Some("token"));
        assert_eq!(request.tenant_slug.as_deref(), Some("tenant-slug"));
        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.id, "product-id");
        assert_eq!(request.locale.as_deref(), Some("en"));
    }
}
