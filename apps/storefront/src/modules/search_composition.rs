use leptos::prelude::*;
use rustok_api::UiRouteContext;
use rustok_product_storefront::fetch_catalog_search_options;
use rustok_search_storefront::{SearchCatalogFilterOption, SearchView};

use crate::shared::context::enabled_modules::use_is_module_enabled;

#[component]
pub fn SearchStorefrontComposition() -> impl IntoView {
    let product_enabled = use_is_module_enabled("product");
    let locale = use_context::<UiRouteContext>()
        .and_then(|context| context.locale)
        .unwrap_or_default();
    let catalog_options = LocalResource::new({
        let locale = locale.clone();
        move || {
            let product_enabled = product_enabled.get();
            let locale = locale.clone();
            async move {
                if !product_enabled || locale.trim().is_empty() {
                    return Ok(Default::default());
                }

                fetch_catalog_search_options(locale).await
            }
        }
    });

    view! {
        {move || {
            let options = catalog_options
                .get()
                .and_then(Result::ok)
                .unwrap_or_default();
            let category_options = options
                .category_options
                .into_iter()
                .map(|option| SearchCatalogFilterOption {
                    value: option.value,
                    label: option.label,
                })
                .collect();
            let attribute_options = options
                .attribute_options
                .into_iter()
                .map(|option| SearchCatalogFilterOption {
                    value: option.value,
                    label: option.label,
                })
                .collect();

            view! {
                <SearchView
                    category_options=category_options
                    attribute_options=attribute_options
                />
            }
        }}
    }
}
