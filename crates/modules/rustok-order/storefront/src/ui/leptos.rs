use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    OrderCheckoutActionLabels, OrderCheckoutResultData, OrderCheckoutResultLabels,
    build_order_checkout_result_view_model, build_order_tracking_steps,
    order_checkout_action_label,
};
use crate::i18n::t;
use crate::transport::{
    CompleteCheckoutRequest, build_complete_checkout_request, fetch_order, fetch_orders,
};

fn order_status_badge_style(status: &str) -> &'static str {
    match status.trim().to_uppercase().as_str() {
        "PENDING" => "bg-amber-500/10 text-amber-600 border-amber-500/25",
        "CONFIRMED" | "PAID" => "bg-blue-500/10 text-blue-600 border-blue-500/25",
        "PROCESSING" => "bg-indigo-500/10 text-indigo-600 border-indigo-500/25",
        "SHIPPED" => "bg-purple-500/10 text-purple-600 border-purple-500/25",
        "DELIVERED" => "bg-emerald-500/10 text-emerald-600 border-emerald-500/25",
        "CANCELLED" => "bg-rose-500/10 text-rose-600 border-rose-500/25",
        _ => "bg-primary/10 text-primary border-primary/25",
    }
}

fn localize_status(locale: Option<&str>, status: &str) -> String {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    match status.trim().to_uppercase().as_str() {
        "PENDING" => if is_ru { "В обработке" } else { "Pending" }.to_string(),
        "CONFIRMED" => if is_ru { "Подтвержден" } else { "Confirmed" }.to_string(),
        "PAID" => if is_ru { "Оплачен" } else { "Paid" }.to_string(),
        "PROCESSING" => if is_ru { "Сборка" } else { "Processing" }.to_string(),
        "SHIPPED" => if is_ru { "Отправлен" } else { "Shipped" }.to_string(),
        "DELIVERED" => if is_ru { "Доставлен" } else { "Delivered" }.to_string(),
        "CANCELLED" => if is_ru { "Отменен" } else { "Cancelled" }.to_string(),
        other => other.to_string(),
    }
}

#[component]
pub fn OrderView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let is_ru = locale.as_deref().map(|l| l.starts_with("ru")).unwrap_or(false);

    let initial_order_id = route_context
        .subpath
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| route_context.query_value("order_id"))
        .or_else(|| route_context.query_value("id"))
        .unwrap_or("")
        .to_string();

    let (order_id, set_order_id) = signal(initial_order_id);
    let (search_input, set_search_input) = signal(String::new());
    let (copied_tracking, set_copied_tracking) = signal(false);

    let order_resource = Resource::new_blocking(
        move || order_id.get(),
        move |id| async move {
            let id = id.trim().to_string();
            if id.is_empty() || id == "pending" {
                return Ok(None);
            }
            fetch_order(id).await
        },
    );

    let labels = OrderCheckoutResultLabels {
        badge: t(locale.as_deref(), "order.checkout.badge", "Order"),
        module_ownership: t(
            locale.as_deref(),
            "order.checkout.moduleOwnership",
            "Order status and checkout completion stay in order-owned UI.",
        ),
        order_status_label: t(locale.as_deref(), "order.checkout.orderStatus", "Order status"),
    };

    view! {
        <section class="mx-auto max-w-4xl px-4 py-8 space-y-6">
            // Search / Lookup bar
            <div class="rounded-2xl border border-border bg-card p-5 shadow-xs">
                <form
                    class="flex flex-col sm:flex-row items-center gap-3"
                    on:submit=move |ev| {
                        ev.prevent_default();
                        let clean = search_input.get().trim().to_string();
                        if !clean.is_empty() {
                            set_order_id.set(clean);
                        }
                    }
                >
                    <input
                        type="text"
                        class="h-10 w-full flex-1 rounded-xl border border-border bg-background px-4 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                        placeholder=if is_ru { "Введите номер заказа (UUID, например: a1b2c3d4...)" } else { "Enter order ID or UUID (e.g. a1b2c3d4...)" }
                        prop:value=move || search_input.get()
                        on:input=move |ev| set_search_input.set(event_target_value(&ev))
                    />
                    <button
                        type="submit"
                        class="w-full sm:w-auto inline-flex h-10 items-center justify-center rounded-xl bg-primary px-5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-xs cursor-pointer"
                    >
                        {if is_ru { "Отследить" } else { "Track" }}
                    </button>
                </form>
            </div>

            // Order Content Display
            <Suspense fallback=move || view! {
                <div class="rounded-3xl border border-border bg-card p-8 shadow-xs animate-pulse space-y-6">
                    <div class="h-6 w-1/3 bg-muted rounded-xl"></div>
                    <div class="h-24 w-full bg-muted/50 rounded-2xl"></div>
                    <div class="h-40 w-full bg-muted/40 rounded-2xl"></div>
                </div>
            }>
                {move || {
                    let current_id = order_id.get();
                    let labels = labels.clone();
                    let loc = locale.clone();
                    order_resource.get().map(|result| match result {
                        Ok(Some(order)) => {
                            let steps = build_order_tracking_steps(&order.status, is_ru);
                            let status_badge_cls = order_status_badge_style(&order.status);
                            let status_text = localize_status(loc.as_deref(), &order.status);
                            let address_opt = order.parse_delivery_address();
                            let tracking = order.tracking_number.clone();
                            let carrier = order.carrier.clone().unwrap_or_else(|| if is_ru { "Курьерская служба" } else { "Standard Express" }.to_string());
                            let currency = order.currency_code.clone();

                            view! {
                                <article class="rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs space-y-6">
                                    // Header
                                    <div class="flex flex-wrap items-center justify-between gap-4 border-b border-border/80 pb-5">
                                        <div>
                                            <div class="text-xs font-bold uppercase tracking-[0.18em] text-primary">
                                                {labels.badge}
                                            </div>
                                            <h3 class="mt-2 text-xl sm:text-2xl font-extrabold text-card-foreground">
                                                {format!("#{}", order.id)}
                                            </h3>
                                            <p class="mt-1 text-xs text-muted-foreground">
                                                {if is_ru { "Оформлен: " } else { "Created: " }}
                                                {order.created_at.clone()}
                                            </p>
                                        </div>
                                        <div class="flex items-center gap-2">
                                            <span class=format!("inline-flex items-center gap-1.5 rounded-full px-3.5 py-1 text-xs font-bold border {}", status_badge_cls)>
                                                {status_text}
                                            </span>
                                        </div>
                                    </div>

                                    // Tracking Stepper
                                    <div>
                                        <div class="grid grid-cols-1 gap-3 sm:grid-cols-5">
                                            {steps.into_iter().map(|step| {
                                                let is_active = step.active;
                                                let is_completed = step.completed;
                                                let card_class = if is_active {
                                                    "flex flex-col p-4 rounded-2xl border border-primary bg-primary/5 ring-1 ring-primary shadow-xs"
                                                } else if is_completed {
                                                    "flex flex-col p-4 rounded-2xl border border-border/80 bg-background/60"
                                                } else {
                                                    "flex flex-col p-4 rounded-2xl border border-border/40 bg-muted/20 opacity-60"
                                                };
                                                let badge_class = if is_completed || is_active {
                                                    "flex h-7 w-7 items-center justify-center rounded-xl bg-primary text-primary-foreground text-xs font-bold"
                                                } else {
                                                    "flex h-7 w-7 items-center justify-center rounded-xl bg-muted text-muted-foreground text-xs font-bold"
                                                };

                                                view! {
                                                    <div class=card_class>
                                                        <div class="flex items-center justify-between mb-2">
                                                            <div class=badge_class>
                                                                {if is_completed { "✓" } else { "●" }}
                                                            </div>
                                                            <span class="text-[10px] font-bold text-muted-foreground">
                                                                {format!("0{}", step.step)}
                                                            </span>
                                                        </div>
                                                        <div class="text-sm font-bold text-foreground">
                                                            {step.title}
                                                        </div>
                                                        <div class="mt-1 text-xs text-muted-foreground">
                                                            {step.description}
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>

                                    // Order Line Items List
                                    <div class="rounded-2xl border border-border/80 bg-background/60 p-5">
                                        <h4 class="text-sm font-bold text-foreground mb-4">
                                            {if is_ru { "Состав заказа" } else { "Order Items" }}
                                            {format!(" ({})", order.line_items.len())}
                                        </h4>
                                        <div class="divide-y divide-border/60">
                                            {order.line_items.into_iter().map(|item| {
                                                let item_currency = item.currency_code.clone();
                                                view! {
                                                    <div class="py-3.5 flex items-center justify-between gap-4">
                                                        <div class="flex-1 min-w-0">
                                                            <div class="text-sm font-semibold text-foreground truncate">
                                                                {item.title}
                                                            </div>
                                                            {item.sku.map(|sku| view! {
                                                                <div class="text-[11px] text-muted-foreground font-mono">
                                                                    "SKU: " {sku}
                                                                </div>
                                                            })}
                                                        </div>
                                                        <div class="text-right shrink-0">
                                                            <div class="text-sm font-bold text-foreground">
                                                                {item.total_price} " " {item_currency}
                                                            </div>
                                                            <div class="text-xs text-muted-foreground">
                                                                {format!("{} × {}", item.quantity, item.unit_price)}
                                                            </div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>

                                    // Details Grid (Delivery Address + Payment & Financial Totals)
                                    <div class="grid gap-4 sm:grid-cols-2">
                                        // Delivery details
                                        <div class="rounded-2xl border border-border/80 bg-background/50 p-5 space-y-3">
                                            <div class="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                                                {if is_ru { "Доставка и получатель" } else { "Delivery & Recipient" }}
                                            </div>
                                            <div class="text-sm text-foreground">
                                                <div class="font-semibold">{carrier}</div>
                                                {if let Some(ref tr) = tracking {
                                                    let tr_val = tr.clone();
                                                    view! {
                                                        <div class="mt-2 flex items-center gap-2">
                                                            <span class="text-xs font-mono bg-muted/60 px-2 py-1 rounded-md text-foreground">
                                                                {tr.clone()}
                                                            </span>
                                                            <button
                                                                type="button"
                                                                class="text-xs text-primary font-semibold hover:underline cursor-pointer"
                                                                on:click=move |_| {
                                                                    set_copied_tracking.set(true);
                                                                    #[cfg(target_arch = "wasm32")]
                                                                    if let Some(win) = web_sys::window() {
                                                                        let _ = win.navigator().clipboard().write_text(&tr_val);
                                                                    }
                                                                    #[cfg(not(target_arch = "wasm32"))]
                                                                    let _ = &tr_val;
                                                                }
                                                            >
                                                                {move || if copied_tracking.get() {
                                                                    if is_ru { "Скопировано!" } else { "Copied!" }
                                                                } else {
                                                                    if is_ru { "Копировать" } else { "Copy" }
                                                                }}
                                                            </button>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! {
                                                        <div class="text-xs text-muted-foreground mt-1">
                                                            {if is_ru { "Трек-номер будет доступен после отправки" } else { "Tracking available once shipped" }}
                                                        </div>
                                                    }.into_any()
                                                }}
                                            </div>
                                            {address_opt.map(|addr| view! {
                                                <div class="pt-3 border-t border-border/60 text-xs text-muted-foreground space-y-1">
                                                    {addr.full_name.map(|name| view! { <div class="font-semibold text-foreground">{name}</div> })}
                                                    {addr.phone.map(|phone| view! { <div>"Тел: " {phone}</div> })}
                                                    {addr.street_address.map(|street| view! { <div>{street}</div> })}
                                                    {addr.city.map(|city| view! { <div>{city}</div> })}
                                                </div>
                                            })}
                                        </div>

                                        // Financial Summary
                                        <div class="rounded-2xl border border-border/80 bg-background/50 p-5 space-y-2.5">
                                            <div class="text-xs font-bold uppercase tracking-wider text-muted-foreground mb-3">
                                                {if is_ru { "Итог к оплате" } else { "Payment Summary" }}
                                            </div>
                                            <div class="flex justify-between text-xs text-muted-foreground">
                                                <span>{if is_ru { "Товары:" } else { "Subtotal:" }}</span>
                                                <span>{order.subtotal_amount} " " {currency.clone()}</span>
                                            </div>
                                            <div class="flex justify-between text-xs text-muted-foreground">
                                                <span>{if is_ru { "Доставка:" } else { "Shipping:" }}</span>
                                                <span>{order.shipping_total} " " {currency.clone()}</span>
                                            </div>
                                            {if !order.adjustment_total.is_empty() && order.adjustment_total != "0" {
                                                view! {
                                                    <div class="flex justify-between text-xs text-emerald-600">
                                                        <span>{if is_ru { "Скидки:" } else { "Adjustments:" }}</span>
                                                        <span>{order.adjustment_total} " " {currency.clone()}</span>
                                                    </div>
                                                }.into_any()
                                            } else {
                                                ().into_any()
                                            }}
                                            <div class="pt-3 border-t border-border/70 flex justify-between text-base font-extrabold text-foreground">
                                                <span>{if is_ru { "Итого:" } else { "Total:" }}</span>
                                                <span class="text-primary">{order.total_amount} " " {currency}</span>
                                            </div>
                                            {order.payment_method.map(|pm| view! {
                                                <div class="mt-2 pt-2 border-t border-border/40 text-[11px] text-muted-foreground flex justify-between">
                                                    <span>{if is_ru { "Способ оплаты:" } else { "Payment method:" }}</span>
                                                    <span class="font-semibold text-foreground uppercase">{pm}</span>
                                                </div>
                                            })}
                                        </div>
                                    </div>
                                </article>
                            }.into_any()
                        }
                        Ok(None) => {
                            if current_id.is_empty() || current_id == "pending" {
                                view! {
                                    <OrderCheckoutResultCard
                                        result=OrderCheckoutResultData {
                                            order_id: current_id,
                                            order_status: "PROCESSING".to_string(),
                                        }
                                        labels=labels
                                    />
                                }.into_any()
                            } else {
                                view! {
                                    <div class="rounded-3xl border border-dashed border-border bg-card p-8 text-center space-y-3">
                                        <div class="text-sm font-semibold text-foreground">
                                            {if is_ru { "Заказ не найден" } else { "Order not found" }}
                                        </div>
                                        <p class="text-xs text-muted-foreground max-w-md mx-auto">
                                            {if is_ru { "Проверьте введённый номер заказа и попробуйте снова." } else { "Please check the order number and try tracking again." }}
                                        </p>
                                    </div>
                                }.into_any()
                            }
                        }
                        Err(err) => {
                            view! {
                                <div class="rounded-2xl border border-destructive/20 bg-destructive/10 p-5 text-xs text-destructive">
                                    {err.to_string()}
                                </div>
                            }.into_any()
                        }
                    })
                }}
            </Suspense>

            // Customer Orders History view embedded below
            <div class="pt-6">
                <OrdersHistoryView on_select_order=Callback::new(move |id: String| {
                    set_order_id.set(id);
                }) />
            </div>
        </section>
    }
}

#[component]
pub fn OrdersHistoryView(
    #[prop(optional)] on_select_order: Option<Callback<String>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let is_ru = locale.as_deref().map(|l| l.starts_with("ru")).unwrap_or(false);

    let (active_tab, set_active_tab) = signal("ALL".to_string());
    let (refresh_nonce, _) = signal(0_u64);

    let orders_resource = Resource::new_blocking(
        move || (active_tab.get(), refresh_nonce.get()),
        move |(tab, _)| async move {
            let status = if tab == "ALL" {
                None
            } else {
                Some(tab)
            };
            fetch_orders(Some(1), Some(30), status).await
        },
    );

    view! {
        <section class="rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs space-y-6">
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 border-b border-border/80 pb-5">
                <div>
                    <h3 class="text-xl font-extrabold text-card-foreground">
                        {if is_ru { "История заказов" } else { "Order History" }}
                    </h3>
                    <p class="mt-1 text-xs text-muted-foreground">
                        {if is_ru { "Список ваших недавних покупок и их статусы" } else { "Your recent purchases and fulfillment status" }}
                    </p>
                </div>

                // Filter tabs
                <div class="flex flex-wrap gap-1.5 p-1 rounded-xl bg-muted/40 border border-border/60">
                    {["ALL", "PENDING", "PROCESSING", "SHIPPED", "DELIVERED", "CANCELLED"].into_iter().map(|status| {
                        let status_val = status.to_string();
                        let status_val_btn = status_val.clone();
                        let label = match status {
                            "ALL" => if is_ru { "Все" } else { "All" },
                            "PENDING" => if is_ru { "Ожидают" } else { "Pending" },
                            "PROCESSING" => if is_ru { "Сборка" } else { "Processing" },
                            "SHIPPED" => if is_ru { "В пути" } else { "Shipped" },
                            "DELIVERED" => if is_ru { "Доставлены" } else { "Delivered" },
                            "CANCELLED" => if is_ru { "Отменены" } else { "Cancelled" },
                            _ => status,
                        };

                        view! {
                            <button
                                type="button"
                                class=move || {
                                    if active_tab.get() == status_val {
                                        "px-3 py-1 text-xs font-bold rounded-lg bg-background text-foreground shadow-xs transition cursor-pointer"
                                    } else {
                                        "px-3 py-1 text-xs font-semibold rounded-lg text-muted-foreground hover:text-foreground transition cursor-pointer"
                                    }
                                }
                                on:click=move |_| set_active_tab.set(status_val_btn.clone())
                            >
                                {label}
                            </button>
                        }
                    }).collect_view()}
                </div>
            </div>

            // Orders List
            <Suspense fallback=move || view! {
                <div class="space-y-3">
                    <div class="h-16 w-full rounded-2xl bg-muted/40 animate-pulse"></div>
                    <div class="h-16 w-full rounded-2xl bg-muted/40 animate-pulse"></div>
                </div>
            }>
                {move || {
                    let on_select = on_select_order;
                    let loc = locale.clone();
                    orders_resource.get().map(|res| match res {
                        Ok(data) => {
                            if data.items.is_empty() {
                                view! {
                                    <div class="rounded-2xl border border-dashed border-border/80 p-8 text-center">
                                        <div class="text-sm font-semibold text-foreground">
                                            {if is_ru { "Заказов не найдено" } else { "No orders found" }}
                                        </div>
                                        <p class="text-xs text-muted-foreground mt-1">
                                            {if is_ru { "В этой категории пока нет заказов" } else { "No orders match the selected status filter." }}
                                        </p>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="space-y-3">
                                        {data.items.into_iter().map(|item| {
                                            let id = item.id.clone();
                                            let id_for_click = id.clone();
                                            let badge_cls = order_status_badge_style(&item.status);
                                            let status_str = localize_status(loc.as_deref(), &item.status);
                                            let currency = item.currency_code.clone();
                                            let items_count = item.line_items.len();

                                            view! {
                                                <div class="p-4 rounded-2xl border border-border/80 bg-background/60 hover:border-primary/40 transition flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
                                                    <div>
                                                        <div class="flex items-center gap-2">
                                                            <span class="text-xs font-mono font-bold text-foreground">
                                                                {format!("#{}", id)}
                                                            </span>
                                                            <span class=format!("inline-flex rounded-full px-2.5 py-0.5 text-[10px] font-bold border {}", badge_cls)>
                                                                {status_str}
                                                            </span>
                                                        </div>
                                                        <div class="text-xs text-muted-foreground mt-1">
                                                            {item.created_at} " • "
                                                            {format!("{} {}", items_count, if is_ru { "поз." } else { "items" })}
                                                        </div>
                                                    </div>

                                                    <div class="flex items-center justify-between sm:justify-end gap-4 shrink-0">
                                                        <div class="text-sm font-bold text-foreground">
                                                            {item.total_amount} " " {currency}
                                                        </div>
                                                        {if let Some(cb) = on_select {
                                                            let id_sel = id_for_click.clone();
                                                            view! {
                                                                <button
                                                                    type="button"
                                                                    class="inline-flex items-center rounded-xl bg-primary/10 border border-primary/25 px-3 py-1 text-xs font-semibold text-primary hover:bg-primary/20 transition cursor-pointer"
                                                                    on:click=move |_| cb.run(id_sel.clone())
                                                                >
                                                                    {if is_ru { "Детали" } else { "Details" }}
                                                                </button>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <a
                                                                    href=format!("/orders?id={}", id_for_click)
                                                                    class="inline-flex items-center rounded-xl bg-primary/10 border border-primary/25 px-3 py-1 text-xs font-semibold text-primary hover:bg-primary/20 transition"
                                                                >
                                                                    {if is_ru { "Детали" } else { "Details" }}
                                                                </a>
                                                            }.into_any()
                                                        }}
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }
                        }
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/20 bg-destructive/10 p-4 text-xs text-destructive">
                                {err.to_string()}
                            </div>
                        }.into_any()
                    })
                }}
            </Suspense>
        </section>
    }
}

#[component]
pub fn OrderCheckoutResultCard(
    result: OrderCheckoutResultData,
    labels: OrderCheckoutResultLabels,
) -> impl IntoView {
    let view_model = build_order_checkout_result_view_model(result, &labels);
    let order_id = view_model.order_id.clone();
    let is_placeholder = order_id == "pending" || order_id.is_empty();

    view! {
        <article class="mt-6 rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs">
            <div class="flex flex-wrap items-center justify-between gap-4 border-b border-border/80 pb-5">
                <div>
                    <div class="text-xs font-bold uppercase tracking-[0.18em] text-primary">
                        {labels.badge}
                    </div>
                    <h3 class="mt-2 text-xl sm:text-2xl font-extrabold text-card-foreground">
                        {if is_placeholder {
                            "Order Tracking".to_string()
                        } else {
                            format!("#{order_id}")
                        }}
                    </h3>
                    <p class="mt-1 text-xs text-muted-foreground">
                        {view_model.module_ownership}
                    </p>
                </div>
                <div class="flex items-center gap-2">
                    <span class="inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-bold bg-primary/10 text-primary">
                        {view_model.order_status_label.clone()}: " " {view_model.order_status.clone()}
                    </span>
                </div>
            </div>

            <div class="mt-6">
                <div class="grid grid-cols-1 gap-3 sm:grid-cols-5">
                    {view_model.steps.into_iter().map(|step| {
                        let is_active = step.active;
                        let is_completed = step.completed;
                        let card_class = if is_active {
                            "flex flex-col p-4 rounded-2xl border border-primary bg-primary/5 ring-1 ring-primary shadow-xs"
                        } else if is_completed {
                            "flex flex-col p-4 rounded-2xl border border-border/80 bg-background/60"
                        } else {
                            "flex flex-col p-4 rounded-2xl border border-border/40 bg-muted/20 opacity-60"
                        };
                        let badge_class = if is_completed || is_active {
                            "flex h-7 w-7 items-center justify-center rounded-xl bg-primary text-primary-foreground text-xs font-bold"
                        } else {
                            "flex h-7 w-7 items-center justify-center rounded-xl bg-muted text-muted-foreground text-xs font-bold"
                        };

                        view! {
                            <div class=card_class>
                                <div class="flex items-center justify-between mb-2">
                                    <div class=badge_class>
                                        {if is_completed { "✓" } else { "●" }}
                                    </div>
                                    <span class="text-[10px] font-bold text-muted-foreground">
                                        {format!("0{}", step.step)}
                                    </span>
                                </div>
                                <div class="text-sm font-bold text-foreground">
                                    {step.title}
                                </div>
                                <div class="mt-1 text-xs text-muted-foreground">
                                    {step.description}
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </div>

            <div class="mt-6 grid gap-4 sm:grid-cols-2">
                <div class="rounded-2xl border border-border/70 bg-background/50 p-4">
                    <div class="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                        {view_model.order_status_label}
                    </div>
                    <div class="mt-2 text-base font-bold text-foreground break-all">
                        {view_model.order_status}
                    </div>
                </div>
                <div class="rounded-2xl border border-border/70 bg-background/50 p-4">
                    <div class="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                        "Fulfillment & Delivery"
                    </div>
                    <div class="mt-2 text-sm text-foreground">
                        "Standard Express Logistics (1–3 business days)"
                    </div>
                </div>
            </div>
        </article>
    }
}

#[component]
pub fn OrderCheckoutCompleteButton(
    cart_id: String,
    busy: ReadSignal<bool>,
    labels: OrderCheckoutActionLabels,
    on_complete_checkout: Callback<CompleteCheckoutRequest>,
) -> impl IntoView {
    // Build once per component instance so network retries reuse the exact same
    // checkout idempotency key instead of opening a second operation.
    let request = build_complete_checkout_request(cart_id);
    view! {
        <button
            type="button"
            class="inline-flex items-center justify-center rounded-full border border-primary bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60 md:col-span-2"
            disabled=move || busy.get()
            on:click={
                let request = request.clone();
                move |_| on_complete_checkout.run(request.clone())
            }
        >
            {move || order_checkout_action_label(busy.get(), &labels)}
        </button>
    }
}
