#![allow(dead_code)]

#[path = "transport.rs"]
mod legacy;
#[path = "transport/admin_catalog_graphql.rs"]
mod admin_catalog_graphql;
#[path = "transport/admin_catalog_native.rs"]
mod admin_catalog_native;

pub use legacy::fetch_catalog_search_options;
pub(crate) use legacy::*;

use crate::catalog_controls::{ProductAdminListInput, build_product_admin_list_input};
use crate::model::ProductList;

pub(crate) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, rustok_graphql::GraphqlHttpError> {
    let route_controls = leptos::prelude::use_context::<ProductAdminListInput>().unwrap_or_default();
    let controls = build_product_admin_list_input(
        search,
        status,
        route_controls.category_id,
        route_controls.sort_by,
        route_controls.sort_direction,
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
