use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;
use rustok_product_admin::fetch_catalog_search_options;
use rustok_search_admin::{SearchAdmin, SearchCatalogFilterOption};

use crate::app::providers::enabled_modules::use_is_module_enabled;

#[component]
pub fn SearchAdminComposition() -> impl IntoView {
    let token = use_token();
    let tenant_slug = use_tenant();
    let product_enabled = use_is_module_enabled("product");
    let locale = use_context::<UiRouteContext>()
        .and_then(|context| context.locale)
        .unwrap_or_default();
    let catalog_options = LocalResource::new(
        move || {
            (
                product_enabled.get(),
                token.get(),
                tenant_slug.get(),
                locale.clone(),
            )
        },
        move |(product_enabled, token, tenant_slug, locale)| async move {
            if !product_enabled || locale.trim().is_empty() {
                return Ok(Default::default());
            }

            fetch_catalog_search_options(token, tenant_slug, locale).await
        },
    );

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
                <SearchAdmin
                    category_options=category_options
                    attribute_options=attribute_options
                />
            }
        }}
    }
}
