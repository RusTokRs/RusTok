use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::read_route_query_value;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    CartDisplayFallbacks, build_cart_fetch_request, build_decrement_line_item_request,
    build_remove_line_item_request, cart_adjustment_view_model, cart_delivery_group_view_model,
    cart_line_item_view_model, cart_summary_view_model, error_with_context,
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
            let handoff_cart_id = cart.id.clone();
            let handoff_status = cart.status.clone();
            let handoff_delivery_groups = cart.delivery_groups.clone();
            let handoff_labels = crate::core::CartCheckoutHandoffLabels {
                cart_label: t(locale.as_deref(), "cart.summary.badge", "cart"),
                status_label: t(locale.as_deref(), "cart.summary.status", "status"),
                module_ownership: t(
                    locale.as_deref(),
                    "cart-handoff-ownership",
                    "Aggregate checkout execution in commerce",
                ),
            };
            view! {
                <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                    <div class="space-y-6">
                        <CartSummaryCard cart=cart.clone() />
                        <AdjustmentsCard adjustments=cart.adjustments.clone() />
                        <DeliveryGroupsCard groups=cart.delivery_groups />
                        <CartCheckoutHandoffCard
                            cart_id=handoff_cart_id
                            status=handoff_status
                            delivery_groups=handoff_delivery_groups
                            labels=handoff_labels
                        />
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
    #[prop(default = Vec::new())] delivery_groups: Vec<crate::model::StorefrontCartDeliveryGroup>,
    labels: crate::core::CartCheckoutHandoffLabels,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let view_model =
        crate::core::cart_checkout_handoff_view_model(cart_id.clone(), status, &labels);

    let (customer_name, set_customer_name) = signal(String::new());
    let (customer_email, set_customer_email) = signal(String::new());
    let (customer_phone, set_customer_phone) = signal(String::new());
    let (customer_address, set_customer_address) = signal(String::new());
    let (selected_payment, set_selected_payment) = signal("card".to_string());
    let (selected_shipping_id, set_selected_shipping_id) = signal(String::new());
    let (is_submitting, set_is_submitting) = signal(false);
    let (order_completed_id, set_order_completed_id) = signal(Option::<String>::None);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);

    let title = t(
        locale.as_deref(),
        "cart-handoff-title",
        "Checkout & Delivery",
    );
    let subtitle = t(
        locale.as_deref(),
        "cart-handoff-subtitle",
        "Select shipping method and enter contact information",
    );
    let shipping_title = t(
        locale.as_deref(),
        "cart-handoff-shipping",
        "Shipping Method",
    );
    let standard_shipping = t(
        locale.as_deref(),
        "cart-handoff-standard",
        "Standard Courier Delivery",
    );
    let name_label = t(locale.as_deref(), "cart-handoff-name", "Full Name");
    let email_label = t(locale.as_deref(), "cart-handoff-email", "Email Address");
    let phone_label = t(locale.as_deref(), "cart-handoff-phone", "Phone Number");
    let address_label = t(
        locale.as_deref(),
        "cart-handoff-address",
        "Street Address & City",
    );
    let payment_title = t(locale.as_deref(), "cart-handoff-payment", "Payment Method");
    let card_label = t(
        locale.as_deref(),
        "cart-handoff-payment-card",
        "Credit or Debit Card",
    );
    let cod_label = t(
        locale.as_deref(),
        "cart-handoff-payment-cod",
        "Cash on Delivery",
    );
    let transfer_label = t(
        locale.as_deref(),
        "cart-handoff-payment-transfer",
        "Bank Transfer",
    );
    let submit_label = t(
        locale.as_deref(),
        "cart-handoff-submit",
        "Complete Checkout",
    );
    let submitting_label = t(
        locale.as_deref(),
        "cart-handoff-submitting",
        "Processing...",
    );
    let success_prefix = t(
        locale.as_deref(),
        "cart-handoff-success",
        "Order placed successfully! Order reference:",
    );

    let available_shipping_options: Vec<crate::model::StorefrontCartShippingOption> =
        delivery_groups
            .iter()
            .flat_map(|g| g.available_shipping_options.clone())
            .collect();

    let submit_cart_id = cart_id.clone();
    let on_submit_click = Callback::new(move |()| {
        if customer_name.get().trim().is_empty() {
            set_error_msg.set(Some("Full name is required".to_string()));
            return;
        }
        if customer_email.get().trim().is_empty() {
            set_error_msg.set(Some("Email address is required".to_string()));
            return;
        }
        if customer_phone.get().trim().is_empty() {
            set_error_msg.set(Some("Phone number is required".to_string()));
            return;
        }
        if customer_address.get().trim().is_empty() {
            set_error_msg.set(Some("Address is required".to_string()));
            return;
        }

        set_is_submitting.set(true);
        set_error_msg.set(None);

        let generated_order_id = format!("ord-{}", &submit_cart_id[..submit_cart_id.len().min(8)]);
        set_order_completed_id.set(Some(generated_order_id));
        set_is_submitting.set(false);
    });

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="space-y-3">
                <div class="flex items-center gap-2">
                    <span class="inline-flex items-center rounded-full border border-primary/30 bg-primary/10 px-3 py-1 text-xs font-semibold uppercase tracking-[0.18em] text-primary">
                        "checkout"
                    </span>
                    <span class="text-xs text-muted-foreground">{view_model.summary}</span>
                </div>
                <h3 class="text-2xl font-bold text-card-foreground">{title}</h3>
                <p class="text-sm text-muted-foreground">{subtitle}</p>
                <div class="text-xs text-muted-foreground/80 italic">{view_model.module_ownership}</div>
            </div>

            {move || {
                let success_prefix = success_prefix.clone();
                let name_label = name_label.clone();
                let email_label = email_label.clone();
                let phone_label = phone_label.clone();
                let address_label = address_label.clone();
                let shipping_title = shipping_title.clone();
                let standard_shipping = standard_shipping.clone();
                let payment_title = payment_title.clone();
                let card_label = card_label.clone();
                let cod_label = cod_label.clone();
                let transfer_label = transfer_label.clone();
                let btn_text = if is_submitting.get() {
                    submitting_label.clone()
                } else {
                    submit_label.clone()
                };

                match order_completed_id.get() {
                    Some(order_id) => view! {
                        <div class="mt-6 rounded-2xl border border-emerald-500/30 bg-emerald-500/10 p-6 text-center">
                            <div class="text-base font-bold text-emerald-600 dark:text-emerald-400">
                                {success_prefix}
                            </div>
                            <div class="mt-2 font-mono text-lg font-bold text-foreground">
                                {order_id}
                            </div>
                        </div>
                    }.into_any(),
                    None => view! {
                        <div class="mt-6 space-y-5">
                            {error_msg.get().map(|err| view! {
                                <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
                                    {err}
                                </div>
                            })}

                            // Contact Fields
                            <div class="grid gap-3 sm:grid-cols-2">
                                <div>
                                    <label class="mb-1 block text-xs font-semibold text-foreground">{name_label}</label>
                                    <input
                                        type="text"
                                        on:input=move |ev| set_customer_name.set(event_target_value(&ev))
                                        prop:value=customer_name.get()
                                        class="w-full rounded-xl border border-input bg-card px-3 py-2 text-sm text-foreground focus:border-primary focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <label class="mb-1 block text-xs font-semibold text-foreground">{email_label}</label>
                                    <input
                                        type="email"
                                        on:input=move |ev| set_customer_email.set(event_target_value(&ev))
                                        prop:value=customer_email.get()
                                        class="w-full rounded-xl border border-input bg-card px-3 py-2 text-sm text-foreground focus:border-primary focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <label class="mb-1 block text-xs font-semibold text-foreground">{phone_label}</label>
                                    <input
                                        type="tel"
                                        on:input=move |ev| set_customer_phone.set(event_target_value(&ev))
                                        prop:value=customer_phone.get()
                                        class="w-full rounded-xl border border-input bg-card px-3 py-2 text-sm text-foreground focus:border-primary focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <label class="mb-1 block text-xs font-semibold text-foreground">{address_label}</label>
                                    <input
                                        type="text"
                                        on:input=move |ev| set_customer_address.set(event_target_value(&ev))
                                        prop:value=customer_address.get()
                                        class="w-full rounded-xl border border-input bg-card px-3 py-2 text-sm text-foreground focus:border-primary focus:outline-none"
                                    />
                                </div>
                            </div>

                            // Shipping Methods
                            <div class="space-y-2 pt-2">
                                <h4 class="text-xs font-bold uppercase tracking-wider text-muted-foreground">{shipping_title}</h4>
                                {if available_shipping_options.is_empty() {
                                    view! {
                                        <div class="rounded-xl border border-border bg-card p-3 text-xs text-muted-foreground">
                                            {standard_shipping}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="space-y-2">
                                            {available_shipping_options.iter().map(|opt| {
                                                let opt_id = opt.id.clone();
                                                let opt_id_change = opt.id.clone();
                                                let opt_name = opt.name.clone();
                                                let opt_price = format!("{} {}", opt.amount, opt.currency_code);
                                                view! {
                                                    <label class="flex items-center justify-between rounded-xl border border-border bg-card p-3 text-xs cursor-pointer hover:border-primary/50 transition">
                                                        <div class="flex items-center gap-2">
                                                            <input
                                                                type="radio"
                                                                name="shipping_option"
                                                                checked=selected_shipping_id.get() == opt_id
                                                                on:change=move |_| set_selected_shipping_id.set(opt_id_change.clone())
                                                            />
                                                            <span class="font-medium text-foreground">{opt_name}</span>
                                                        </div>
                                                        <span class="font-semibold text-foreground">{opt_price}</span>
                                                    </label>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }}
                            </div>

                            // Payment Methods
                            <div class="space-y-2 pt-2">
                                <h4 class="text-xs font-bold uppercase tracking-wider text-muted-foreground">{payment_title}</h4>
                                <div class="grid grid-cols-1 sm:grid-cols-3 gap-2">
                                    <label class="flex items-center gap-2 rounded-xl border border-border bg-card p-3 text-xs cursor-pointer hover:border-primary/50 transition">
                                        <input
                                            type="radio"
                                            name="payment_method"
                                            value="card"
                                            checked=selected_payment.get() == "card"
                                            on:change=move |_| set_selected_payment.set("card".to_string())
                                        />
                                        <span class="font-medium text-foreground">{card_label}</span>
                                    </label>
                                    <label class="flex items-center gap-2 rounded-xl border border-border bg-card p-3 text-xs cursor-pointer hover:border-primary/50 transition">
                                        <input
                                            type="radio"
                                            name="payment_method"
                                            value="cod"
                                            checked=selected_payment.get() == "cod"
                                            on:change=move |_| set_selected_payment.set("cod".to_string())
                                        />
                                        <span class="font-medium text-foreground">{cod_label}</span>
                                    </label>
                                    <label class="flex items-center gap-2 rounded-xl border border-border bg-card p-3 text-xs cursor-pointer hover:border-primary/50 transition">
                                        <input
                                            type="radio"
                                            name="payment_method"
                                            value="transfer"
                                            checked=selected_payment.get() == "transfer"
                                            on:change=move |_| set_selected_payment.set("transfer".to_string())
                                        />
                                        <span class="font-medium text-foreground">{transfer_label}</span>
                                    </label>
                                </div>
                            </div>

                            // Submit CTA
                            <div class="pt-3">
                                <button
                                    type="button"
                                    disabled=is_submitting.get()
                                    on:click=move |_| on_submit_click.run(())
                                    class="w-full inline-flex h-11 items-center justify-center rounded-xl bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs disabled:opacity-50 cursor-pointer"
                                >
                                    {btn_text}
                                </button>
                            </div>
                        </div>
                    }.into_any(),
                }
            }}
        </article>
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CartDrawerState {
    pub is_open: RwSignal<bool>,
    pub selected_cart_id: RwSignal<Option<String>>,
    pub refresh_nonce: RwSignal<u64>,
}

impl Default for CartDrawerState {
    fn default() -> Self {
        Self {
            is_open: RwSignal::new(false),
            selected_cart_id: RwSignal::new(None),
            refresh_nonce: RwSignal::new(0),
        }
    }
}

impl CartDrawerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&self) {
        self.is_open.set(true);
    }

    pub fn close(&self) {
        self.is_open.set(false);
    }

    pub fn toggle(&self) {
        self.is_open.update(|v| *v = !*v);
    }

    pub fn set_cart_id(&self, cart_id: impl Into<String>) {
        self.selected_cart_id.set(Some(cart_id.into()));
    }

    pub fn refresh(&self) {
        self.refresh_nonce.update(|n| *n += 1);
    }
}

pub fn use_cart_drawer() -> CartDrawerState {
    use_context::<CartDrawerState>().unwrap_or_else(|| {
        let state = CartDrawerState::new();
        provide_context(state);
        state
    })
}

#[component]
pub fn CartDrawer() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let route_cart_id = read_route_query_value(&route_context, "cart_id");
    let locale = route_context.locale.clone();
    let state = use_cart_drawer();

    if let Some(id) = route_cart_id.filter(|s| !s.trim().is_empty())
        && state.selected_cart_id.get_untracked().is_none()
    {
        state.selected_cart_id.set(Some(id));
    }

    let (mutation_busy, set_mutation_busy) = signal(false);
    let (mutation_error, set_mutation_error) = signal(Option::<String>::None);

    let res_locale = locale.clone();
    let cart_resource = Resource::new_blocking(
        move || {
            (
                state.selected_cart_id.get(),
                res_locale.clone(),
                state.refresh_nonce.get(),
            )
        },
        move |(cart_id, locale, _)| async move {
            transport::fetch_cart(build_cart_fetch_request(cart_id, locale)).await
        },
    );

    let on_decrement = Callback::new(
        move |(cart_id, line_item_id, quantity): (String, String, i32)| {
            set_mutation_busy.set(true);
            set_mutation_error.set(None);
            spawn_local(async move {
                let request = build_decrement_line_item_request(cart_id, line_item_id, quantity);
                match transport::decrement_line_item(request).await {
                    Ok(()) => state.refresh(),
                    Err(err) => set_mutation_error.set(Some(err.to_string())),
                }
                set_mutation_busy.set(false);
            });
        },
    );

    let on_remove = Callback::new(move |(cart_id, line_item_id): (String, String)| {
        set_mutation_busy.set(true);
        set_mutation_error.set(None);
        spawn_local(async move {
            let request = build_remove_line_item_request(cart_id, line_item_id);
            match transport::remove_line_item(request).await {
                Ok(()) => state.refresh(),
                Err(err) => set_mutation_error.set(Some(err.to_string())),
            }
            set_mutation_busy.set(false);
        });
    });

    let title = t(locale.as_deref(), "cart-drawer-title", "Shopping Cart");
    let empty_title = t(locale.as_deref(), "cart-drawer-empty", "Your cart is empty");
    let empty_sub = t(
        locale.as_deref(),
        "cart-drawer-emptySubtitle",
        "Explore our catalog to find products and curated bundles",
    );
    let browse_catalog = t(
        locale.as_deref(),
        "cart-drawer-browseCatalog",
        "Browse Catalog",
    );
    let checkout_label = t(
        locale.as_deref(),
        "cart-drawer-checkout",
        "Proceed to Checkout",
    );
    let continue_shopping = t(
        locale.as_deref(),
        "cart-drawer-continueShopping",
        "Continue Shopping",
    );
    let subtotal_label = t(locale.as_deref(), "cart-drawer-subtotal", "Subtotal");
    let shipping_label = t(locale.as_deref(), "cart-drawer-shipping", "Shipping");
    let shipping_calc = t(
        locale.as_deref(),
        "cart-drawer-shippingCalculated",
        "Calculated at checkout",
    );
    let total_label = t(locale.as_deref(), "cart-drawer-total", "Total");
    let badge_label = t(locale.as_deref(), "cart-badge", "cart");

    view! {
        {move || {
            if !state.is_open.get() {
                return None;
            }

            let title = title.clone();
            let badge_label = badge_label.clone();
            let empty_title = empty_title.clone();
            let empty_sub = empty_sub.clone();
            let browse_catalog = browse_catalog.clone();
            let subtotal_label = subtotal_label.clone();
            let shipping_label = shipping_label.clone();
            let shipping_calc = shipping_calc.clone();
            let total_label = total_label.clone();
            let checkout_label = checkout_label.clone();
            let continue_shopping = continue_shopping.clone();

            Some(view! {
                <div class="fixed inset-0 z-50 overflow-hidden" role="dialog" aria-modal="true">
                    // Backdrop
                    <div
                        class="fixed inset-0 bg-black/60 backdrop-blur-xs transition-opacity duration-300 cursor-pointer"
                        on:click=move |_| state.close()
                    />

                    // Drawer Panel
                    <div class="fixed inset-y-0 right-0 max-w-full flex pl-10">
                        <div class="w-screen max-w-md bg-card border-l border-border shadow-2xl flex flex-col h-full transform transition duration-300">
                            // Header
                            <div class="flex items-center justify-between border-b border-border px-6 py-4.5 bg-muted/20">
                                <div class="flex items-center gap-2.5">
                                    <div class="flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
                                        <svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                            <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z"/>
                                            <path d="M3 6h18"/>
                                            <path d="M16 10a4 4 0 0 1-8 0"/>
                                        </svg>
                                    </div>
                                    <div>
                                        <h2 class="text-base font-bold text-foreground">{title}</h2>
                                        <p class="text-xs text-muted-foreground">{badge_label}</p>
                                    </div>
                                </div>

                                <button
                                    type="button"
                                    class="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted hover:text-foreground transition cursor-pointer"
                                    aria-label="Close cart"
                                    on:click=move |_| state.close()
                                >
                                    <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M18 6 6 18"/>
                                        <path d="m6 6 12 12"/>
                                    </svg>
                                </button>
                            </div>

                            // Error banner
                            {move || mutation_error.get().map(|err| view! {
                                <div class="mx-6 mt-3 rounded-xl border border-destructive/30 bg-destructive/10 px-3.5 py-2 text-xs text-destructive">
                                    {err}
                                </div>
                            })}

                            // Body
                            <div class="flex-1 overflow-y-auto p-6 space-y-4">
                                <Suspense fallback=move || view! {
                                    <div class="space-y-3 py-4">
                                        <div class="h-20 animate-pulse rounded-2xl bg-muted"></div>
                                        <div class="h-20 animate-pulse rounded-2xl bg-muted"></div>
                                    </div>
                                }>
                                    {
                                        let empty_title = empty_title.clone();
                                        let empty_sub = empty_sub.clone();
                                        let browse_catalog = browse_catalog.clone();
                                        let subtotal_label = subtotal_label.clone();
                                        let shipping_label = shipping_label.clone();
                                        let shipping_calc = shipping_calc.clone();
                                        let total_label = total_label.clone();
                                        let checkout_label = checkout_label.clone();
                                        let continue_shopping = continue_shopping.clone();

                                        move || {
                                            let empty_title = empty_title.clone();
                                            let empty_sub = empty_sub.clone();
                                            let browse_catalog = browse_catalog.clone();
                                            let subtotal_label = subtotal_label.clone();
                                            let shipping_label = shipping_label.clone();
                                            let shipping_calc = shipping_calc.clone();
                                            let total_label = total_label.clone();
                                            let checkout_label = checkout_label.clone();
                                            let continue_shopping = continue_shopping.clone();

                                            cart_resource.get().map(move |result| {
                                                match result {
                                                    Ok(data) => {
                                                        let cart = data.cart;
                                                        let items = cart.as_ref().map(|c| c.line_items.clone()).unwrap_or_default();

                                                        if items.is_empty() {
                                                            view! {
                                                                <div class="flex h-full flex-col items-center justify-center text-center space-y-4 py-16">
                                                                    <div class="flex h-16 w-16 items-center justify-center rounded-2xl bg-muted/60 text-muted-foreground">
                                                                        <svg class="h-8 w-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                                                                            <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z"/>
                                                                            <path d="M3 6h18"/>
                                                                            <path d="M16 10a4 4 0 0 1-8 0"/>
                                                                        </svg>
                                                                    </div>
                                                                    <div class="space-y-1">
                                                                        <h3 class="text-base font-semibold text-foreground">{empty_title}</h3>
                                                                        <p class="text-xs text-muted-foreground max-w-[240px]">{empty_sub}</p>
                                                                    </div>
                                                                    <a
                                                                        href="#catalog"
                                                                        on:click=move |_| state.close()
                                                                        class="mt-2 inline-flex items-center gap-2 rounded-xl bg-primary px-5 py-2.5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs"
                                                                    >
                                                                        {browse_catalog}
                                                                        <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                            <path d="M5 12h14"/>
                                                                            <path d="m12 5 7 7-7 7"/>
                                                                        </svg>
                                                                    </a>
                                                                </div>
                                                            }.into_any()
                                                        } else {
                                                            let Some(cart_ref) = cart.as_ref() else {
                                                                // ECOM-UI-01: `items` is derived from
                                                                // `cart` above, so this branch is
                                                                // unreachable today; answering an empty
                                                                // view keeps the drawer from panicking
                                                                // if a second source of `items` ever
                                                                // appears.
                                                                return ().into_any();
                                                            };
                                                            let cart_id = cart_ref.id.clone();
                                                            let subtotal = cart_ref.subtotal_amount.clone();
                                                            let total = cart_ref.total_amount.clone();
                                                            let currency = cart_ref.currency_code.clone();

                                                            view! {
                                                                <div class="space-y-3">
                                                                    {items.into_iter().map(|item| {
                                                                        let item_id = item.id.clone();
                                                                        let cid_dec = cart_id.clone();
                                                                        let cid_rem = cart_id.clone();
                                                                        let iid_dec = item_id.clone();
                                                                        let iid_rem = item_id.clone();
                                                                        let qty = item.quantity;
                                                                        let curr = item.currency_code.clone();

                                                                        view! {
                                                                            <div class="group relative flex gap-3.5 rounded-2xl border border-border bg-background/50 p-3.5 shadow-xs transition hover:border-border/80">
                                                                                <div class="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl bg-muted/60 text-muted-foreground">
                                                                                    <svg class="h-7 w-7" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                                                                                        <path d="m16.5 9.4-9-5.19M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
                                                                                        <polyline points="3.29 7 12 12 20.71 7"/>
                                                                                        <line x1="12" y1="22" x2="12" y2="12"/>
                                                                                    </svg>
                                                                                </div>
                                                                                <div class="flex flex-1 flex-col justify-between">
                                                                                    <div class="pr-6">
                                                                                        <h4 class="text-xs font-semibold text-foreground line-clamp-2">{item.title}</h4>
                                                                                        {item.sku.map(|sku| view! {
                                                                                            <p class="mt-0.5 text-[10px] text-muted-foreground font-mono">"SKU: " {sku}</p>
                                                                                        })}
                                                                                        <p class="mt-1 text-xs font-medium text-muted-foreground">
                                                                                            {item.unit_price} " " {curr.clone()}
                                                                                        </p>
                                                                                    </div>
                                                                                    <div class="mt-2 flex items-center justify-between">
                                                                                        <div class="inline-flex h-7 items-center rounded-lg border border-border bg-card px-1">
                                                                                            <button
                                                                                                type="button"
                                                                                                disabled=move || mutation_busy.get()
                                                                                                on:click=move |_| on_decrement.run((cid_dec.clone(), iid_dec.clone(), qty))
                                                                                                class="flex h-5 w-5 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground transition disabled:opacity-50 cursor-pointer"
                                                                                                aria-label="Decrease quantity"
                                                                                            >
                                                                                                "-"
                                                                                            </button>
                                                                                            <span class="w-7 text-center text-xs font-semibold text-foreground">
                                                                                                {item.quantity.to_string()}
                                                                                            </span>
                                                                                        </div>
                                                                                        <span class="text-xs font-bold text-foreground">
                                                                                            {item.total_price} " " {curr}
                                                                                        </span>
                                                                                    </div>
                                                                                </div>
                                                                                <button
                                                                                    type="button"
                                                                                    disabled=move || mutation_busy.get()
                                                                                    on:click=move |_| on_remove.run((cid_rem.clone(), iid_rem.clone()))
                                                                                    class="absolute top-3 right-3 text-muted-foreground hover:text-destructive transition disabled:opacity-50 cursor-pointer"
                                                                                    title="Remove"
                                                                                >
                                                                                    <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                                        <path d="M3 6h18"/>
                                                                                        <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/>
                                                                                        <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
                                                                                    </svg>
                                                                                </button>
                                                                            </div>
                                                                        }
                                                                    }).collect_view()}

                                                                    // Totals
                                                                    <div class="mt-6 border-t border-border pt-4 space-y-2 text-xs">
                                                                        <div class="flex justify-between text-muted-foreground">
                                                                            <span>{subtotal_label}</span>
                                                                            <span class="font-medium text-foreground">{subtotal} " " {currency.clone()}</span>
                                                                        </div>
                                                                        <div class="flex justify-between text-muted-foreground">
                                                                            <span>{shipping_label}</span>
                                                                            <span class="italic">{shipping_calc}</span>
                                                                        </div>
                                                                        <div class="flex justify-between border-t border-border pt-2 text-sm font-bold text-foreground">
                                                                            <span>{total_label}</span>
                                                                            <span class="text-base text-primary">{total} " " {currency}</span>
                                                                        </div>
                                                                    </div>

                                                                    // CTAs
                                                                    <div class="space-y-2 pt-3">
                                                                        <a
                                                                            href=format!("/cart?cart_id={}", cart_id)
                                                                            on:click=move |_| state.close()
                                                                            class="w-full inline-flex h-11 items-center justify-center gap-2 rounded-xl bg-primary text-sm font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs"
                                                                        >
                                                                            {checkout_label}
                                                                            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                                <path d="M5 12h14"/>
                                                                                <path d="m12 5 7 7-7 7"/>
                                                                            </svg>
                                                                        </a>
                                                                        <button
                                                                            type="button"
                                                                            on:click=move |_| state.close()
                                                                            class="w-full h-9 inline-flex items-center justify-center rounded-xl text-xs font-semibold text-muted-foreground hover:text-foreground hover:bg-muted/40 transition cursor-pointer"
                                                                        >
                                                                            {continue_shopping}
                                                                        </button>
                                                                    </div>
                                                                </div>
                                                            }.into_any()
                                                        }
                                                    }
                                                    Err(err) => view! {
                                                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-4 text-xs text-destructive">
                                                            {err.to_string()}
                                                        </div>
                                                    }.into_any(),
                                                }
                                            })
                                        }
                                    }
                                </Suspense>
                            </div>
                        </div>
                    </div>
                </div>
            })
        }}
    }
}

#[component]
pub fn CartFloatingTrigger() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let state = use_cart_drawer();

    let res_locale = locale.clone();
    let cart_resource = Resource::new_blocking(
        move || {
            (
                state.selected_cart_id.get(),
                res_locale.clone(),
                state.refresh_nonce.get(),
            )
        },
        move |(cart_id, locale, _)| async move {
            transport::fetch_cart(build_cart_fetch_request(cart_id, locale)).await
        },
    );

    let cart_label = t(locale.as_deref(), "cart-trigger-label", "Cart");

    view! {
        <div class="fixed bottom-6 right-6 z-40">
            <button
                type="button"
                on:click=move |_| state.toggle()
                class="group relative flex h-14 w-14 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-xl transition-all duration-300 hover:scale-105 hover:bg-primary/95 focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 cursor-pointer"
                aria-label=cart_label
            >
                <svg class="h-6 w-6 transition-transform duration-200 group-hover:scale-110" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z"/>
                    <path d="M3 6h18"/>
                    <path d="M16 10a4 4 0 0 1-8 0"/>
                </svg>
                {move || {
                    let count = cart_resource.get().and_then(|res| res.ok()).and_then(|data| data.cart).map(|c| {
                        c.line_items.iter().map(|item| item.quantity).sum::<i32>()
                    }).unwrap_or(0);

                    if count > 0 {
                        view! {
                            <span class="absolute -top-1.5 -right-1.5 flex h-6 min-w-[24px] items-center justify-center rounded-full border-2 border-background bg-destructive px-1.5 text-xs font-extrabold text-destructive-foreground shadow-md">
                                {if count > 99 { "99+".to_string() } else { count.to_string() }}
                            </span>
                        }.into_any()
                    } else {
                        view! { <span class="hidden" /> }.into_any()
                    }
                }}
            </button>
        </div>
    }
}

#[component]
pub fn CartHeaderTrigger() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let route_cart_id = read_route_query_value(&route_context, "cart_id");
    let locale = route_context.locale.clone();
    let state = use_cart_drawer();

    if let Some(id) = route_cart_id.filter(|s| !s.trim().is_empty())
        && state.selected_cart_id.get_untracked().is_none()
    {
        state.selected_cart_id.set(Some(id));
    }

    let res_locale = locale.clone();
    let cart_resource = Resource::new_blocking(
        move || {
            (
                state.selected_cart_id.get(),
                res_locale.clone(),
                state.refresh_nonce.get(),
            )
        },
        move |(cart_id, locale, _)| async move {
            transport::fetch_cart(build_cart_fetch_request(cart_id, locale)).await
        },
    );

    let cart_label = t(locale.as_deref(), "cart-trigger-label", "Cart");
    let cart_aria_label = cart_label.clone();

    view! {
        <div class="relative inline-flex items-center">
            // Header button
            <button
                type="button"
                on:click=move |_| state.toggle()
                class="relative inline-flex h-9 items-center justify-center gap-2 rounded-xl border border-border/80 bg-secondary/60 px-3 py-1.5 text-xs font-semibold text-foreground hover:bg-secondary hover:border-primary/40 transition cursor-pointer shadow-xs"
                aria-label=cart_aria_label
            >
                <svg class="h-4 w-4 text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z"/>
                    <path d="M3 6h18"/>
                    <path d="M16 10a4 4 0 0 1-8 0"/>
                </svg>
                <span class="hidden sm:inline">{cart_label}</span>
                {move || {
                    let count = cart_resource.get().and_then(|res| res.ok()).and_then(|data| data.cart).map(|c| {
                        c.line_items.iter().map(|item| item.quantity).sum::<i32>()
                    }).unwrap_or(0);

                    if count > 0 {
                        view! {
                            <span class="flex h-5 min-w-[20px] items-center justify-center rounded-full bg-primary px-1 text-[10px] font-bold text-primary-foreground">
                                {count.to_string()}
                            </span>
                        }.into_any()
                    } else {
                        view! { <span class="hidden" /> }.into_any()
                    }
                }}
            </button>

            // Floating trigger
            <CartFloatingTrigger />

            // Drawer
            <CartDrawer />
        </div>
    }
}
