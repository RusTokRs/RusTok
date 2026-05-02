mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    StorefrontCheckoutAdjustment, StorefrontCheckoutCart, StorefrontCheckoutCompletion,
    StorefrontCheckoutDeliveryGroup, StorefrontCheckoutPaymentCollection,
    StorefrontCheckoutWorkspace, StorefrontCommerceData,
};

type ShippingSelectCallback = Callback<(
    StorefrontCheckoutCart,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
)>;

#[component]
pub fn CommerceView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_locale = route_context.locale.clone();
    let selected_cart_id = read_route_query_value(&route_context, "cart_id");
    let badge = t(selected_locale.as_deref(), "commerce.badge", "commerce");
    let title = t(
        selected_locale.as_deref(),
        "commerce.title",
        "Commerce orchestration hub",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "commerce.subtitle",
        "Catalog, pricing, regions, and cart line-item handling now live in module-owned storefront packages. Commerce remains the aggregate storefront handoff for checkout context and cross-domain flow.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "commerce.error.load",
        "Failed to load commerce storefront data",
    );
    let action_error_label = t(
        selected_locale.as_deref(),
        "commerce.error.action",
        "Failed to update aggregate checkout state",
    );

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (action_busy, set_action_busy) = signal(false);
    let (action_error, set_action_error) = signal(Option::<String>::None);
    let (completion, set_completion) = signal(Option::<StorefrontCheckoutCompletion>::None);

    let resource = Resource::new_blocking(
        move || {
            (
                selected_cart_id.clone(),
                selected_locale.clone(),
                refresh_nonce.get(),
            )
        },
        move |(cart_id, locale, _)| async move { api::fetch_storefront_commerce(cart_id, locale).await },
    );

    let on_create_payment_collection = {
        let action_error_label = action_error_label.clone();
        Callback::new(move |cart_id: String| {
            let action_error_label = action_error_label.clone();
            set_action_busy.set(true);
            set_action_error.set(None);
            set_completion.set(None);
            spawn_local(async move {
                match api::create_storefront_payment_collection(cart_id).await {
                    Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                    Err(err) => set_action_error.set(Some(format!("{action_error_label}: {err}"))),
                }
                set_action_busy.set(false);
            });
        })
    };

    let on_select_shipping_option = {
        let action_error_label = action_error_label.clone();
        Callback::new(
            move |(cart, shipping_profile_slug, seller_id, seller_scope, shipping_option_id): (
                StorefrontCheckoutCart,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            )| {
                let action_error_label = action_error_label.clone();
                set_action_busy.set(true);
                set_action_error.set(None);
                set_completion.set(None);
                spawn_local(async move {
                    match api::select_storefront_shipping_option(
                        cart,
                        shipping_profile_slug,
                        seller_id,
                        seller_scope,
                        shipping_option_id,
                    )
                    .await
                    {
                        Ok(()) => set_refresh_nonce.update(|value| *value += 1),
                        Err(err) => {
                            set_action_error.set(Some(format!("{action_error_label}: {err}")))
                        }
                    }
                    set_action_busy.set(false);
                });
            },
        )
    };

    let on_complete_checkout = {
        let action_error_label = action_error_label.clone();
        Callback::new(move |cart_id: String| {
            let action_error_label = action_error_label.clone();
            set_action_busy.set(true);
            set_action_error.set(None);
            spawn_local(async move {
                match api::complete_storefront_checkout(cart_id).await {
                    Ok(result) => {
                        set_completion.set(Some(result));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_action_error.set(Some(format!("{action_error_label}: {err}"))),
                }
                set_action_busy.set(false);
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
                    action_error.get().map(|error| {
                        view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {error}
                            </div>
                        }
                    })
                }}
                <Suspense fallback=|| view! { <div class="space-y-4"><div class="h-48 animate-pulse rounded-3xl bg-muted"></div><div class="grid gap-3 md:grid-cols-2"><div class="h-48 animate-pulse rounded-2xl bg-muted"></div><div class="h-48 animate-pulse rounded-2xl bg-muted"></div></div></div> }>
                    {move || {
                        let resource = resource;
                        let load_error = load_error.clone();
                        let on_create_payment_collection = on_create_payment_collection;
                        let on_select_shipping_option = on_select_shipping_option;
                        let on_complete_checkout = on_complete_checkout;
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! {
                                    <CommerceShowcase
                                        data
                                        busy=action_busy
                                        completion
                                        on_create_payment_collection
                                        on_select_shipping_option
                                        on_complete_checkout
                                    />
                                }.into_any(),
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
fn CommerceShowcase(
    data: StorefrontCommerceData,
    busy: ReadSignal<bool>,
    completion: ReadSignal<Option<StorefrontCheckoutCompletion>>,
    on_create_payment_collection: Callback<String>,
    on_select_shipping_option: ShippingSelectCallback,
    on_complete_checkout: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
            <ContextCard data=data.clone() />
            <div class="space-y-6">
                <CheckoutWorkspace
                    selected_cart_id=data.selected_cart_id
                    checkout=data.checkout
                    busy
                    completion
                    on_create_payment_collection
                    on_select_shipping_option
                    on_complete_checkout
                />
                <SurfaceRail />
            </div>
        </div>
    }
}

#[component]
fn ContextCard(data: StorefrontCommerceData) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let tenant_value = data.tenant_slug.unwrap_or_else(|| {
        t(
            locale.as_deref(),
            "commerce.context.tenantMissing",
            "host tenant",
        )
    });
    let channel_value = data
        .channel_slug
        .unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved"));
    let resolution_value = data
        .channel_resolution_source
        .unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved"));

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "commerce.context.badge", "storefront context")}
                </span>
                <h3 class="text-2xl font-semibold text-card-foreground">
                    {t(locale.as_deref(), "commerce.context.title", "Active storefront context")}
                </h3>
                <p class="text-sm leading-7 text-muted-foreground">
                    {t(locale.as_deref(), "commerce.context.subtitle", "This aggregate route now exposes the request context that still coordinates checkout, payment collection, and delivery orchestration across split modules.")}
                </p>
            </div>
            <div class="mt-6 grid gap-3 md:grid-cols-2">
                <MetricCard title=t(locale.as_deref(), "commerce.context.locale", "Effective locale") value=data.effective_locale />
                <MetricCard title=t(locale.as_deref(), "commerce.context.tenant", "Tenant") value=tenant_value />
                <MetricCard title=t(locale.as_deref(), "commerce.context.tenantDefault", "Tenant default locale") value=data.tenant_default_locale />
                <MetricCard title=t(locale.as_deref(), "commerce.context.channel", "Channel") value=channel_value />
            </div>
            <div class="mt-3">
                <MetricCard title=t(locale.as_deref(), "commerce.context.resolution", "Channel source") value=resolution_value />
            </div>
        </article>
    }
}

#[component]
fn CheckoutWorkspace(
    selected_cart_id: Option<String>,
    checkout: Option<StorefrontCheckoutWorkspace>,
    busy: ReadSignal<bool>,
    completion: ReadSignal<Option<StorefrontCheckoutCompletion>>,
    on_create_payment_collection: Callback<String>,
    on_select_shipping_option: ShippingSelectCallback,
    on_complete_checkout: Callback<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let cart_base_href = route_context.module_route_base("cart");

    match selected_cart_id {
        None => view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "commerce.checkout.empty.title", "No checkout cart selected")}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "commerce.checkout.empty.body", "Open this route with `?cart_id=` to load aggregate checkout state for a storefront cart. Discovery and line-item edits already live in module-owned packages.")}
                </p>
                <div class="mt-4">
                    <a class="inline-flex text-sm font-medium text-primary hover:underline" href=cart_base_href>
                        {t(locale.as_deref(), "commerce.checkout.empty.openCart", "Open cart module")}
                    </a>
                </div>
            </article>
        }.into_any(),
        Some(cart_id) => {
            let cart_href = format!("{cart_base_href}?cart_id={cart_id}");
            match checkout.and_then(|workspace| workspace.cart.map(|cart| (cart, workspace.payment_collection))) {
                Some((cart, payment_collection)) => {
                    let delivery_groups = cart.delivery_groups.clone();
                    let create_pending_locale = locale.clone();
                    let create_action_locale = locale.clone();
                    let shipping_pending_locale = locale.clone();
                    let complete_pending_locale = locale.clone();
                    let complete_action_locale = locale.clone();
                    view! {
                    <article class="rounded-3xl border border-border bg-background p-8">
                        <div class="space-y-3">
                            <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                                {t(locale.as_deref(), "commerce.checkout.badge", "checkout workspace")}
                            </span>
                            <h3 class="text-2xl font-semibold text-card-foreground">
                                {t(locale.as_deref(), "commerce.checkout.title", "Aggregate checkout orchestration")}
                            </h3>
                            <p class="text-sm leading-7 text-muted-foreground">
                                {t(locale.as_deref(), "commerce.checkout.subtitle", "This workspace keeps only the aggregate steps that still span cart, payment, order, and fulfillment boundaries.")}
                            </p>
                        </div>
                        {move || {
                            completion.get().map(|result| {
                                view! { <CheckoutCompletionCard result /> }
                            })
                        }}
                        <div class="mt-6 grid gap-3 md:grid-cols-2">
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.id", "Cart") value=cart.id.clone() />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.status", "Cart status") value=cart.status.clone() />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.subtotal", "Cart subtotal") value=format!("{} {}", cart.currency_code, cart.subtotal_amount) />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.adjustments", "Cart adjustments") value=format!("{} {}", cart.currency_code, cart.adjustment_total) />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.shippingTotal", "Cart shipping") value=format!("{} {}", cart.currency_code, cart.shipping_total) />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.total", "Cart total") value=format!("{} {}", cart.currency_code, cart.total_amount) />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.lineItems", "Line items") value=cart.line_item_count.to_string() />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.adjustmentCount", "Adjustment rows") value=cart.adjustment_count.to_string() />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.groups", "Delivery groups") value=cart.delivery_group_count.to_string() />
                            <MetricCard title=t(locale.as_deref(), "commerce.checkout.cart.shipping", "Legacy shipping shortcut") value=cart.selected_shipping_option_id.clone().unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved")) />
                        </div>
                        <div class="mt-6">
                            <DeliveryGroupsCard
                                cart=cart.clone()
                                groups=delivery_groups
                                busy
                                pending_label=t(shipping_pending_locale.as_deref(), "commerce.checkout.pending", "Processing...")
                                on_select_shipping_option
                            />
                        </div>
                        <div class="mt-6">
                            <AdjustmentsCard adjustments=cart.adjustments.clone() />
                        </div>
                        <div class="mt-6 grid gap-3 md:grid-cols-2">
                            <a class="inline-flex items-center justify-center rounded-full border border-border px-4 py-2 text-sm font-medium text-card-foreground transition hover:bg-muted" href=cart_href>
                                {t(locale.as_deref(), "commerce.checkout.openCart", "Open cart workspace")}
                            </a>
                            <button
                                type="button"
                                class="inline-flex items-center justify-center rounded-full border border-border px-4 py-2 text-sm font-medium text-card-foreground transition hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
                                disabled=move || busy.get()
                                on:click={
                                    let cart_id = cart.id.clone();
                                    move |_| on_create_payment_collection.run(cart_id.clone())
                                }
                            >
                                {move || {
                                    if busy.get() {
                                        t(create_pending_locale.as_deref(), "commerce.checkout.pending", "Processing...")
                                    } else {
                                        t(create_action_locale.as_deref(), "commerce.checkout.createCollection", "Create or reuse payment collection")
                                    }
                                }}
                            </button>
                            <button
                                type="button"
                                class="inline-flex items-center justify-center rounded-full border border-primary bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60 md:col-span-2"
                                disabled=move || busy.get()
                                on:click={
                                    let cart_id = cart.id.clone();
                                    move |_| on_complete_checkout.run(cart_id.clone())
                                }
                            >
                                {move || {
                                    if busy.get() {
                                        t(complete_pending_locale.as_deref(), "commerce.checkout.pending", "Processing...")
                                    } else {
                                        t(complete_action_locale.as_deref(), "commerce.checkout.complete", "Complete checkout")
                                    }
                                }}
                            </button>
                        </div>
                        <div class="mt-6">
                            <PaymentCollectionCard payment_collection />
                        </div>
                    </article>
                }.into_any()
                },
                None => view! {
                    <article class="rounded-3xl border border-dashed border-border p-8">
                        <h3 class="text-lg font-semibold text-card-foreground">
                            {t(locale.as_deref(), "commerce.checkout.missing.title", "Checkout cart not found")}
                        </h3>
                        <p class="mt-2 text-sm text-muted-foreground">
                            {t(locale.as_deref(), "commerce.checkout.missing.body", "The requested cart could not be resolved for aggregate checkout state in this tenant.")}
                        </p>
                        <div class="mt-4 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{cart_id}</div>
                    </article>
                }.into_any(),
            }
        }
    }
}

#[component]
fn DeliveryGroupsCard(
    cart: StorefrontCheckoutCart,
    groups: Vec<StorefrontCheckoutDeliveryGroup>,
    busy: ReadSignal<bool>,
    pending_label: String,
    on_select_shipping_option: ShippingSelectCallback,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let empty_value = t(locale.as_deref(), "commerce.context.empty", "not resolved");

    view! {
        <article class="rounded-2xl border border-border bg-card p-5">
            <div class="space-y-2">
                <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "commerce.delivery.badge", "delivery orchestration")}
                </div>
                <h4 class="text-base font-semibold text-card-foreground">
                    {t(locale.as_deref(), "commerce.delivery.title", "Delivery groups and shipping selections")}
                </h4>
                <p class="text-sm leading-7 text-muted-foreground">
                    {t(locale.as_deref(), "commerce.delivery.subtitle", "Shipping option selection remains in `commerce` because it coordinates cart grouping, shipping availability, and checkout readiness across split module boundaries.")}
                </p>
            </div>
            <div class="mt-4 space-y-3">
                {groups
                    .into_iter()
                    .map(|group| {
                        let current_cart = cart.clone();
                        let selected_shipping_option_id = group.selected_shipping_option_id.clone();
                        let available_options = group.available_shipping_options.clone();
                        let shipping_profile_slug = group.shipping_profile_slug.clone();
                        let seller_id = group.seller_id.clone();
                        let seller_scope = group.seller_scope.clone();
                        let shipping_profile_slug_for_clear = shipping_profile_slug.clone();
                        let seller_id_for_clear = seller_id.clone();
                        let seller_scope_for_clear = seller_scope.clone();
                        let seller_identity = seller_id.clone().or_else(|| seller_scope.clone());
                        view! {
                            <div class="rounded-2xl border border-border bg-background p-4">
                                <div class="flex flex-wrap items-start justify-between gap-3">
                                    <div class="space-y-1">
                                        <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{shipping_profile_slug.clone()}</div>
                                        {seller_identity.as_ref().map(|seller_identity| view! {
                                            <div class="text-sm text-muted-foreground break-all">{seller_identity.clone()}</div>
                                        })}
                                        <div class="text-sm text-muted-foreground">
                                            {format!(
                                                "{}: {}",
                                                t(locale.as_deref(), "commerce.delivery.lineItems", "Line items"),
                                                group.line_item_count
                                            )}
                                        </div>
                                        <div class="text-sm text-muted-foreground break-all">
                                            {format!(
                                                "{}: {}",
                                                t(locale.as_deref(), "commerce.delivery.selected", "Selected option"),
                                                selected_shipping_option_id.clone().unwrap_or_else(|| empty_value.clone())
                                            )}
                                        </div>
                                    </div>
                                    {if selected_shipping_option_id.is_some() {
                                        let clear_pending_label = pending_label.clone();
                                        let clear_locale = locale.clone();
                                        view! {
                                            <button
                                                type="button"
                                                class="inline-flex items-center justify-center rounded-full border border-border px-3 py-1.5 text-xs font-medium text-card-foreground transition hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
                                                disabled=move || busy.get()
                                                on:click={
                                                    let current_cart = current_cart.clone();
                                                    let shipping_profile_slug = shipping_profile_slug_for_clear.clone();
                                                    move |_| on_select_shipping_option.run((
                                                        current_cart.clone(),
                                                        shipping_profile_slug.clone(),
                                                        seller_id_for_clear.clone(),
                                                        seller_scope_for_clear.clone(),
                                                        None,
                                                    ))
                                                }
                                            >
                                                {move || {
                                                    if busy.get() {
                                                        clear_pending_label.clone()
                                                    } else {
                                                        t(clear_locale.as_deref(), "commerce.delivery.clear", "Clear selection")
                                                    }
                                                }}
                                            </button>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                                                {t(locale.as_deref(), "commerce.delivery.required", "selection required")}
                                            </div>
                                        }.into_any()
                                    }}
                                </div>
                                <div class="mt-4 grid gap-3">
                                    {if available_options.is_empty() {
                                        view! {
                                            <div class="rounded-2xl border border-dashed border-border px-4 py-3 text-sm text-muted-foreground">
                                                {t(locale.as_deref(), "commerce.delivery.empty", "No shipping options are currently available for this delivery group in the active storefront context.")}
                                            </div>
                                        }.into_any()
                                    } else {
                                        {available_options
                                                .into_iter()
                                                .map(|option| {
                                                    let option_locale = locale.clone();
                                                    let option_pending_label = pending_label.clone();
                                                    let current_cart = cart.clone();
                                                    let shipping_profile_slug = group.shipping_profile_slug.clone();
                                                    let seller_id = group.seller_id.clone();
                                                    let seller_scope = group.seller_scope.clone();
                                                    let option_id = option.id.clone();
                                                    let option_name = option.name.clone();
                                                    let option_label = format!(
                                                        "{} {} · {}",
                                                        option.currency_code,
                                                        option.amount,
                                                        option.provider_id
                                                    );
                                                    let is_selected = group
                                                        .selected_shipping_option_id
                                                        .as_ref()
                                                        .map(|value| value == &option_id)
                                                        .unwrap_or(false);
                                                    view! {
                                                        <div class="rounded-2xl border border-border px-4 py-3">
                                                            <div class="flex flex-wrap items-center justify-between gap-3">
                                                                <div class="space-y-1">
                                                                    <div class="text-sm font-medium text-card-foreground">{option_name}</div>
                                                                    <div class="text-xs text-muted-foreground">{option_label}</div>
                                                                </div>
                                                                <div class="flex items-center gap-2">
                                                                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                                                                        {if is_selected {
                                                                            t(option_locale.as_deref(), "commerce.delivery.current", "selected")
                                                                        } else if option.active {
                                                                            t(option_locale.as_deref(), "commerce.delivery.available", "available")
                                                                        } else {
                                                                            t(option_locale.as_deref(), "commerce.delivery.inactive", "inactive")
                                                                        }}
                                                                    </div>
                                                                    <button
                                                                        type="button"
                                                                        class="inline-flex items-center justify-center rounded-full border border-border px-3 py-1.5 text-xs font-medium text-card-foreground transition hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
                                                                        disabled=move || busy.get() || is_selected || !option.active
                                                                        on:click={
                                                                            let current_cart = current_cart.clone();
                                                                            let shipping_profile_slug = shipping_profile_slug.clone();
                                                                            let option_id = option_id.clone();
                                                                            move |_| on_select_shipping_option.run((
                                                                                current_cart.clone(),
                                                                                shipping_profile_slug.clone(),
                                                                                seller_id.clone(),
                                                                                seller_scope.clone(),
                                                                                Some(option_id.clone()),
                                                                            ))
                                                                        }
                                                                    >
                                                                        {move || {
                                                                            if busy.get() {
                                                                                option_pending_label.clone()
                                                                            } else if is_selected {
                                                                                t(option_locale.as_deref(), "commerce.delivery.current", "selected")
                                                                            } else {
                                                                                t(option_locale.as_deref(), "commerce.delivery.choose", "Choose")
                                                                            }
                                                                        }}
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    }
                                                })
                                                .collect_view().into_any()}
                                    }}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </article>
    }
}

#[component]
fn PaymentCollectionCard(
    payment_collection: Option<StorefrontCheckoutPaymentCollection>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    match payment_collection {
        Some(payment_collection) => view! {
            <article class="rounded-2xl border border-border bg-card p-5">
                <div class="flex items-center justify-between gap-3">
                    <div>
                        <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                            {t(locale.as_deref(), "commerce.payment.badge", "payment collection")}
                        </div>
                        <h4 class="mt-2 text-base font-semibold text-card-foreground">{payment_collection.id}</h4>
                    </div>
                    <div class="text-sm text-muted-foreground">{payment_collection.status.clone()}</div>
                </div>
                <div class="mt-4 grid gap-3 md:grid-cols-2">
                    <MetricCard title=t(locale.as_deref(), "commerce.payment.amount", "Amount") value=format!("{} {}", payment_collection.currency_code, payment_collection.amount) />
                    <MetricCard title=t(locale.as_deref(), "commerce.payment.authorized", "Authorized") value=payment_collection.authorized_amount />
                    <MetricCard title=t(locale.as_deref(), "commerce.payment.captured", "Captured") value=payment_collection.captured_amount />
                    <MetricCard title=t(locale.as_deref(), "commerce.payment.payments", "Payments") value=payment_collection.payment_count.to_string() />
                </div>
            </article>
        }.into_any(),
        None => view! {
            <article class="rounded-2xl border border-dashed border-border p-5">
                <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "commerce.payment.badge", "payment collection")}
                </div>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "commerce.payment.empty", "No reusable payment collection is attached to this cart yet.")}
                </p>
            </article>
        }.into_any(),
    }
}

#[component]
fn CheckoutCompletionCard(result: StorefrontCheckoutCompletion) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    view! {
        <article class="mt-6 rounded-2xl border border-primary/30 bg-primary/5 p-5">
            <div class="text-xs font-medium uppercase tracking-[0.18em] text-primary">
                {t(locale.as_deref(), "commerce.checkout.result.badge", "checkout result")}
            </div>
            <h4 class="mt-2 text-base font-semibold text-card-foreground">{result.order_id}</h4>
            <div class="mt-4 grid gap-3 md:grid-cols-2">
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.orderStatus", "Order status") value=result.order_status />
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.shipping", "Order shipping") value=format!("{} {}", result.currency_code, result.shipping_total) />
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.adjustments", "Order adjustments") value=format!("{} {}", result.currency_code, result.adjustment_total) />
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.total", "Order total") value=format!("{} {}", result.currency_code, result.total_amount) />
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.collection", "Payment collection") value=result.payment_collection_id />
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.collectionStatus", "Collection status") value=result.payment_collection_status />
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.fulfillments", "Fulfillments") value=result.fulfillment_count.to_string() />
                <MetricCard title=t(locale.as_deref(), "commerce.checkout.result.locale", "Resolved locale") value=result.context_locale />
            </div>
            <div class="mt-4">
                <AdjustmentsCard adjustments=result.adjustments />
            </div>
        </article>
    }
}

#[component]
fn AdjustmentsCard(adjustments: Vec<StorefrontCheckoutAdjustment>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    view! {
        <article class="rounded-2xl border border-border bg-card p-5">
            <div class="flex items-center justify-between gap-3">
                <h4 class="text-base font-semibold text-card-foreground">
                    {t(locale.as_deref(), "commerce.checkout.adjustments.title", "Typed adjustments")}
                </h4>
                <span class="text-sm text-muted-foreground">{adjustments.len().to_string()}</span>
            </div>
            {if adjustments.is_empty() {
                view! {
                    <p class="mt-3 text-sm text-muted-foreground">
                        {t(locale.as_deref(), "commerce.checkout.adjustments.empty", "No typed adjustments are attached to this checkout snapshot.")}
                    </p>
                }.into_any()
            } else {
                view! {
                    <div class="mt-4 space-y-3">
                        {adjustments.into_iter().map(|adjustment| {
                            let locale = locale.clone();
                            let source = adjustment.source_id.unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved"));
                            let scope = adjustment.scope.unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved"));
                            let line_item = adjustment.line_item_id.unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved"));
                            let metadata = adjustment.metadata;
                            view! {
                                <article class="rounded-2xl border border-border/70 bg-background/60 p-4">
                                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{adjustment.source_type}</div>
                                    <div class="mt-2 grid gap-2 md:grid-cols-4">
                                        <MetricCard title=t(locale.as_deref(), "commerce.checkout.adjustments.source", "Source") value=source />
                                        <MetricCard title=t(locale.as_deref(), "commerce.checkout.adjustments.scope", "Scope") value=scope />
                                        <MetricCard title=t(locale.as_deref(), "commerce.checkout.adjustments.lineItem", "Line item") value=line_item />
                                        <MetricCard title=t(locale.as_deref(), "commerce.checkout.adjustments.amount", "Amount") value=format!("{} {}", adjustment.currency_code, adjustment.amount) />
                                    </div>
                                    <div class="mt-3 rounded-2xl border border-border/60 bg-card p-3">
                                        <div class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground">
                                            {t(locale.as_deref(), "commerce.checkout.adjustments.metadata", "Metadata")}
                                        </div>
                                        <pre class="mt-2 whitespace-pre-wrap break-all text-xs text-muted-foreground">{metadata}</pre>
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
fn SurfaceRail() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let region_href = route_context.module_route_base("regions");
    let product_href = route_context.module_route_base("products");
    let pricing_href = route_context.module_route_base("pricing");
    let cart_href = route_context.module_route_base("cart");

    view! {
        <div class="space-y-4">
            <div class="space-y-2">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "commerce.surface.title", "Module-owned storefront surfaces")}
                </h3>
                <p class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "commerce.surface.subtitle", "Discovery and cart editing now belong to split modules. Commerce stays here only where flows still cross cart, region, pricing, order, and fulfillment boundaries.")}
                </p>
            </div>
            <div class="grid gap-3">
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.region.badge", "region")
                    title=t(locale.as_deref(), "commerce.surface.region.title", "Regions")
                    body=t(locale.as_deref(), "commerce.surface.region.body", "Region discovery now lives in the region-owned storefront package and owns country/currency selection.")
                    href=Some(region_href)
                />
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.product.badge", "product")
                    title=t(locale.as_deref(), "commerce.surface.product.title", "Catalog")
                    body=t(locale.as_deref(), "commerce.surface.product.body", "Published catalog discovery and product detail now live in the product-owned storefront package.")
                    href=Some(product_href)
                />
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.pricing.badge", "pricing")
                    title=t(locale.as_deref(), "commerce.surface.pricing.title", "Pricing")
                    body=t(locale.as_deref(), "commerce.surface.pricing.body", "Public pricing atlas, currency coverage, and sale markers now live in the pricing-owned storefront package.")
                    href=Some(pricing_href)
                />
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.cart.badge", "cart")
                    title=t(locale.as_deref(), "commerce.surface.cart.title", "Cart")
                    body=t(locale.as_deref(), "commerce.surface.cart.body", "Cart inspection and safe line-item decrement/remove actions now live in the cart-owned storefront package.")
                    href=Some(cart_href)
                />
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.aggregate.badge", "aggregate")
                    title=t(locale.as_deref(), "commerce.surface.aggregate.title", "Remaining aggregate scope")
                    body=t(locale.as_deref(), "commerce.surface.aggregate.body", "Payment collection, checkout completion, and delivery orchestration remain in commerce because they still coordinate multiple split modules.")
                    href=None
                />
            </div>
        </div>
    }
}

#[component]
fn SurfaceCard(badge: String, title: String, body: String, href: Option<String>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    view! {
        <article class="rounded-2xl border border-border bg-background p-5">
            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{badge}</div>
            <h4 class="mt-2 text-base font-semibold text-card-foreground">{title}</h4>
            <p class="mt-2 text-sm leading-7 text-muted-foreground">{body}</p>
            {match href {
                Some(href) => view! {
                    <div class="mt-4">
                        <a class="inline-flex text-sm font-medium text-primary hover:underline" href=href>
                            {t(locale.as_deref(), "commerce.surface.open", "Open")}
                        </a>
                    </div>
                }.into_any(),
                None => view! {
                    <div class="mt-4 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {t(locale.as_deref(), "commerce.surface.here", "stays here")}
                    </div>
                }.into_any(),
            }}
        </article>
    }
}

#[component]
fn MetricCard(title: String, value: String) -> impl IntoView {
    view! { <article class="rounded-2xl border border-border bg-card p-4"><div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{title}</div><div class="mt-2 text-lg font-semibold text-card-foreground break-all">{value}</div></article> }
}
