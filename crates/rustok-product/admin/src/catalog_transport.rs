#![allow(dead_code)]

#[path = "transport.rs"]
mod legacy;
#[path = "transport/admin_catalog_graphql.rs"]
mod admin_catalog_graphql;
#[path = "transport/admin_catalog_native.rs"]
mod admin_catalog_native;

pub(crate) use legacy::*;

use crate::catalog_controls::{ProductAdminListInput, build_product_admin_list_input};
use crate::model::{ProductCatalogSearchOptions, ProductList};

const PRODUCT_ADMIN_CATALOG_OPTIONS_OWNER: &str = "rustok_product.admin";
const PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION: &str = "fetch_catalog_search_options";
const PRODUCT_ADMIN_CATALOG_OPTIONS_BOUNDARY: &str =
    "product_admin_catalog_search_options_graphql_fallback";
const PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE: &str =
    "Product catalog search options are temporarily unavailable";

struct CatalogSearchOptionsErrorContext {
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    locale_length: usize,
}

impl CatalogSearchOptionsErrorContext {
    fn new(token: Option<&str>, tenant_slug: Option<&str>, locale: &str) -> Self {
        Self {
            correlation_id: format!(
                "product-admin-catalog-options:{PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION}:{}",
                uuid::Uuid::new_v4()
            ),
            token_present: token.is_some(),
            tenant_slug_length: tenant_slug.map(|value| value.chars().count()),
            locale_length: locale.chars().count(),
        }
    }

    fn map_error(&self, raw_error: String) -> String {
        tracing::error!(
            raw_error = %raw_error,
            owner = PRODUCT_ADMIN_CATALOG_OPTIONS_OWNER,
            owner_operation = PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION,
            correlation_id = %self.correlation_id,
            token_present = self.token_present,
            tenant_slug_present = self.tenant_slug_length.is_some(),
            tenant_slug_length = ?self.tenant_slug_length,
            locale_length = self.locale_length,
            code = "product.admin_catalog_search_options_graphql_unavailable",
            boundary = PRODUCT_ADMIN_CATALOG_OPTIONS_BOUNDARY,
            "product admin catalog search options GraphQL fallback failed"
        );

        PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE.to_string()
    }
}

pub async fn fetch_catalog_search_options(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<ProductCatalogSearchOptions, String> {
    let context = CatalogSearchOptionsErrorContext::new(
        token.as_deref(),
        tenant_slug.as_deref(),
        locale.as_str(),
    );

    legacy::fetch_catalog_search_options(token, tenant_slug, locale)
        .await
        .map_err(|error| context.map_error(error))
}

pub(crate) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, rustok_graphql::GraphqlHttpError> {
    let route_controls = leptos::prelude::use_context::<ProductAdminListInput>().unwrap_or_default();
    let attribute_filters = if route_controls.attribute_filters.is_empty() {
        None
    } else {
        Some(route_controls.attribute_filters.join(";"))
    };
    let controls = build_product_admin_list_input(
        search,
        status,
        route_controls.category_id,
        route_controls.sort_by,
        route_controls.sort_direction,
        attribute_filters,
    );
    let native_controls = controls.clone();
    match admin_catalog_native::fetch_products(
        tenant_id.clone(),
        locale.clone(),
        native_controls,
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            admin_catalog_graphql::fetch_products(
                token,
                tenant_slug,
                tenant_id,
                locale,
                controls,
            )
            .await
        }
    }
}
