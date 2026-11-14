//! Framework bindings only: Product owns selection/quotes, Cart owns its buyer lifecycle.

use crate::core::buyer::{self, BuyerInput, BuyerQuoteCycle, ProductBuyerSnapshot};
use leptos::prelude::*;
use rustok_ui::{ButtonVariant, SelectOption};
use rustok_ui_leptos::{Alert, Button, Input, Label, Select};

#[component]
pub(super) fn ProductBuyerIsland(snapshot: ProductBuyerSnapshot) -> impl IntoView {
    let locale = snapshot.route.locale.clone();
    let encoded = match buyer::encode_snapshot(&snapshot) {
        Ok(encoded) => encoded,
        Err(error) => {
            return view! { <Alert>{buyer::error_message(error, locale.as_deref())}</Alert> }
                .into_any();
        }
    };
    let tenant_slug = snapshot.tenant_slug.clone();
    view! {
        <div data-product-buyer-island="true" data-product-buyer-snapshot=encoded data-product-buyer-client-error-label=buyer::labels(locale.as_deref()).javascript>
            <leptos_auth::AuthProvider tenant_slug=tenant_slug.clone()><ProductBuyerInteraction snapshot /></leptos_auth::AuthProvider>
        </div>
    }.into_any()
}

fn change_input(input: RwSignal<BuyerInput>, generation: RwSignal<u64>, next: BuyerInput) {
    // Invalidate synchronously, including A -> B -> A and same-key retries before any effect runs.
    generation.update(|value| *value = value.saturating_add(1));
    input.set(next);
}

#[component]
pub(super) fn ProductBuyerInteraction(snapshot: ProductBuyerSnapshot) -> impl IntoView {
    let initial = match buyer::build_input(&snapshot.product, &snapshot.route, &snapshot.input, &snapshot.handle) {
        Ok(input) => input,
        Err(error) => return view! { <Alert>{buyer::error_message(error, snapshot.route.locale.as_deref())}</Alert> }.into_any(),
    };
    let labels = StoredValue::new(buyer::labels(snapshot.route.locale.as_deref()));
    let base = snapshot
        .route
        .module_route_base(&crate::core::resolve_route_segment(
            snapshot.route.route_segment.as_deref(),
        ));
    let base = StoredValue::new(base);
    let snapshot = StoredValue::new(snapshot);
    let input = RwSignal::new(initial);
    let generation = RwSignal::new(0_u64);
    let link_error = RwSignal::new(false);
    let saved_href = RwSignal::new(None::<(BuyerQuoteCycle, String, String)>);
    let interactive = RwSignal::new(cfg!(target_arch = "wasm32"));
    // A detached or server-rendered buyer has no host capability and stays read-only.
    // The mounted host injects the same Cart controller used by its drawer and landing bridge.
    let cart = use_context::<rustok_cart_storefront::BuyerCartController>().unwrap_or_else(|| {
        rustok_cart_storefront::use_buyer_cart(
            snapshot.with_value(|snapshot| snapshot.route.query.get("cart_id").cloned()),
            snapshot.with_value(|snapshot| snapshot.input.locale.clone()),
            snapshot.with_value(|snapshot| snapshot.tenant_slug.clone()),
            rustok_cart_storefront::CartCapability::new(false),
            Callback::new(|_| false),
            Callback::new(|_| None),
        )
    });
    let cycle = Memo::new(move |_| BuyerQuoteCycle {
        input: generation.get(),
        scope: cart.generation(),
        capability: cart.capability_generation(),
        auth: cart.auth_generation(),
    });
    let quote = Resource::new(
        move || {
            (
                if generation.get() < u64::MAX
                    && (cfg!(feature = "ssr") || cart.quote_context_ready())
                {
                    snapshot.with_value(|snapshot| {
                        buyer::quote_request(&snapshot.product, &input.get())
                    })
                } else {
                    Ok(None)
                },
                cycle.get(),
            )
        },
        move |(request, quote_cycle)| async move {
            match request {
                Ok(Some(request)) => {
                    // A resource fetch can be polled after its input or shared host changes.
                    // Do not transmit or publish a quote for an earlier buyer cycle.
                    if cfg!(target_arch = "wasm32")
                        && (cycle.try_get_untracked() != Some(quote_cycle)
                            || !cart.quote_context_ready_untracked())
                    {
                        return None;
                    }
                    let result = crate::transport::fetch_buyer_quote(request.clone())
                        .await
                        .map_err(|error| error.to_string());
                    if cfg!(target_arch = "wasm32")
                        && (cycle.try_get_untracked() != Some(quote_cycle)
                            || !cart.quote_context_ready_untracked())
                    {
                        return None;
                    }
                    snapshot.try_with_value(|snapshot| {
                        buyer::observe_quote(&snapshot.product, request, quote_cycle, result)
                    })
                }
                Ok(None) | Err(_) => None,
            }
        },
    );
    let view_model = Memo::new(move |_| {
        snapshot.with_value(|snapshot| {
            buyer::build_view(
                &snapshot.product,
                &input.get(),
                cycle.get(),
                quote.get().flatten().as_ref(),
                snapshot.route.locale.as_deref(),
            )
        })
    });
    let href = Memo::new(move |_| {
        snapshot.with_value(|snapshot| {
            buyer::selection_with_cart_href(
                &base.get_value(),
                &snapshot.route,
                &snapshot.product,
                &input.get(),
                cart.reference(),
            )
        })
    });
    Effect::new(move |_| {
        let href = href.get();
        let selection_cycle = cycle.get();
        if interactive.get() && cart.quote_context_ready() {
            let saved = cart.persist_selection(href.clone());
            let still_current = cycle.try_get_untracked() == Some(selection_cycle)
                && cart.quote_context_ready_untracked();
            // The host callback may synchronously dispose this Product owner.
            let _ = link_error.try_set(saved.is_none() || !still_current);
            let _ = saved_href.try_set(
                saved
                    .filter(|_| still_current)
                    .map(|actual| (selection_cycle, href, actual)),
            );
        }
    });
    let locked = Signal::derive(move || !interactive.get() || cart.busy.try_get().unwrap_or(true));
    let (quantity, set_quantity) = signal(input.get_untracked().quantity_text);
    let (currency, set_currency) = signal(
        input
            .get_untracked()
            .request
            .currency_code
            .unwrap_or_default(),
    );
    Effect::new(move |_| {
        let current = input.get();
        if quantity.get_untracked() != current.quantity_text {
            set_quantity.set(current.quantity_text);
        }
        let current = current.request.currency_code.unwrap_or_default();
        if currency.get_untracked() != current {
            set_currency.set(current);
        }
    });
    let currency_options = Signal::derive(move || {
        let mut options = vec![SelectOption::new("", labels.get_value().not_selected)];
        options.extend(
            view_model
                .get()
                .currencies
                .into_iter()
                .map(|currency| SelectOption::new(currency.clone(), currency)),
        );
        options
    });
    let purchase_disabled = Signal::derive(move || {
        !interactive.get()
            || generation.get() == u64::MAX
            || link_error.get()
            || saved_href
                .get()
                .as_ref()
                .is_none_or(|(saved_cycle, saved, _)| {
                    *saved_cycle != cycle.get() || saved != &href.get()
                })
            || !cart.can_write()
            || !view_model.get().can_purchase
    });
    let added_cycle = RwSignal::new(None::<BuyerQuoteCycle>);
    let add = move || {
        if purchase_disabled.get_untracked() {
            return;
        }
        let view = view_model.get_untracked();
        let Some(request) = view.request else {
            return;
        };
        let Some(observation) = quote.get_untracked().flatten() else {
            return;
        };
        let Some(context) = observation.quote.and_then(|quote| quote.resolution_context) else {
            return;
        };
        let submitted_cycle = cycle.get_untracked();
        leptos::task::spawn_local(async move {
            // spawn_local may start after another input/auth/policy transition. Do not send
            // a quote selected by a previous cycle, even when the eventual values look equal.
            if cycle.try_get_untracked() != Some(submitted_cycle)
                || purchase_disabled.try_get_untracked() != Some(false)
            {
                return;
            }
            if cart
                .add_variant(request.variant_id, context, request.locale)
                .await
            {
                let _ = added_cycle.try_set(Some(submitted_cycle));
            }
        });
    };
    view! {
        <section class="mt-6 space-y-4 border-t border-border pt-5" data-product-buyer="true" aria-busy=move || quote.get().is_none() || cart.busy.try_get().unwrap_or(true)>
            {move || (!interactive.get()).then(|| view! { <Alert>{labels.get_value().javascript}</Alert> })}
            <div class="space-y-3">
                <For each=move || snapshot.with_value(|snapshot| snapshot.product.variant_axes.clone()) key=|axis| axis.attribute_id.clone()
                    children=move |axis| view! { <BuyerAxisControl snapshot input generation attribute_id=axis.attribute_id label=axis.name locked /> } />
            </div>
            <div class="grid gap-3 sm:grid-cols-2">
                <div class="space-y-2" on:change=move |event| change_input(input, generation, buyer::change_currency(&input.get_untracked(), event_target_value(&event)))>
                    <Label r#for="product-buyer-currency">{labels.get_value().currency}</Label>
                    <Select id="product-buyer-currency" options=currency_options value=currency set_value=set_currency disabled=locked />
                </div>
                <div class="space-y-2" on:input=move |event| {
                    change_input(input, generation, buyer::change_quantity(&input.get_untracked(), event_target_value(&event)));
                }>
                    <Label r#for="product-buyer-quantity">{labels.get_value().quantity}</Label>
                    <Input id="product-buyer-quantity" input_mode="numeric" described_by="product-buyer-status" value=quantity set_value=set_quantity
                        invalid=Signal::derive(move || !view_model.get().quantity_valid) disabled=locked />
                </div>
            </div>
            <Suspense fallback=|| view! { <p aria-live="polite">"…"</p> }>
                {move || Suspend::new(async move {
                    quote.await;
                    view! { <div aria-live="polite" aria-atomic="true" class="space-y-2">
                    {move || view_model.get().selected_sku.map(|sku| view! { <p class="text-sm">{labels.get_value().sku}": "<span class="font-mono">{sku}</span></p> })}
                    {move || view_model.get().price.map(|price| view! { <p class="text-2xl font-semibold">{price}</p> })}
                    <p id="product-buyer-status" class="text-sm text-muted-foreground">{move || view_model.get().status}</p>
                    {move || view_model.get().observed_at.map(|time| view! { <p class="text-xs text-muted-foreground">{labels.get_value().observation}" "<time datetime=time.clone()>{time.clone()}</time></p> })}
                </div> }
                })}
            </Suspense>
            {move || {
                let session = cart.session.get();
                if session.phase == rustok_cart_storefront::core::BuyerCartPhase::Added
                    && added_cycle.get() != Some(cycle.get()) { None } else {
                    session.notice(snapshot.with_value(|snapshot| snapshot.route.locale.clone()).as_deref())
                        .map(|notice| view! { <Alert>{notice}</Alert> })
                }
            }}
            {move || link_error.get().then(|| view! { <Alert>{labels.get_value().link_error}</Alert> })}
            <div class="flex flex-wrap gap-2">
                <Button on_click=Box::new(add) disabled=purchase_disabled loading=Signal::derive(move || cart.busy.try_get().unwrap_or(true))>{labels.get_value().add}</Button>
                {move || view_model.get().unavailable.then(|| view! { <Button variant=ButtonVariant::Outline disabled=locked on_click=Box::new(move || generation.update(|value| *value = value.saturating_add(1)))>{labels.get_value().retry}</Button> })}
                {move || view_model.get().preview.then(|| view! { <Button variant=ButtonVariant::Outline disabled=locked on_click=Box::new(move || change_input(input, generation, buyer::ordinary_context(&input.get_untracked())))>{labels.get_value().ordinary}</Button> })}
                {move || cart.session.get().cart.map(|cart| {
                    let href = snapshot.with_value(|snapshot| buyer::cart_context_href(&snapshot.route, &snapshot.product, &input.get(), &cart));
                    match href {
                        Some(href) => view! { <a href=href class="text-sm underline">{labels.get_value().use_cart_context}</a> }.into_any(),
                        None => view! { <p role="status">{labels.get_value().cart_locale}</p> }.into_any(),
                    }
                })}
                {move || cart.session.get().cart_id.map(|id| view! { <a class="text-sm underline" href=snapshot.with_value(|snapshot| buyer::cart_href(&snapshot.route, &id))>{labels.get_value().inspect_cart}</a> })}
                {move || cart.session.get().can_inspect().then(|| view! { <Button variant=ButtonVariant::Outline disabled=Signal::derive(move || !interactive.get() || !cart.can_reset()) on_click=Box::new(move || cart.inspect())>{labels.get_value().inspect_cart}</Button> })}
                <Button variant=ButtonVariant::Outline disabled=Signal::derive(move || !interactive.get() || !cart.can_reset()) on_click=Box::new(move || cart.reset_reference())>{labels.get_value().reset_cart}</Button>
                {move || {
                    let desired = href.get();
                    let actual = if !interactive.get() {
                        Some(desired.clone())
                    } else if cart.quote_context_ready() {
                        saved_href.get().and_then(|(saved_cycle, saved, actual)| {
                            (saved_cycle == cycle.get() && saved == desired).then_some(actual)
                        })
                    } else {
                        None
                    };
                    actual.map(|actual| view! {
                        <a class="text-sm underline" href=actual>{labels.get_value().selection_link}</a>
                    })
                }}
            </div>
        </section>
    }.into_any()
}

#[component]
fn BuyerAxisControl(
    snapshot: StoredValue<ProductBuyerSnapshot>,
    input: RwSignal<BuyerInput>,
    generation: RwSignal<u64>,
    attribute_id: String,
    label: String,
    locked: Signal<bool>,
) -> impl IntoView {
    let attribute_id = StoredValue::new(attribute_id);
    let (value, set_value) = signal(
        input
            .get_untracked()
            .values
            .get(&attribute_id.get_value())
            .cloned()
            .unwrap_or_default(),
    );
    Effect::new(move |_| {
        let next = input
            .get()
            .values
            .get(&attribute_id.get_value())
            .cloned()
            .unwrap_or_default();
        if value.get_untracked() != next {
            set_value.set(next);
        }
    });
    let options = Signal::derive(move || {
        snapshot.with_value(|snapshot| {
            let mut options = vec![SelectOption::new(
                "",
                buyer::labels(snapshot.route.locale.as_deref()).not_selected,
            )];
            if let Some(axis) = buyer::axis_views(&snapshot.product, &input.get().values)
                .into_iter()
                .find(|axis| axis.attribute_id == attribute_id.get_value())
            {
                options.extend(axis.options.into_iter().map(|option| SelectOption {
                    value: option.value,
                    label: option.label,
                    disabled: option.disabled,
                }));
            }
            options
        })
    });
    view! { <div on:change=move |event| {
        let next = snapshot.with_value(|snapshot| buyer::pick_axis(&snapshot.product, &input.get_untracked(), &attribute_id.get_value(), &event_target_value(&event)));
        if let Ok(next) = next { change_input(input, generation, next); }
    }><Label>{label}<Select id=format!("product-axis-{}", attribute_id.get_value()) options value set_value disabled=locked /></Label></div> }
}

#[cfg(target_arch = "wasm32")]
pub fn mount_product_buyer(
    root: web_sys::HtmlElement,
    tenant_slug: &str,
    current_query: &std::collections::BTreeMap<String, String>,
    cart: rustok_cart_storefront::BuyerCartController,
) -> Result<Box<dyn FnOnce()>, String> {
    let encoded = root
        .get_attribute("data-product-buyer-snapshot")
        .ok_or_else(|| "Missing product buyer snapshot".to_string())?;
    let snapshot = buyer::decode_snapshot(&encoded)
        .map_err(|_| "Invalid product buyer snapshot".to_string())?;
    if snapshot.tenant_slug != tenant_slug {
        return Err("Product buyer tenant does not match the storefront".to_string());
    }
    if &snapshot.route.query != current_query {
        return Err("Product buyer route changed before the client mounted".to_string());
    }
    root.set_inner_html("");
    let handle = mount_to(root, move || {
        provide_context(cart);
        view! { <ProductBuyerInteraction snapshot /> }
    });
    Ok(Box::new(move || drop(handle)))
}
