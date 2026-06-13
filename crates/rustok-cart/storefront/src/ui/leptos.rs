use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::core::{
    build_cart_fetch_request, build_decrement_line_item_request, build_remove_line_item_request,
    cart_adjustment_view_model, cart_delivery_group_view_model, cart_line_item_view_model,
    cart_summary_view_model, error_with_context, CartDisplayFallbacks,
};
use crate::i18n::t;
use crate::model::{
    StorefrontCart, StorefrontCartAdjustment, StorefrontCartData, StorefrontCartDeliveryGroup,
    StorefrontCartLineItem,
};
use crate::transport;

fn cart_display_fallbacks(locale: Option<&str>) -> CartDisplayFallbacks {
    CartDisplayFallbacks::new(
        t(locale, "cart.summary.empty", "not set"),
        t(locale, "cart.summary.guest", "guest"),
    )
}

#[component]
pub fn CartView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_cart_id = read_route_query_value(&route_context, "cart_id");
    let selected_locale = route_context.locale.clone();
    let badge = t(selected_locale.as_deref(), "cart.badge", "cart");
    let title = t(
        selected_locale.as_deref(),
        "cart.title",
        "Cart workspace from the module package",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "cart.subtitle",
        "The cart module now owns a storefront cart workspace for cart state, line items, and delivery-group snapshots. Checkout completion still remains aggregate in commerce.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "cart.error.load",
        "Failed to load storefront cart data",
    );
    let update_error = t(
        selected_locale.as_deref(),
        "cart.error.update",
        "Failed to update cart line items",
    );

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (mutation_busy, set_mutation_busy) = signal(false);
    let (mutation_error, set_mutation_error) =
        signal(Option::<(String, transport::CartTransportError)>::None);

    let resource = Resource::new_blocking(
        move || {
            (
                selected_cart_id.clone(),
                selected_locale.clone(),
                refresh_nonce.get(),
            )
        },
        move |(cart_id, locale, _)| async move {
            transport::fetch_cart(build_cart_fetch_request(cart_id, locale)).await
        },
    );

    let on_decrement = {
        let update_error = update_error.clone();
        Callback::new(
            move |(cart_id, line_item_id, quantity): (String, String, i32)| {
                let update_error = update_error.clone();
                set_mutation_busy.set(true);
                set_mutation_error.set(None);
                spawn_local(async move {
                    let request =
                        build_decrement_line_item_request(cart_id, line_item_id, quantity);
                    match transport::decrement_line_item(request).await {
                        Ok(()) => set_refresh_nonce.update(|value| *value += 1),
                        Err(err) => set_mutation_error.set(Some((update_error.clone(), err))),
                    }
                    set_mutation_busy.set(false);
                });
            },
        )
    };

    let on_remove = {
        let update_error = update_error.clone();
        Callback::new(move |(cart_id, line_item_id): (String, String)| {
            let update_error = update_error.clone();
            set_mutation_busy.set(true);
            set_mutation_error.set(None);
            spawn_local(async move {
                let request = build_remove_line_item_request(cart_id, line_item_id);
                match transport::remove_line_item(request).await {
                    Ok(()) => set_refresh_nonce.update(|value| *value += 1),
                    Err(err) => set_mutation_error.set(Some((update_error.clone(), err))),
                }
                set_mutation_busy.set(false);
            });
        })
    };

    view! {
        <section class="rounded-[2rem] border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{badge}</span>
                <h2 class="text-3xl font-semibold text-card-foreground">{title}</h2>
                <p class="text-sm text-muted-foreground">{subtitle}</p>
            </div>
            <div class="mt-6 space-y-4">
                {move || {
                    mutation_error.get().map(|(context, error)| {
                        view! { <CartTransportErrorMessage context error /> }
                    })
                }}
                <Suspense fallback=|| view! { <div class="space-y-4"><div class="h-48 animate-pulse rounded-3xl bg-muted"></div><div class="grid gap-3 md:grid-cols-2"><div class="h-40 animate-pulse rounded-2xl bg-muted"></div><div class="h-40 animate-pulse rounded-2xl bg-muted"></div></div></div> }>
                    {move || {
                        let resource = resource;
                        let load_error = load_error.clone();
                        let on_decrement = on_decrement;
                        let on_remove = on_remove;
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! {
                                    <CartWorkspace
                                        data
                                        on_decrement
                                        on_remove
                                        busy=mutation_busy
                                    />
                                }
                                .into_any(),
                                Err(err) => view! { <CartTransportErrorMessage context=load_error error=err /> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn CartTransportErrorMessage(
    context: String,
    error: transport::CartTransportError,
) -> impl IntoView {
    let failed_path = error.failed_path.as_str().to_string();
    let fallback_attempted = error.fallback_attempted.to_string();
    let native_error = error.native_error.clone().unwrap_or_default();
    let graphql_error = error.graphql_error.clone().unwrap_or_default();
    let message = error_with_context(&context, &error.to_string());

    view! {
        <div
            class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
            data-cart-transport-failed-path=failed_path
            data-cart-transport-fallback-attempted=fallback_attempted
            data-cart-transport-native-error=native_error
            data-cart-transport-graphql-error=graphql_error
        >
            {message}
        </div>
    }
}

#[component]
fn CartWorkspace(
    data: StorefrontCartData,
    on_decrement: Callback<(String, String, i32)>,
    on_remove: Callback<(String, String)>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    match (data.selected_cart_id.clone(), data.cart) {
        (None, _) => view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "cart.empty.title", "No cart selected")}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "cart.empty.body", "Open this route with `?cart_id=` to inspect an active storefront cart from the cart-owned module package.")}
                </p>
            </article>
        }.into_any(),
        (Some(cart_id), None) => view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "cart.missing.title", "Cart not found")}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "cart.missing.body", "The requested storefront cart could not be found in this tenant or is not accessible for the current storefront customer.")}
                </p>
                <div class="mt-4 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{cart_id}</div>
            </article>
        }.into_any(),
        (_, Some(cart)) => {
            let cart_id = cart.id.clone();
            view! {
                <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                    <div class="space-y-6">
                        <CartSummaryCard cart=cart.clone() />
                        <AdjustmentsCard adjustments=cart.adjustments.clone() />
                        <DeliveryGroupsCard groups=cart.delivery_groups />
                    </div>
                    <LineItemsRail
                        cart_id
                        items=cart.line_items
                        on_decrement
                        on_remove
                        busy
                    />
                </div>
            }
            .into_any()
        }
    }
}

#[component]
fn CartSummaryCard(cart: StorefrontCart) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let view_model = cart_summary_view_model(&cart, &cart_display_fallbacks(locale.as_deref()));

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "cart.summary.badge", "cart snapshot")}
                </span>
                <h3 class="text-2xl font-semibold text-card-foreground">{view_model.id}</h3>
                <p class="text-sm leading-7 text-muted-foreground">
                    {t(locale.as_deref(), "cart.summary.subtitle", "Cart state, identity, and locale/channel snapshot now come directly from the cart module.")}
                </p>
            </div>
            <div class="mt-6 grid gap-3 md:grid-cols-2">
                <MetricCard title=t(locale.as_deref(), "cart.summary.status", "Status") value=view_model.status />
                <MetricCard title=t(locale.as_deref(), "cart.summary.subtotal", "Subtotal") value=view_model.subtotal />
                <MetricCard title=t(locale.as_deref(), "cart.summary.adjustments", "Adjustments") value=view_model.adjustments />
                <MetricCard title=t(locale.as_deref(), "cart.summary.shipping", "Shipping") value=view_model.shipping />
                <MetricCard title=t(locale.as_deref(), "cart.summary.total", "Total") value=view_model.total />
                <MetricCard title=t(locale.as_deref(), "cart.summary.email", "Email") value=view_model.email />
                <MetricCard title=t(locale.as_deref(), "cart.summary.channel", "Channel") value=view_model.channel />
                <MetricCard title=t(locale.as_deref(), "cart.summary.customer", "Customer") value=view_model.customer />
                <MetricCard title=t(locale.as_deref(), "cart.summary.region", "Region") value=view_model.region />
                <MetricCard title=t(locale.as_deref(), "cart.summary.country", "Country") value=view_model.country />
                <MetricCard title=t(locale.as_deref(), "cart.summary.locale", "Locale") value=view_model.locale />
            </div>
        </article>
    }
}

#[component]
fn AdjustmentsCard(adjustments: Vec<StorefrontCartAdjustment>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "cart.adjustments.title", "Adjustments")}</h3>
                <span class="text-sm text-muted-foreground">{adjustments.len().to_string()}</span>
            </div>
            {if adjustments.is_empty() {
                view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        {t(locale.as_deref(), "cart.adjustments.empty", "No typed cart adjustments are attached to this cart yet.")}
                    </p>
                }.into_any()
            } else {
                view! {
                    <div class="mt-4 space-y-3">
                        {adjustments.into_iter().map(|adjustment| {
                            let locale = locale.clone();
                            let view_model = cart_adjustment_view_model(
                                adjustment,
                                &cart_display_fallbacks(locale.as_deref()),
                            );
                            view! {
                                <article class="rounded-2xl border border-border bg-card p-4">
                                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{view_model.source_type}</div>
                                    <div class="mt-2 grid gap-2 md:grid-cols-4">
                                        <MetricCard title=t(locale.as_deref(), "cart.adjustments.source", "Source") value=view_model.source />
                                        <MetricCard title=t(locale.as_deref(), "cart.adjustments.scope", "Scope") value=view_model.scope />
                                        <MetricCard title=t(locale.as_deref(), "cart.adjustments.lineItem", "Line item") value=view_model.line_item />
                                        <MetricCard title=t(locale.as_deref(), "cart.adjustments.amount", "Amount") value=view_model.amount />
                                    </div>
                                    <div class="mt-3 rounded-2xl border border-border/60 bg-background/60 p-3">
                                        <div class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground">
                                            {t(locale.as_deref(), "cart.adjustments.metadata", "Metadata")}
                                        </div>
                                        <pre class="mt-2 whitespace-pre-wrap break-all text-xs text-muted-foreground">{view_model.metadata}</pre>
                                    </div>
                                </article>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </article>
    }
}

#[component]
fn DeliveryGroupsCard(groups: Vec<StorefrontCartDeliveryGroup>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "cart.groups.title", "Delivery groups")}</h3>
                <span class="text-sm text-muted-foreground">{groups.len().to_string()}</span>
            </div>
            {if groups.is_empty() {
                view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        {t(locale.as_deref(), "cart.groups.empty", "This cart does not have delivery groups yet.")}
                    </p>
                }.into_any()
            } else {
                view! {
                    <div class="mt-4 space-y-3">
                        {groups.into_iter().map(|group| {
                            let locale = locale.clone();
                            let view_model = cart_delivery_group_view_model(
                                group,
                                &cart_display_fallbacks(locale.as_deref()),
                            );
                            let seller_identity = view_model.seller_identity.clone();
                            view! {
                                <article class="rounded-2xl border border-border bg-card p-4">
                                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{view_model.shipping_profile_slug}</div>
                                    {seller_identity.map(|seller_identity| view! {
                                        <div class="mt-2 text-xs text-muted-foreground break-all">{seller_identity}</div>
                                    })}
                                    <div class="mt-2 grid gap-2 md:grid-cols-3">
                                        <MetricCard title=t(locale.as_deref(), "cart.groups.items", "Line items") value=view_model.line_item_count />
                                        <MetricCard title=t(locale.as_deref(), "cart.groups.selected", "Selected shipping option") value=view_model.selected_shipping_option />
                                        <MetricCard title=t(locale.as_deref(), "cart.groups.available", "Available shipping options") value=view_model.available_option_count />
                                    </div>
                                </article>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </article>
    }
}

#[component]
fn LineItemsRail(
    cart_id: String,
    items: Vec<StorefrontCartLineItem>,
    on_decrement: Callback<(String, String, i32)>,
    on_remove: Callback<(String, String)>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let busy_label = t(locale.as_deref(), "cart.items.pending", "Updating...");

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <div>
                    <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "cart.items.title", "Line items")}</h3>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {t(locale.as_deref(), "cart.items.actions.hint", "The cart module can safely decrement or remove line items here. Quantity increases and checkout stay in aggregate commerce flows.")}
                    </p>
                </div>
                <span class="text-sm text-muted-foreground">{items.len().to_string()}</span>
            </div>
            {if items.is_empty() {
                view! {
                    <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                        {t(locale.as_deref(), "cart.items.empty", "This cart does not contain any line items yet.")}
                    </article>
                }.into_any()
            } else {
                view! {
                    <div class="space-y-3">
                        {items.into_iter().map(|item| {
                            let locale = locale.clone();
                            let view_model = cart_line_item_view_model(
                                item,
                                &cart_display_fallbacks(locale.as_deref()),
                            );
                            let decrement_cart_id = cart_id.clone();
                            let decrement_line_item_id = view_model.id.clone();
                            let decrement_quantity = view_model.quantity;
                            let remove_cart_id = cart_id.clone();
                            let remove_line_item_id = view_model.id.clone();
                            let decrement_label_locale = locale.clone();
                            let remove_label_locale = locale.clone();
                            let decrement_busy_label = busy_label.clone();
                            let remove_busy_label = busy_label.clone();

                            view! {
                                <article class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div>
                                            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{view_model.shipping_profile_slug}</div>
                                            <h4 class="mt-2 text-base font-semibold text-card-foreground">{view_model.title}</h4>
                                            <div class="mt-1 text-xs text-muted-foreground break-all">{view_model.seller_identity}</div>
                                        </div>
                                        <div class="flex flex-wrap gap-2">
                                            <button
                                                type="button"
                                                class="inline-flex items-center rounded-full border border-border px-3 py-1.5 text-xs font-medium uppercase tracking-[0.14em] text-card-foreground transition hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
                                                disabled=move || busy.get()
                                                on:click=move |_| {
                                                    on_decrement.run((
                                                        decrement_cart_id.clone(),
                                                        decrement_line_item_id.clone(),
                                                        decrement_quantity,
                                                    ));
                                                }
                                            >
                                                {move || if busy.get() { decrement_busy_label.clone() } else { t(decrement_label_locale.as_deref(), "cart.items.actions.decrement", "Decrease") }}
                                            </button>
                                            <button
                                                type="button"
                                                class="inline-flex items-center rounded-full border border-destructive/30 px-3 py-1.5 text-xs font-medium uppercase tracking-[0.14em] text-destructive transition hover:bg-destructive/10 disabled:cursor-not-allowed disabled:opacity-60"
                                                disabled=move || busy.get()
                                                on:click=move |_| {
                                                    on_remove.run((
                                                        remove_cart_id.clone(),
                                                        remove_line_item_id.clone(),
                                                    ));
                                                }
                                            >
                                                {move || if busy.get() { remove_busy_label.clone() } else { t(remove_label_locale.as_deref(), "cart.items.actions.remove", "Remove") }}
                                            </button>
                                        </div>
                                    </div>
                                    <div class="mt-4 grid gap-3 md:grid-cols-2">
                                        <MetricCard title=t(locale.as_deref(), "cart.items.sku", "SKU") value=view_model.sku />
                                        <MetricCard title=t(locale.as_deref(), "cart.items.quantity", "Quantity") value=view_model.quantity_label />
                                        <MetricCard title=t(locale.as_deref(), "cart.items.unitPrice", "Unit price") value=view_model.unit_price />
                                        <MetricCard title=t(locale.as_deref(), "cart.items.totalPrice", "Total price") value=view_model.total_price />
                                    </div>
                                </article>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn MetricCard(title: String, value: String) -> impl IntoView {
    view! { <article class="rounded-2xl border border-border bg-card p-4"><div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{title}</div><div class="mt-2 text-lg font-semibold text-card-foreground break-all">{value}</div></article> }
}

#[component]
pub fn CartCheckoutHandoffCard(
    cart_id: String,
    status: String,
    labels: crate::core::CartCheckoutHandoffLabels,
) -> impl IntoView {
    let view_model = crate::core::cart_checkout_handoff_view_model(cart_id, status, &labels);

    view! {
        <div class="mt-6 rounded-2xl border border-dashed border-border px-4 py-3 text-sm text-muted-foreground">
            {view_model.summary}
            <span class="ml-2">
                {view_model.module_ownership}
            </span>
        </div>
    }
}
