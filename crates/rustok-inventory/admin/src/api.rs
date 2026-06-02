use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::core::{InventoryProductRequest, InventoryProductsRequest};
use crate::model::{InventoryAdminBootstrap, InventoryProductDetail, InventoryProductList};
use crate::transport::{
    CommerceGraphqlInventoryReadAdapter, InventoryReadTransport, InventoryTransportError,
};

#[derive(Debug, Clone)]
pub enum ApiError {
    ServerFn(String),
    Transport(InventoryTransportError),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

impl From<InventoryTransportError> for ApiError {
    fn from(value: InventoryTransportError) -> Self {
        Self::Transport(value)
    }
}

fn transitional_read_transport() -> impl InventoryReadTransport {
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
    match crate::native::fetch_bootstrap().await {
        Ok(value) => Ok(value),
        Err(err) if cfg!(feature = "ssr") => Err(err.into()),
        Err(_) => transitional_read_transport()
            .fetch_bootstrap(token, tenant_slug)
            .await
            .map_err(Into::into),
    }
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<InventoryProductList, ApiError> {
    match crate::native::fetch_products(
        tenant_id.clone(),
        locale.clone(),
        search.clone(),
        status.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(err) if cfg!(feature = "ssr") => Err(err.into()),
        Err(_) => transitional_read_transport()
            .fetch_products(products_request(
                token,
                tenant_slug,
                tenant_id,
                locale,
                search,
                status,
            ))
            .await
            .map_err(Into::into),
    }
}

pub async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<InventoryProductDetail>, ApiError> {
    match crate::native::fetch_product(tenant_id.clone(), id.clone(), locale.clone()).await {
        Ok(value) => Ok(value),
        Err(err) if cfg!(feature = "ssr") => Err(err.into()),
        Err(_) => transitional_read_transport()
            .fetch_product(product_request(token, tenant_slug, tenant_id, id, locale))
            .await
            .map_err(Into::into),
    }
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
            Some("en".to_string()),
            Some("boots".to_string()),
            Some("ACTIVE".to_string()),
        );

        assert_eq!(request.token.as_deref(), Some("token"));
        assert_eq!(request.tenant_slug.as_deref(), Some("tenant-slug"));
        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.locale.as_deref(), Some("en"));
        assert_eq!(request.search.as_deref(), Some("boots"));
        assert_eq!(request.status.as_deref(), Some("ACTIVE"));
    }

    #[test]
    fn product_request_preserves_inventory_facade_context() {
        let request = product_request(
            Some("token".to_string()),
            Some("tenant-slug".to_string()),
            "tenant-id".to_string(),
            "product-id".to_string(),
            Some("de".to_string()),
        );

        assert_eq!(request.token.as_deref(), Some("token"));
        assert_eq!(request.tenant_slug.as_deref(), Some("tenant-slug"));
        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.id, "product-id");
        assert_eq!(request.locale.as_deref(), Some("de"));
    }
}
