use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::read_route_query_value;
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::catalog_controls::{
    build_product_admin_catalog_controls_labels, build_product_admin_list_input,
    serialize_attribute_filters,
};
use crate::transport;

#[component]
pub fn ProductAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let token = use_token();
    let tenant = use_tenant();
    let selected_product_id =
        read_route_query_value(&route_context, AdminQueryKey::ProductId.as_str());
    let catalog_controls = build_product_admin_list_input(
        None,
        None,
        read_route_query_value(&route_context, "category_id"),
        read_route_query_value(&route_context, "sort_by"),
        read_route_query_value(&route_context, "sort_direction"),
        read_route_query_value(&route_context, "attribute_filters"),
    );
    let current_category = catalog_controls.category_id.clone().unwrap_or_default();
    let current_attribute_filters =
        serialize_attribute_filters(catalog_controls.attribute_filters.as_slice());
    let current_sort_by = catalog_controls.sort_by.clone().unwrap_or_default();
    let current_sort_direction = catalog_controls.sort_direction.clone().unwrap_or_default();
    provide_context(catalog_controls);

    let labels = build_product_admin_catalog_controls_labels(locale.as_deref());
    let options_locale = locale.clone().unwrap_or_default();
    let catalog_options = LocalResource::new(move || {
        let token = token.get();
        let tenant = tenant.get();
        let locale = options_locale.clone();
        async move { transport::fetch_catalog_search_options(token, tenant, locale).await }
    });

    view! {
        <section class="space-y-6">
            <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h3 class="text-lg font-semibold text-card-foreground">{labels.title}</h3>
                    <p class="text-sm text-muted-foreground">{labels.subtitle}</p>
                </div>
                <form method="get" class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)_minmax(0,1fr)_auto] xl:items-end">
                    {selected_product_id.map(|value| view! {
                        <input type="hidden" name=AdminQueryKey::ProductId.as_str() value=value />
                    })}
                    <label class="grid gap-2 text-sm text-foreground">
                        <span class="font-medium">{labels.category}</span>
                        <select
                            name="category_id"
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            prop:value=current_category
                        >
                            <option value="">{labels.all_categories}</option>
                            {move || catalog_options
                                .get()
                                .and_then(Result::ok)
                                .map(|options| options.category_options.into_iter().map(|option| {
                                    view! { <option value=option.value>{option.label}</option> }
                                }).collect_view())
                                .unwrap_or_default()}
                        </select>
                    </label>
                    <label class="grid gap-2 text-sm text-foreground">
                        <span class="font-medium">{labels.attribute_filters}</span>
                        <input
                            name="attribute_filters"
                            type="text"
                            value=current_attribute_filters
                            placeholder=labels.attribute_filters_placeholder
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                        />
                        <span class="text-xs text-muted-foreground">{labels.attribute_filters_help}</span>
                    </label>
                    <label class="grid gap-2 text-sm text-foreground">
                        <span class="font-medium">{labels.sort_by}</span>
                        <select
                            name="sort_by"
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            prop:value=current_sort_by
                        >
                            <option value="published_at">{labels.published_at}</option>
                            <option value="created_at">{labels.created_at}</option>
                        </select>
                    </label>
                    <label class="grid gap-2 text-sm text-foreground">
                        <span class="font-medium">{labels.sort_direction}</span>
                        <select
                            name="sort_direction"
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            prop:value=current_sort_direction
                        >
                            <option value="desc">{labels.descending}</option>
                            <option value="asc">{labels.ascending}</option>
                        </select>
                    </label>
                    <button type="submit" class="inline-flex h-10 items-center justify-center rounded-xl bg-primary px-4 text-sm font-medium text-primary-foreground transition hover:bg-primary/90">
                        {labels.apply}
                    </button>
                </form>
            </section>
            <super::leptos::ProductAdmin />
        </section>
    }
}
