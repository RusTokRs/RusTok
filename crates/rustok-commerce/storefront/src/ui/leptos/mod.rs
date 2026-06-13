use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;
use rustok_cart_storefront::core::CartCheckoutHandoffLabels;
use rustok_cart_storefront::CartCheckoutHandoffCard;
use rustok_fulfillment_storefront::FulfillmentShippingHandoffNotice;
use rustok_order_storefront::core::{
    OrderCheckoutActionLabels, OrderCheckoutResultData, OrderCheckoutResultLabels,
};
use rustok_order_storefront::{OrderCheckoutCompleteButton, OrderCheckoutResultCard};
use rustok_payment_storefront::core::{
    PaymentCollectionActionLabels, PaymentCollectionCardData, PaymentCollectionCardLabels,
};
use rustok_payment_storefront::{PaymentCollectionActionButton, PaymentCollectionCard};

use crate::i18n::t;
use crate::model::{
    StorefrontCheckoutCompletion, StorefrontCheckoutPaymentCollection, StorefrontCheckoutWorkspace,
    StorefrontCommerceData,
};
use crate::{core, transport};

#[component]
pub fn CommerceView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_locale = route_context.locale.clone();
    let route_state = core::build_storefront_route_state(read_route_query_value(
        &route_context,
        core::SELECTED_CART_QUERY_KEY,
    ));
    let selected_cart_id = route_state.selected_cart_id.clone();
    let shell_view = core::build_storefront_shell_view_model(selected_locale.as_deref());
    let badge = shell_view.badge;
    let title = shell_view.title;
    let subtitle = shell_view.subtitle;
    let load_error = shell_view.load_error;
    let action_error_label = shell_view.action_error;

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
        move |(cart_id, locale, _)| async move {
            transport::fetch_storefront_commerce(core::build_fetch_commerce_request(
                cart_id, locale,
            ))
            .await
        },
    );

    let on_create_payment_collection = {
        let action_error_label = action_error_label.clone();
        Callback::new(move |cart_id: String| {
            let action_error_label = action_error_label.clone();
            set_action_busy.set(true);
            set_action_error.set(None);
            set_completion.set(None);
            spawn_local(async move {
                match transport::create_storefront_payment_collection(
                    core::build_cart_command_request(cart_id),
                )
                .await
                {
                    Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                    Err(err) => set_action_error.set(Some(core::error_with_context(
                        action_error_label.as_str(),
                        &err.to_string(),
                    ))),
                }
                set_action_busy.set(false);
            });
        })
    };

    let on_complete_checkout = {
        let action_error_label = action_error_label.clone();
        Callback::new(move |cart_id: String| {
            let action_error_label = action_error_label.clone();
            set_action_busy.set(true);
            set_action_error.set(None);
            spawn_local(async move {
                match transport::complete_storefront_checkout(core::build_cart_command_request(
                    cart_id,
                ))
                .await
                {
                    Ok(result) => {
                        set_completion.set(Some(result));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_action_error.set(Some(core::error_with_context(
                        action_error_label.as_str(),
                        &err.to_string(),
                    ))),
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
                        let on_complete_checkout = on_complete_checkout;
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! {
                                    <CommerceShowcase
                                        data
                                        busy=action_busy
                                        completion
                                        on_create_payment_collection
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
    let context_view = core::build_storefront_context_view_model(data, locale.as_deref());

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
                <MetricCard title=t(locale.as_deref(), "commerce.context.locale", "Effective locale") value=context_view.effective_locale />
                <MetricCard title=t(locale.as_deref(), "commerce.context.tenant", "Tenant") value=context_view.tenant />
                <MetricCard title=t(locale.as_deref(), "commerce.context.tenantDefault", "Tenant default locale") value=context_view.tenant_default_locale />
                <MetricCard title=t(locale.as_deref(), "commerce.context.channel", "Channel") value=context_view.channel />
            </div>
            <div class="mt-3">
                <MetricCard title=t(locale.as_deref(), "commerce.context.resolution", "Channel source") value=context_view.channel_resolution_source />
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
                    let cart_id = cart.id.clone();
                    let cart_status = cart.status.clone();
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
                                view! { <OrderCheckoutResultCard result=order_checkout_result_data(result) labels=order_checkout_result_labels(locale.as_deref()) /> }
                            })
                        }}
                        <CartCheckoutHandoffCard
                            cart_id=cart_id
                            status=cart_status
                            labels=cart_checkout_handoff_labels(locale.as_deref())
                        />
                        <FulfillmentShippingHandoffNotice
                            message=t(locale.as_deref(), "commerce.delivery.moduleOwnership", "Shipping options and fulfillment details stay in fulfillment-owned UI; commerce only triggers cross-module checkout orchestration.")
                        />
                        <div class="mt-6 grid gap-3 md:grid-cols-2">
                            <a class="inline-flex items-center justify-center rounded-full border border-border px-4 py-2 text-sm font-medium text-card-foreground transition hover:bg-muted" href=cart_href>
                                {t(locale.as_deref(), "commerce.checkout.openCart", "Open cart workspace")}
                            </a>
                            <PaymentCollectionActionButton
                                cart_id=cart.id.clone()
                                busy
                                labels=payment_collection_action_labels(locale.as_deref())
                                on_create_payment_collection
                            />
                            <OrderCheckoutCompleteButton
                                cart_id=cart.id.clone()
                                busy
                                labels=order_checkout_action_labels(locale.as_deref())
                                on_complete_checkout
                            />
                        </div>
                        <div class="mt-6">
                            <PaymentCollectionCard
                                payment_collection=payment_collection.map(payment_collection_card_data)
                                labels=payment_collection_card_labels(locale.as_deref())
                            />
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

fn cart_checkout_handoff_labels(locale: Option<&str>) -> CartCheckoutHandoffLabels {
    CartCheckoutHandoffLabels {
        cart_label: t(locale, "commerce.checkout.cart.id", "Cart"),
        status_label: t(locale, "commerce.checkout.cart.status", "Cart status"),
        module_ownership: t(
            locale,
            "commerce.checkout.cart.moduleOwnership",
            "Cart totals, line items and adjustments stay in the cart module workspace.",
        ),
    }
}

fn payment_collection_action_labels(locale: Option<&str>) -> PaymentCollectionActionLabels {
    PaymentCollectionActionLabels {
        pending: t(locale, "commerce.checkout.pending", "Processing..."),
        create_or_reuse: t(
            locale,
            "commerce.checkout.createCollection",
            "Create or reuse payment collection",
        ),
    }
}

fn order_checkout_action_labels(locale: Option<&str>) -> OrderCheckoutActionLabels {
    OrderCheckoutActionLabels {
        pending: t(locale, "commerce.checkout.pending", "Processing..."),
        complete: t(locale, "commerce.checkout.complete", "Complete checkout"),
    }
}

fn payment_collection_card_data(
    payment_collection: StorefrontCheckoutPaymentCollection,
) -> PaymentCollectionCardData {
    PaymentCollectionCardData {
        id: payment_collection.id,
        status: payment_collection.status,
    }
}

fn payment_collection_card_labels(locale: Option<&str>) -> PaymentCollectionCardLabels {
    PaymentCollectionCardLabels {
        badge: t(locale, "commerce.payment.badge", "payment collection"),
        module_ownership: t(locale, "commerce.payment.moduleOwnership", "Payment collection details stay in payment-owned UI; commerce only shows checkout orchestration handoff state."),
        empty_id: t(locale, "commerce.payment.emptyId", "not attached"),
        empty_status: t(locale, "commerce.payment.emptyStatus", "pending"),
    }
}

fn order_checkout_result_data(result: StorefrontCheckoutCompletion) -> OrderCheckoutResultData {
    OrderCheckoutResultData {
        order_id: result.order_id,
        order_status: result.order_status,
    }
}

fn order_checkout_result_labels(locale: Option<&str>) -> OrderCheckoutResultLabels {
    OrderCheckoutResultLabels {
        badge: t(locale, "commerce.checkout.result.badge", "checkout result"),
        module_ownership: t(locale, "commerce.checkout.result.moduleOwnership", "Order, payment, fulfillment and adjustment details remain in their module-owned workspaces; commerce shows only the aggregate checkout outcome."),
        order_status_label: t(locale, "commerce.checkout.result.orderStatus", "Order status"),
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
