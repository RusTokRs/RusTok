use crate::core::{InventoryProductRequest, InventoryProductsRequest};
use crate::model::{InventoryAdminBootstrap, InventoryProductDetail, InventoryProductList};
use crate::transport::{
    CommerceGraphqlInventoryReadAdapter, InventoryReadTransport, InventoryTransportError,
};

pub type ApiError = InventoryTransportError;

fn read_transport() -> impl InventoryReadTransport {
    CommerceGraphqlInventoryReadAdapter
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
        .fetch_products(InventoryProductsRequest {
            token,
            tenant_slug,
            tenant_id,
            locale,
            search,
            status,
        })
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
        .fetch_product(InventoryProductRequest {
            token,
            tenant_slug,
            tenant_id,
            id,
            locale,
        })
        .await
}
