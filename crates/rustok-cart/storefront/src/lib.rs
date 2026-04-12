mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    StorefrontCart, StorefrontCartAdjustment, StorefrontCartData, StorefrontCartDeliveryGroup,
    StorefrontCartLineItem,
};

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
    let (mutation_error, set_mutation_error) = signal(Option::<String>::None);

    let resource = Resource::new_blocking(
        move || {
            (
                selected_cart_id.clone(),
                selected_locale.clone(),
                refresh_nonce.get(),
            )
        },
        move |(cart_id, locale, _)| async move { api::fetch_storefront_cart(cart_id, locale).await },
    );

    let on_decrement = {
        let update_error = update_error.clone();
        Callback::new(
            move |(cart_id, line_item_id, quantity): (String, String, i32)| {
                let update_error = update_error.clone();
                set_mutation_busy.set(true);
                set_mutation_error.set(None);
                spawn_local(async move {
                    match api::decrement_storefront_cart_line_item(cart_id, line_item_id, quantity)
                        .await
                    {
                        Ok(()) => set_refresh_nonce.update(|value| *value += 1),
                        Err(err) => set_mutation_error.set(Some(format!("{update_error}: {err}"))),
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
                match api::remove_storefront_cart_line_item(cart_id, line_item_id).await {
                    Ok(()) => set_refresh_nonce.update(|value| *value += 1),
                    Err(err) => set_mutation_error.set(Some(format!("{update_error}: {err}"))),
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
                    mutation_error.get().map(|error| {
                        view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {error}
                            </div>
                        }
                    })
                }}
                <Suspense fallback=|| view! { <div class="space-y-4"><div class="h-48 animate-pulse rounded-3xl bg-muted"></div><div class="grid gap-3 md:grid-cols-2"><div class="h-40 animate-pulse rounded-2xl bg-muted"></div><div class="h-40 animate-pulse rounded-2xl bg-muted"></div></div></div> }>
                    {move || {
                        let resource = resource.clone();
                        let load_error = load_error.clone();
                        let on_decrement = on_decrement.clone();
                        let on_remove = on_remove.clone();
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
                                Err(err) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{}: {err}", load_error)}</div> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
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
    let email = cart
        .email
        .unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
    let channel = cart
        .channel_slug
        .unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
    let customer = cart
        .customer_id
        .unwrap_or_else(|| t(locale.as_deref(), "cart.summary.guest", "guest"));
    let region = cart
        .region_id
        .unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
    let country = cart
        .country_code
        .unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
    let locale_code = cart
        .locale_code
        .unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "cart.summary.badge", "cart snapshot")}
                </span>
                <h3 class="text-2xl font-semibold text-card-foreground">{cart.id}</h3>
                <p class="text-sm leading-7 text-muted-foreground">
                    {t(locale.as_deref(), "cart.summary.subtitle", "Cart state, identity, and locale/channel snapshot now come directly from the cart module.")}
                </p>
            </div>
            <div class="mt-6 grid gap-3 md:grid-cols-2">
                <MetricCard title=t(locale.as_deref(), "cart.summary.status", "Status") value=cart.status />
                <MetricCard title=t(locale.as_deref(), "cart.summary.subtotal", "Subtotal") value=format!("{} {}", cart.currency_code, cart.subtotal_amount) />
                <MetricCard title=t(locale.as_deref(), "cart.summary.adjustments", "Adjustments") value=format!("{} {}", cart.currency_code, cart.adjustment_total) />
                <MetricCard title=t(locale.as_deref(), "cart.summary.total", "Total") value=format!("{} {}", cart.currency_code, cart.total_amount) />
                <MetricCard title=t(locale.as_deref(), "cart.summary.email", "Email") value=email />
                <MetricCard title=t(locale.as_deref(), "cart.summary.channel", "Channel") value=channel />
                <MetricCard title=t(locale.as_deref(), "cart.summary.customer", "Customer") value=customer />
                <MetricCard title=t(locale.as_deref(), "cart.summary.region", "Region") value=region />
                <MetricCard title=t(locale.as_deref(), "cart.summary.country", "Country") value=country />
                <MetricCard title=t(locale.as_deref(), "cart.summary.locale", "Locale") value=locale_code />
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
                            let source = adjustment.source_id.unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
                            let line_item = adjustment.line_item_id.unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
                            view! {
                                <article class="rounded-2xl border border-border bg-card p-4">
                                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{adjustment.source_type}</div>
                                    <div class="mt-2 grid gap-2 md:grid-cols-3">
                                        <MetricCard title=t(locale.as_deref(), "cart.adjustments.source", "Source") value=source />
                                        <MetricCard title=t(locale.as_deref(), "cart.adjustments.lineItem", "Line item") value=line_item />
                                        <MetricCard title=t(locale.as_deref(), "cart.adjustments.amount", "Amount") value=format!("{} {}", adjustment.currency_code, adjustment.amount) />
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
                            let shipping_option = group.selected_shipping_option_id.unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
                            let seller_identity = group.seller_id.clone().or_else(|| group.seller_scope.clone());
                            view! {
                                <article class="rounded-2xl border border-border bg-card p-4">
                                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{group.shipping_profile_slug}</div>
                                    {seller_identity.map(|seller_identity| view! {
                                        <div class="mt-2 text-xs text-muted-foreground break-all">{seller_identity}</div>
                                    })}
                                    <div class="mt-2 grid gap-2 md:grid-cols-3">
                                        <MetricCard title=t(locale.as_deref(), "cart.groups.items", "Line items") value=group.line_item_count.to_string() />
                                        <MetricCard title=t(locale.as_deref(), "cart.groups.selected", "Selected shipping option") value=shipping_option />
                                        <MetricCard title=t(locale.as_deref(), "cart.groups.available", "Available shipping options") value=group.available_option_count.to_string() />
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
                            let StorefrontCartLineItem {
                                id,
                                title,
                                sku,
                                quantity,
                                unit_price,
                                total_price,
                                currency_code,
                                shipping_profile_slug,
                                seller_id,
                                seller_scope,
                            } = item;
                            let sku = sku.unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
                            let seller_identity = seller_id
                                .or(seller_scope)
                                .unwrap_or_else(|| t(locale.as_deref(), "cart.summary.empty", "not set"));
                            let decrement_cart_id = cart_id.clone();
                            let decrement_line_item_id = id.clone();
                            let decrement_quantity = quantity;
                            let remove_cart_id = cart_id.clone();
                            let remove_line_item_id = id;
                            let on_decrement = on_decrement.clone();
                            let on_remove = on_remove.clone();
                            let decrement_label_locale = locale.clone();
                            let remove_label_locale = locale.clone();
                            let decrement_busy_label = busy_label.clone();
                            let remove_busy_label = busy_label.clone();
                            let unit_price_value = format!("{} {}", currency_code.clone(), unit_price);
                            let total_price_value = format!("{} {}", currency_code, total_price);
                            view! {
                                <article class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div>
                                            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{shipping_profile_slug}</div>
                                            <h4 class="mt-2 text-base font-semibold text-card-foreground">{title}</h4>
                                            <div class="mt-1 text-xs text-muted-foreground break-all">{seller_identity}</div>
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
                                        <MetricCard title=t(locale.as_deref(), "cart.items.sku", "SKU") value=sku />
                                        <MetricCard title=t(locale.as_deref(), "cart.items.quantity", "Quantity") value=quantity.to_string() />
                                        <MetricCard title=t(locale.as_deref(), "cart.items.unitPrice", "Unit price") value=unit_price_value />
                                        <MetricCard title=t(locale.as_deref(), "cart.items.totalPrice", "Total price") value=total_price_value />
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
