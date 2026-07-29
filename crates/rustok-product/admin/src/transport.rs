#![allow(dead_code)]

mod admin_catalog_graphql;
mod admin_catalog_native;
mod graphql_adapter;
mod native_server_adapter;
#[path = "transport/legacy.rs"]
mod legacy;

pub(crate) use legacy::*;

use crate::catalog_controls::build_product_admin_list_input;
use crate::model::ProductList;
use graphql_adapter::ApiError;

#[cfg(target_arch = "wasm32")]
fn browser_query_value(key: &str) -> Option<String> {
    let query = leptos::web_sys::window()?.location().search().ok()?;
    query
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(name, value)| (name == key).then(|| value.to_string()))
        .filter(|value| !value.trim().is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn browser_query_value(_key: &str) -> Option<String> {
    None
}

pub(crate) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, ApiError> {
    let controls = build_product_admin_list_input(
        search,
        status,
        browser_query_value("category_id"),
        browser_query_value("sort_by"),
        browser_query_value("sort_direction"),
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
