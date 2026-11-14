use leptos::prelude::*;

use crate::catalog_controls::CatalogListInput;
use crate::core::FetchRequest;
#[cfg(feature = "ssr")]
use crate::model::{ProductImage, ProductListItem, ProductListPrice};
use crate::model::{ProductList, StorefrontProductsData};

use super::native_server_adapter::{self, ApiError};

#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_CATALOG_OPERATION: &str = "storefront_catalog_list";

pub async fn fetch_products(
    request: FetchRequest,
    controls: CatalogListInput,
) -> Result<StorefrontProductsData, ApiError> {
    let products = storefront_catalog_list_native(
        request.locale.clone(),
        controls.search,
        controls.category_id,
        controls.sort_by,
        controls.sort_direction,
        controls.attribute_filters,
        controls.currency_code,
    )
    .await
    .map_err(ApiError::from)?;
    let resolved_handle = request
        .selected_handle
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            products
                .items
                .first()
                .map(|item| item.handle.clone())
                .filter(|value| !value.is_empty())
        });
    let selected_product = if let Some(handle) = resolved_handle.as_ref() {
        native_server_adapter::fetch_product_detail(handle.clone(), request.locale).await?
    } else {
        None
    };
    Ok(StorefrontProductsData {
        products,
        selected_product,
        selected_handle: resolved_handle,
    })
}

#[cfg(feature = "ssr")]
fn map_product_service_error(
    error: rustok_product::CommerceError,
    operation: &'static str,
) -> ServerFnError {
    ServerFnError::new(
        rustok_product::map_product_public_error(
            &error,
            operation,
            "product_storefront_catalog_list_native",
        )
        .to_string(),
    )
}

#[cfg(feature = "ssr")]
fn map_product_list(value: rustok_product::StorefrontProductList) -> ProductList {
    ProductList {
        items: value
            .items
            .into_iter()
            .map(|item| ProductListItem {
                id: item.id.to_string(),
                status: item.status.to_string(),
                title: item.title,
                handle: item.handle,
                seller_id: item.seller_id,
                vendor: item.vendor,
                product_type: item.product_type,
                tags: item.tags,
                primary_image: item.primary_image.map(map_owner_product_image),
                price_from: item.price_from.map(map_owner_product_list_price),
                created_at: item.created_at.to_rfc3339(),
                published_at: item.published_at.map(|value| value.to_rfc3339()),
            })
            .collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[server(prefix = "/api/fn", endpoint = "product/storefront/catalog-list", client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient)]
async fn storefront_catalog_list_native(
    locale: Option<String>,
    search: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Vec<String>,
    currency_code: Option<String>,
) -> Result<ProductList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_product::StorefrontProductListQuery;

        let scope =
            native_server_adapter::product_buyer_scope(PRODUCT_STOREFRONT_CATALOG_OPERATION)
                .await?;
        let requested_locale = crate::core::resolve_requested_locale(
            locale,
            Some(&scope.request.locale),
            &scope.tenant.default_locale,
        );
        let public_channel_slug = scope.request.channel_slug.clone();
        let list_query = StorefrontProductListQuery::try_from_transport_with_attribute_filters(
            search,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "storefront_catalog_list_input"))?
        .with_pagination(1, 12)
        .with_currency_code(currency_code)
        .map_err(|error| map_product_service_error(error, "storefront_catalog_list_input"))?;
        let products = native_server_adapter::product_read_call(
            &scope,
            native_server_adapter::product_read_port(&scope).list_filtered_published_products(
                native_server_adapter::product_read_context(
                    &scope,
                    requested_locale.clone(),
                    public_channel_slug.clone(),
                ),
                rustok_product::FilteredPublishedProductsRequest {
                    locale: Some(requested_locale),
                    fallback_locale: Some(scope.tenant.default_locale.clone()),
                    public_channel_slug,
                    query: list_query,
                },
            ),
        )
        .await
        .map_err(native_server_adapter::map_product_port_error)?;

        Ok(map_product_list(products))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            locale,
            search,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
            currency_code,
        );
        Err(ServerFnError::new(
            "product/storefront/catalog-list requires the `ssr` feature",
        ))
    }
}

/// Maps the owner-resolved catalog-card image into the storefront model.
#[cfg(feature = "ssr")]
pub(crate) fn map_owner_product_image(
    value: rustok_product::StorefrontProductListImage,
) -> ProductImage {
    ProductImage {
        media_id: value.media_id.to_string(),
        url: value.url,
        alt_text: value.alt_text,
        position: value.position,
    }
}

/// Maps the owner-resolved catalog-card price snapshot into the storefront model.
#[cfg(feature = "ssr")]
pub(crate) fn map_owner_product_list_price(
    value: rustok_product::StorefrontProductListPrice,
) -> ProductListPrice {
    ProductListPrice {
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        compare_at_amount: value
            .compare_at_amount
            .map(|compare_at| compare_at.normalize().to_string()),
        on_sale: value.on_sale,
    }
}
