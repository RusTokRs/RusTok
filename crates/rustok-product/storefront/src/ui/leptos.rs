use crate::catalog_controls::{
    build_catalog_list_input, build_catalog_search_labels, serialize_attribute_filters,
};
use crate::core::{
    build_catalog_rail_view_model, build_fetch_request, build_product_catalog_rail_labels,
    build_route_input, build_selected_product_empty_view_model, build_selected_product_view_model,
    build_shell_view_model, build_transport_error_dom_evidence, resolve_route_segment,
};
use crate::model::{
    ProductCatalogSearchOptions, ProductDetail, ProductListItem, ProductPricingContext,
    ProductPricingDetail, StorefrontProductsData,
};
use crate::transport;
use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn ProductView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let route_input = build_route_input(
        read_route_query_value(&route_context, "handle"),
        route_context.locale.clone(),
        read_route_query_value(&route_context, "currency"),
        read_route_query_value(&route_context, "region_id"),
        read_route_query_value(&route_context, "price_list_id"),
        read_route_query_value(&route_context, "channel_id"),
        read_route_query_value(&route_context, "channel_slug"),
        read_route_query_value(&route_context, "quantity"),
    );
    let catalog_input = build_catalog_list_input(
        read_route_query_value(&route_context, "search"),
        read_route_query_value(&route_context, "category_id"),
        read_route_query_value(&route_context, "sort_by"),
        read_route_query_value(&route_context, "sort_direction"),
        read_route_query_value(&route_context, "attribute_filters"),
    );
    let control_labels = build_catalog_search_labels(route_input.locale.as_deref());
    let current_search = catalog_input.search.clone().unwrap_or_default();
    let current_category_id = catalog_input.category_id.clone().unwrap_or_default();
    let current_attribute_filters =
        serialize_attribute_filters(catalog_input.attribute_filters.as_slice());
    let current_sort_by = catalog_input
        .sort_by
        .clone()
        .unwrap_or_else(|| "published_at".to_string());
    let current_sort_direction = catalog_input
        .sort_direction
        .clone()
        .unwrap_or_else(|| "desc".to_string());
    let options_locale = route_input.locale.clone().unwrap_or_default();
    let options_resource = Resource::new_blocking(
        move || options_locale.clone(),
        move |locale| async move {
            transport::fetch_catalog_search_options(locale)
                .await
                .unwrap_or_else(|_| ProductCatalogSearchOptions::default())
        },
    );
    let search_label = control_labels.search_label;
    let search_placeholder = control_labels.search_placeholder;
    let category_label = control_labels.category_label;
    let all_categories = control_labels.all_categories;
    let attribute_filters_label = control_labels.attribute_filters_label;
    let attribute_filters_placeholder = control_labels.attribute_filters_placeholder;
    let attribute_filters_help = control_labels.attribute_filters_help;
    let sort_by_label = control_labels.sort_by_label;
    let sort_by_published_at = control_labels.sort_by_published_at;
    let sort_by_created_at = control_labels.sort_by_created_at;
    let sort_direction_label = control_labels.sort_direction_label;
    let sort_direction_desc = control_labels.sort_direction_desc;
    let sort_direction_asc = control_labels.sort_direction_asc;
    let submit_label = control_labels.submit;
    let category_fallback_value = current_category_id.clone();
    let category_fallback_label = all_categories.clone();
    let category_options_selected = current_category_id.clone();
    let category_options_all_label = all_categories.clone();
    let currency = route_input.currency_code.clone();
    let region_id = route_input.region_id.clone();
    let price_list_id = route_input.price_list_id.clone();
    let channel_id = route_input.channel_id.clone();
    let channel_slug = route_input.channel_slug.clone();
    let quantity = route_input.quantity.map(|value| value.to_string());
    let shell = build_shell_view_model(route_input.locale.as_deref());
    let fetch_request = build_fetch_request(&route_input);

    let resource = Resource::new_blocking(
        move || (fetch_request.clone(), catalog_input.clone()),
        move |(request, controls)| async move {
            transport::fetch_products(request, controls).await
        },
    );

    view! {
        <section class="rounded-[2rem] border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{shell.badge}</span>
                <h2 class="text-3xl font-semibold text-card-foreground">{shell.title}</h2>
                <p class="text-sm text-muted-foreground">{shell.subtitle}</p>
            </div>
            <form method="get" class="mt-6 grid gap-3 rounded-2xl border border-border bg-background p-4 md:grid-cols-2 xl:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_auto] xl:items-end">
                <div class="min-w-0 space-y-2">
                    <label for="product-catalog-search" class="text-sm font-medium text-foreground">
                        {search_label}
                    </label>
                    <input
                        id="product-catalog-search"
                        name="search"
                        type="search"
                        value=current_search
                        placeholder=search_placeholder
                        class="h-10 w-full rounded-lg border border-input bg-background px-3 text-sm text-foreground outline-none transition focus:border-ring focus:ring-2 focus:ring-ring/20"
                    />
                </div>
                <div class="min-w-0 space-y-2">
                    <label for="product-catalog-category" class="text-sm font-medium text-foreground">
                        {category_label}
                    </label>
                    <Suspense fallback=move || view! {
                        <select
                            id="product-catalog-category"
                            name="category_id"
                            class="h-10 w-full rounded-lg border border-input bg-background px-3 text-sm text-foreground"
                        >
                            <option value=category_fallback_value.clone()>{category_fallback_label.clone()}</option>
                        </select>
                    }>
                        {move || {
                            let options_resource = options_resource;
                            let selected_category_id = category_options_selected.clone();
                            let all_categories = category_options_all_label.clone();
                            Suspend::new(async move {
                                let options = options_resource.await;
                                view! {
                                    <select
                                        id="product-catalog-category"
                                        name="category_id"
                                        class="h-10 w-full rounded-lg border border-input bg-background px-3 text-sm text-foreground outline-none transition focus:border-ring focus:ring-2 focus:ring-ring/20"
                                    >
                                        <option value="" selected=selected_category_id.is_empty()>{all_categories}</option>
                                        {options.category_options.into_iter().map(|option| {
                                            let selected = option.value == selected_category_id;
                                            view! {
                                                <option value=option.value selected=selected>{option.label}</option>
                                            }
                                        }).collect_view()}
                                    </select>
                                }
                            })
                        }}
                    </Suspense>
                </div>
                <div class="min-w-0 space-y-2">
                    <label for="product-catalog-attribute-filters" class="text-sm font-medium text-foreground">
                        {attribute_filters_label}
                    </label>
                    <input
                        id="product-catalog-attribute-filters"
                        name="attribute_filters"
                        type="text"
                        value=current_attribute_filters
                        placeholder=attribute_filters_placeholder
                        class="h-10 w-full rounded-lg border border-input bg-background px-3 text-sm text-foreground outline-none transition focus:border-ring focus:ring-2 focus:ring-ring/20"
                    />
                    <p class="text-xs text-muted-foreground">{attribute_filters_help}</p>
                </div>
                <div class="min-w-0 space-y-2">
                    <label for="product-catalog-sort-by" class="text-sm font-medium text-foreground">
                        {sort_by_label}
                    </label>
                    <select
                        id="product-catalog-sort-by"
                        name="sort_by"
                        class="h-10 w-full rounded-lg border border-input bg-background px-3 text-sm text-foreground outline-none transition focus:border-ring focus:ring-2 focus:ring-ring/20"
                    >
                        <option value="published_at" selected=current_sort_by == "published_at">{sort_by_published_at}</option>
                        <option value="created_at" selected=current_sort_by == "created_at">{sort_by_created_at}</option>
                    </select>
                </div>
                <div class="min-w-0 space-y-2">
                    <label for="product-catalog-sort-direction" class="text-sm font-medium text-foreground">
                        {sort_direction_label}
                    </label>
                    <select
                        id="product-catalog-sort-direction"
                        name="sort_direction"
                        class="h-10 w-full rounded-lg border border-input bg-background px-3 text-sm text-foreground outline-none transition focus:border-ring focus:ring-2 focus:ring-ring/20"
                    >
                        <option value="desc" selected=current_sort_direction == "desc">{sort_direction_desc}</option>
                        <option value="asc" selected=current_sort_direction == "asc">{sort_direction_asc}</option>
                    </select>
                </div>
                {currency.map(|value| view! { <input type="hidden" name="currency" value=value /> })}
                {region_id.map(|value| view! { <input type="hidden" name="region_id" value=value /> })}
                {price_list_id.map(|value| view! { <input type="hidden" name="price_list_id" value=value /> })}
                {channel_id.map(|value| view! { <input type="hidden" name="channel_id" value=value /> })}
                {channel_slug.map(|value| view! { <input type="hidden" name="channel_slug" value=value /> })}
                {quantity.map(|value| view! { <input type="hidden" name="quantity" value=value /> })}
                <button
                    type="submit"
                    class="inline-flex h-10 items-center justify-center rounded-lg bg-primary px-4 text-sm font-medium text-primary-foreground transition hover:bg-primary/90"
                >
                    {submit_label}
                </button>
            </form>
            <div class="mt-8">
                <Suspense fallback=|| view! { <div class="space-y-4"><div class="h-48 animate-pulse rounded-3xl bg-muted"></div><div class="grid gap-3 md:grid-cols-3"><div class="h-28 animate-pulse rounded-2xl bg-muted"></div><div class="h-28 animate-pulse rounded-2xl bg-muted"></div><div class="h-28 animate-pulse rounded-2xl bg-muted"></div></div></div> }>
                    {move || {
                        let resource = resource;
                        let load_error = shell.load_error.clone();
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! { <ProductShowcase data /> }.into_any(),
                                Err(err) => view! { <ProductTransportErrorMessage context=load_error error=err /> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn ProductTransportErrorMessage(
    context: String,
    error: transport::ProductTransportError,
) -> impl IntoView {
    let evidence = build_transport_error_dom_evidence(
        &context,
        error.failed_path.as_str(),
        error.fallback_attempted,
        error.native_error.as_deref(),
        error.graphql_error.as_deref(),
        error.to_string().as_str(),
    );

    view! {
        <div
            class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
            data-product-transport-failed-path=evidence.failed_path
            data-product-transport-fallback-attempted=evidence.fallback_attempted
            data-product-transport-native-error=evidence.native_error
            data-product-transport-graphql-error=evidence.graphql_error
        >
            {evidence.message}
        </div>
    }
}

#[component]
fn ProductShowcase(data: StorefrontProductsData) -> impl IntoView {
    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
            <SelectedProductCard
                product=data.selected_product
                pricing=data.selected_pricing
                resolution_context=data.resolution_context
                selected_handle=data.selected_handle
            />
            <CatalogRail items=data.products.items total=data.products.total />
        </div>
    }
}

#[component]
fn SelectedProductCard(
    product: Option<ProductDetail>,
    pricing: Option<ProductPricingDetail>,
    resolution_context: Option<ProductPricingContext>,
    selected_handle: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let Some(product) = product else {
        let view_model = build_selected_product_empty_view_model(locale.as_deref());
        return view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {view_model.title}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {view_model.body}
                </p>
            </article>
        }
        .into_any();
    };

    let pricing_route_base = route_context.module_route_base("pricing");
    let view_model = build_selected_product_view_model(
        &product,
        pricing.as_ref(),
        resolution_context.as_ref(),
        selected_handle.as_deref(),
        locale.as_deref(),
        pricing_route_base.as_str(),
    );

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                {view_model.metadata_items.into_iter().map(|item| view! {
                    <span>{item}</span>
                }).collect_view()}
            </div>
            <p class="mt-3 text-xs font-medium text-muted-foreground">{view_model.seller_boundary}</p>
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{view_model.title}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">{view_model.description}</p>
            {view_model.pricing_context.as_ref().map(|pricing_context| view! {
                <div class="mt-4 inline-flex flex-wrap items-center gap-2 rounded-2xl border border-primary/20 bg-primary/5 px-4 py-2 text-xs text-primary">
                    <span class="font-semibold uppercase tracking-[0.16em]">
                        {view_model.preview_context_label.clone()}
                    </span>
                    <span>{pricing_context.clone()}</span>
                </div>
            })}
            <p class="mt-4 text-xs text-muted-foreground">
                {view_model.pricing_ownership_note.clone()}
            </p>
            <div class="mt-6 grid gap-3 md:grid-cols-3">
                <MetricCard title=view_model.catalog_snapshot_label.clone() value=view_model.catalog_snapshot />
                <MetricCard title=view_model.pricing_preview_label.clone() value=view_model.pricing_preview />
                <MetricCard title=view_model.inventory_label.clone() value=view_model.inventory.to_string() />
            </div>
            <div class="mt-4">
                <a
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                    href=view_model.pricing_href
                >
                    {view_model.open_pricing_label}
                </a>
            </div>
        </article>
    }.into_any()
}

#[component]
fn CatalogRail(items: Vec<ProductListItem>, total: u64) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = resolve_route_segment(route_context.route_segment.as_deref());
    let module_route_base = route_context.module_route_base(route_segment.as_str());
    let view_model = build_catalog_rail_view_model(
        module_route_base.as_str(),
        &items,
        total,
        locale.as_deref(),
        build_product_catalog_rail_labels(locale.as_deref()),
    );

    if view_model.show_empty_state {
        return view! { <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{view_model.empty_message}</article> }.into_any();
    }

    let open_label = view_model.open_label.clone();

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{view_model.title.clone()}</h3>
                <span class="text-sm text-muted-foreground">
                    {view_model.total_label.clone()}
                </span>
            </div>
            <div class="space-y-3">
                {view_model.items.into_iter().map(|product| {
                    let open_label = open_label.clone();
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{product.product_type}</div>
                            <h4 class="mt-2 text-base font-semibold text-card-foreground">{product.title}</h4>
                            <p class="mt-2 text-sm text-muted-foreground">{product.vendor}</p>
                            <p class="mt-1 text-xs text-muted-foreground">{product.seller_boundary}</p>
                            <div class="mt-4 flex items-center justify-between gap-3">
                                <span class="text-xs text-muted-foreground">{product.published_at}</span>
                                <a class="inline-flex text-sm font-medium text-primary hover:underline" href=product.href>{open_label}</a>
                            </div>
                        </article>
                    }
                }).collect_view()}
            </div>
        </div>
    }.into_any()
}

#[component]
fn MetricCard(title: String, value: String) -> impl IntoView {
    view! { <article class="rounded-2xl border border-border bg-card p-4"><div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{title}</div><div class="mt-2 text-lg font-semibold text-card-foreground">{value}</div></article> }
}
