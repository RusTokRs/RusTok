use crate::core::{
    build_catalog_rail_view_model, build_fetch_request, build_product_catalog_rail_labels,
    build_route_input, build_selected_product_empty_view_model, build_selected_product_view_model,
    build_shell_view_model, build_transport_error_dom_evidence, resolve_route_segment,
};
use crate::model::{
    ProductDetail, ProductListItem, ProductPricingContext, ProductPricingDetail,
    StorefrontProductsData,
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
    let shell = build_shell_view_model(route_input.locale.as_deref());
    let fetch_request = build_fetch_request(&route_input);

    let resource = Resource::new_blocking(
        move || fetch_request.clone(),
        move |request| async move { transport::fetch_products(request).await },
    );

    view! {
        <section class="rounded-[2rem] border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{shell.badge}</span>
                <h2 class="text-3xl font-semibold text-card-foreground">{shell.title}</h2>
                <p class="text-sm text-muted-foreground">{shell.subtitle}</p>
            </div>
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
