use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};

use crate::helpers::{
    action_hint, apply_order_detail, clear_order_detail, format_order_caption,
    handle_action_result, localized_order_status, order_status_badge, short_order_id,
    summarize_order_header, summarize_order_lines, summarize_order_timeline, text_or_dash,
    text_or_none,
};
use crate::i18n::t;
use crate::model::{OrderAdminBootstrap, OrderDetailEnvelope};

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn OrderAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_order_query = use_route_query_value(AdminQueryKey::OrderId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<OrderDetailEnvelope>::None);
    let (status_filter, set_status_filter) = signal(String::new());
    let (payment_id, set_payment_id) = signal(String::new());
    let (payment_method, set_payment_method) = signal("manual".to_string());
    let (tracking_number, set_tracking_number) = signal(String::new());
    let (carrier, set_carrier) = signal("manual".to_string());
    let (delivered_signature, set_delivered_signature) = signal(String::new());
    let (cancel_reason, set_cancel_reason) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            crate::api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let orders = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                status_filter.get(),
            )
        },
        move |(token_value, tenant_value, _, status_value)| async move {
            let bootstrap =
                crate::api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            crate::api::fetch_orders(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                text_or_none(status_value),
            )
            .await
        },
    );

    let bootstrap_loading_label = t(
        ui_locale.as_deref(),
        "order.error.bootstrapLoading",
        "Bootstrap is still loading.",
    );
    let load_order_error_label = t(
        ui_locale.as_deref(),
        "order.error.loadOrder",
        "Failed to load order",
    );
    let order_not_found_label = t(
        ui_locale.as_deref(),
        "order.error.orderNotFound",
        "Order not found.",
    );
    let load_orders_error_label = t(
        ui_locale.as_deref(),
        "order.error.loadOrders",
        "Failed to load orders",
    );
    let mark_paid_requirements_label = t(
        ui_locale.as_deref(),
        "order.error.markPaidRequirements",
        "Payment id and payment method are required.",
    );
    let ship_requirements_label = t(
        ui_locale.as_deref(),
        "order.error.shipRequirements",
        "Tracking number and carrier are required.",
    );
    let mark_paid_error_label = t(
        ui_locale.as_deref(),
        "order.error.markPaid",
        "Failed to mark order as paid",
    );
    let ship_error_label = t(
        ui_locale.as_deref(),
        "order.error.ship",
        "Failed to ship order",
    );
    let deliver_error_label = t(
        ui_locale.as_deref(),
        "order.error.deliver",
        "Failed to deliver order",
    );
    let cancel_error_label = t(
        ui_locale.as_deref(),
        "order.error.cancel",
        "Failed to cancel order",
    );
    let action_requires_selection_label = t(
        ui_locale.as_deref(),
        "order.error.selectionRequired",
        "Open an order first.",
    );
    let empty_state_label = t(
        ui_locale.as_deref(),
        "order.detail.empty",
        "Open an order to inspect line items, payment state and fulfillment progress.",
    );
    let all_statuses_label = t(
        ui_locale.as_deref(),
        "order.filter.allStatuses",
        "All statuses",
    );
    let refresh_label = t(ui_locale.as_deref(), "order.action.refresh", "Refresh");
    let open_label = t(ui_locale.as_deref(), "order.action.open", "Open");
    let mark_paid_label = t(ui_locale.as_deref(), "order.action.markPaid", "Mark paid");
    let ship_label = t(ui_locale.as_deref(), "order.action.ship", "Ship");
    let deliver_label = t(ui_locale.as_deref(), "order.action.deliver", "Deliver");
    let cancel_label = t(ui_locale.as_deref(), "order.action.cancel", "Cancel");
    let loading_label = t(ui_locale.as_deref(), "order.loading", "Loading...");
    let no_orders_label = t(
        ui_locale.as_deref(),
        "order.list.empty",
        "No orders match the current filters.",
    );
    let load_related_empty_label = t(
        ui_locale.as_deref(),
        "order.detail.none",
        "No related record.",
    );
    let payment_id_placeholder = t(ui_locale.as_deref(), "order.field.paymentId", "Payment ID");
    let payment_method_placeholder = t(
        ui_locale.as_deref(),
        "order.field.paymentMethod",
        "Payment method",
    );
    let tracking_number_placeholder = t(
        ui_locale.as_deref(),
        "order.field.trackingNumber",
        "Tracking number",
    );
    let carrier_placeholder = t(ui_locale.as_deref(), "order.field.carrier", "Carrier");
    let delivered_signature_placeholder = t(
        ui_locale.as_deref(),
        "order.field.deliveredSignature",
        "Delivered signature",
    );
    let cancel_reason_placeholder = t(
        ui_locale.as_deref(),
        "order.field.cancelReason",
        "Cancellation reason",
    );

    let open_bootstrap_loading_label = bootstrap_loading_label.clone();
    let open_load_order_error_label = load_order_error_label.clone();
    let open_order_not_found_label = order_not_found_label.clone();
    let open_order = Callback::new(move |order_id: String| {
        let Some(OrderAdminBootstrap { current_tenant, .. }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(open_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let not_found_label = open_order_not_found_label.clone();
        let load_error_label = open_load_order_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match crate::api::fetch_order_detail(
                token_value,
                tenant_value,
                current_tenant.id,
                order_id,
            )
            .await
            {
                Ok(Some(detail)) => apply_order_detail(
                    &detail,
                    set_selected_id,
                    set_selected,
                    set_payment_id,
                    set_payment_method,
                    set_tracking_number,
                    set_carrier,
                    set_delivered_signature,
                    set_cancel_reason,
                ),
                Ok(None) => {
                    clear_order_detail(
                        set_selected_id,
                        set_selected,
                        set_payment_id,
                        set_payment_method,
                        set_tracking_number,
                        set_carrier,
                        set_delivered_signature,
                        set_cancel_reason,
                    );
                    set_error.set(Some(not_found_label));
                }
                Err(err) => {
                    clear_order_detail(
                        set_selected_id,
                        set_selected,
                        set_payment_id,
                        set_payment_method,
                        set_tracking_number,
                        set_carrier,
                        set_delivered_signature,
                        set_cancel_reason,
                    );
                    set_error.set(Some(format!("{load_error_label}: {err}")));
                }
            }
            set_busy.set(false);
        });
    });

    let mark_paid_bootstrap_loading_label = bootstrap_loading_label.clone();
    let mark_paid_action_requires_selection_label = action_requires_selection_label.clone();
    let mark_paid_requirements_error_label = mark_paid_requirements_label.clone();
    let mark_paid_submit_error_label = mark_paid_error_label.clone();
    let mark_paid_order_not_found_label = order_not_found_label.clone();
    let mark_paid_load_order_error_label = load_order_error_label.clone();
    let mark_paid_order = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(OrderAdminBootstrap { current_tenant, me }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(mark_paid_bootstrap_loading_label.clone()));
            return;
        };
        let Some(order_id) = selected_id.get_untracked() else {
            set_error.set(Some(mark_paid_action_requires_selection_label.clone()));
            return;
        };
        let payment_id_value = payment_id.get_untracked().trim().to_string();
        let payment_method_value = payment_method.get_untracked().trim().to_string();
        if payment_id_value.is_empty() || payment_method_value.is_empty() {
            set_error.set(Some(mark_paid_requirements_error_label.clone()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let tenant_id = current_tenant.id.clone();
        let user_id = me.id.clone();
        let submit_error_label = mark_paid_submit_error_label.clone();
        let load_error_label = mark_paid_load_order_error_label.clone();
        let not_found_label = mark_paid_order_not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = crate::api::mark_order_paid(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                payment_id_value,
                payment_method_value,
            )
            .await;
            handle_action_result(
                result.map(|_| ()),
                token_value,
                tenant_value,
                tenant_id,
                order_id,
                submit_error_label,
                load_error_label,
                not_found_label,
                set_refresh_nonce,
                set_busy,
                set_error,
                set_selected_id,
                set_selected,
                set_payment_id,
                set_payment_method,
                set_tracking_number,
                set_carrier,
                set_delivered_signature,
                set_cancel_reason,
            )
            .await;
        });
    });

    let ship_bootstrap_loading_label = bootstrap_loading_label.clone();
    let ship_action_requires_selection_label = action_requires_selection_label.clone();
    let ship_requirements_error_label = ship_requirements_label.clone();
    let ship_submit_error_label = ship_error_label.clone();
    let ship_order_not_found_label = order_not_found_label.clone();
    let ship_load_order_error_label = load_order_error_label.clone();
    let ship_order = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(OrderAdminBootstrap { current_tenant, me }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(ship_bootstrap_loading_label.clone()));
            return;
        };
        let Some(order_id) = selected_id.get_untracked() else {
            set_error.set(Some(ship_action_requires_selection_label.clone()));
            return;
        };
        let tracking_number_value = tracking_number.get_untracked().trim().to_string();
        let carrier_value = carrier.get_untracked().trim().to_string();
        if tracking_number_value.is_empty() || carrier_value.is_empty() {
            set_error.set(Some(ship_requirements_error_label.clone()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let tenant_id = current_tenant.id.clone();
        let user_id = me.id.clone();
        let submit_error_label = ship_submit_error_label.clone();
        let load_error_label = ship_load_order_error_label.clone();
        let not_found_label = ship_order_not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = crate::api::ship_order(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                tracking_number_value,
                carrier_value,
            )
            .await;
            handle_action_result(
                result.map(|_| ()),
                token_value,
                tenant_value,
                tenant_id,
                order_id,
                submit_error_label,
                load_error_label,
                not_found_label,
                set_refresh_nonce,
                set_busy,
                set_error,
                set_selected_id,
                set_selected,
                set_payment_id,
                set_payment_method,
                set_tracking_number,
                set_carrier,
                set_delivered_signature,
                set_cancel_reason,
            )
            .await;
        });
    });

    let deliver_bootstrap_loading_label = bootstrap_loading_label.clone();
    let deliver_action_requires_selection_label = action_requires_selection_label.clone();
    let deliver_submit_error_label = deliver_error_label.clone();
    let deliver_order_not_found_label = order_not_found_label.clone();
    let deliver_load_order_error_label = load_order_error_label.clone();
    let deliver_order = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(OrderAdminBootstrap { current_tenant, me }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(deliver_bootstrap_loading_label.clone()));
            return;
        };
        let Some(order_id) = selected_id.get_untracked() else {
            set_error.set(Some(deliver_action_requires_selection_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let tenant_id = current_tenant.id.clone();
        let user_id = me.id.clone();
        let delivered_signature_value = text_or_none(delivered_signature.get_untracked());
        let submit_error_label = deliver_submit_error_label.clone();
        let load_error_label = deliver_load_order_error_label.clone();
        let not_found_label = deliver_order_not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = crate::api::deliver_order(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                delivered_signature_value,
            )
            .await;
            handle_action_result(
                result.map(|_| ()),
                token_value,
                tenant_value,
                tenant_id,
                order_id,
                submit_error_label,
                load_error_label,
                not_found_label,
                set_refresh_nonce,
                set_busy,
                set_error,
                set_selected_id,
                set_selected,
                set_payment_id,
                set_payment_method,
                set_tracking_number,
                set_carrier,
                set_delivered_signature,
                set_cancel_reason,
            )
            .await;
        });
    });

    let cancel_bootstrap_loading_label = bootstrap_loading_label.clone();
    let cancel_action_requires_selection_label = action_requires_selection_label.clone();
    let cancel_submit_error_label = cancel_error_label.clone();
    let cancel_order_not_found_label = order_not_found_label.clone();
    let cancel_load_order_error_label = load_order_error_label.clone();
    let cancel_order = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(OrderAdminBootstrap { current_tenant, me }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(cancel_bootstrap_loading_label.clone()));
            return;
        };
        let Some(order_id) = selected_id.get_untracked() else {
            set_error.set(Some(cancel_action_requires_selection_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let tenant_id = current_tenant.id.clone();
        let user_id = me.id.clone();
        let reason_value = text_or_none(cancel_reason.get_untracked());
        let submit_error_label = cancel_submit_error_label.clone();
        let load_error_label = cancel_load_order_error_label.clone();
        let not_found_label = cancel_order_not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = crate::api::cancel_order(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                reason_value,
            )
            .await;
            handle_action_result(
                result.map(|_| ()),
                token_value,
                tenant_value,
                tenant_id,
                order_id,
                submit_error_label,
                load_error_label,
                not_found_label,
                set_refresh_nonce,
                set_busy,
                set_error,
                set_selected_id,
                set_selected,
                set_payment_id,
                set_payment_method,
                set_tracking_number,
                set_carrier,
                set_delivered_signature,
                set_cancel_reason,
            )
            .await;
        });
    });

    let ui_locale_for_status_options = ui_locale.clone();
    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_payment = ui_locale.clone();
    let ui_locale_for_fulfillment = ui_locale.clone();
    let ui_locale_for_actions = ui_locale.clone();
    let initial_open_order = open_order;
    let list_query_writer = query_writer.clone();
    Effect::new(move |_| match selected_order_query.get() {
        Some(order_id) if !order_id.trim().is_empty() => {
            if bootstrap.get().and_then(Result::ok).is_none() {
                return;
            }
            initial_open_order.run(order_id);
        }
        _ => {
            clear_order_detail(
                set_selected_id,
                set_selected,
                set_payment_id,
                set_payment_method,
                set_tracking_number,
                set_carrier,
                set_delivered_signature,
                set_cancel_reason,
            );
        }
    });

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-3">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale.as_deref(), "order.badge", "order")}</span>
                    <h2 class="text-2xl font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.title", "Order Operations")}</h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">{t(ui_locale.as_deref(), "order.subtitle", "Module-owned operator workspace for order lifecycle, payment state visibility and delivery progress.")}</p>
                </div>
            </header>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,1.2fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.list.title", "Orders")}</h3>
                            <p class="text-sm text-muted-foreground">{t(ui_locale.as_deref(), "order.list.subtitle", "Inspect checkout-created orders and jump into operational state transitions.")}</p>
                        </div>
                        <div class="flex flex-wrap items-center gap-3">
                            <select class="min-w-52 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || status_filter.get() on:change=move |ev| set_status_filter.set(event_target_value(&ev))>
                                <option value="">{all_statuses_label.clone()}</option>
                                {["pending", "confirmed", "paid", "shipped", "delivered", "cancelled"].into_iter().map(|status| {
                                    let value = status.to_string();
                                    let label = localized_order_status(ui_locale_for_status_options.as_deref(), status);
                                    view! { <option value=value.clone()>{label}</option> }
                                }).collect_view()}
                            </select>
                            <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| set_refresh_nonce.update(|value| *value += 1)>{refresh_label.clone()}</button>
                        </div>
                    </div>

                    <div class="mt-5 space-y-4">
                        {move || match orders.get() {
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{no_orders_label.clone()}</div> }.into_any(),
                            Some(Ok(list)) => list.items.into_iter().map(|order| {
                                let open_id = order.id.clone();
                                let item_query_writer = list_query_writer.clone();
                                let status_label = localized_order_status(ui_locale_for_list.as_deref(), order.status.as_str());
                                let order_lines = summarize_order_lines(order.line_items.as_slice());
                                view! {
                                    <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                        <div class="flex items-start justify-between gap-3">
                                            <div class="space-y-2">
                                                <div class="flex flex-wrap items-center gap-2">
                                                    <h4 class="font-medium text-card-foreground">{short_order_id(order.id.as_str())}</h4>
                                                    <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", order_status_badge(order.status.as_str()))>{status_label}</span>
                                                </div>
                                                <p class="text-sm text-muted-foreground">{format_order_caption(&order)}</p>
                                                <p class="text-xs text-muted-foreground">{order_lines}</p>
                                            </div>
                                            <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| item_query_writer.push_value(AdminQueryKey::OrderId.as_str(), open_id.clone())>{open_label.clone()}</button>
                                        </div>
                                    </article>
                                }
                            }).collect_view().into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_orders_error_label}: {err}")}</div> }.into_any(),
                            None => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{loading_label.clone()}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="space-y-6 rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="space-y-2">
                        <h3 class="text-lg font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.detail.title", "Order detail")}</h3>
                        <p class="text-sm text-muted-foreground">{t(ui_locale.as_deref(), "order.detail.subtitle", "Read the operational snapshot and execute only the lifecycle step that matches the current state.")}</p>
                    </div>
                    <Show when=move || error.get().is_some()>
                        <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || error.get().unwrap_or_default()}</div>
                    </Show>
                    {move || selected.get().map(|detail| {
                        let order = detail.order.clone();
                        let payment_collection = detail.payment_collection.clone();
                        let fulfillment = detail.fulfillment.clone();
                        let mark_paid_disabled = busy.get() || order.status.as_str() != "confirmed";
                        let ship_disabled = busy.get() || order.status.as_str() != "paid";
                        let deliver_disabled = busy.get() || order.status.as_str() != "shipped";
                        let cancel_disabled = busy.get() || matches!(order.status.as_str(), "delivered" | "cancelled");

                        view! {
                            <div class="space-y-6">
                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div class="space-y-2">
                                            <div class="flex flex-wrap items-center gap-2">
                                                <h4 class="text-base font-semibold text-card-foreground">{short_order_id(order.id.as_str())}</h4>
                                                <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", order_status_badge(order.status.as_str()))>{localized_order_status(ui_locale_for_detail.as_deref(), order.status.as_str())}</span>
                                            </div>
                                            <p class="text-sm text-muted-foreground">{summarize_order_header(&order)}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground"><p>{format!("created {}", order.created_at)}</p><p>{format!("updated {}", order.updated_at)}</p></div>
                                    </div>
                                    <div class="mt-4 grid gap-3 md:grid-cols-2">
                                        <div class="rounded-xl border border-border p-4"><p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale.as_deref(), "order.section.lifecycle", "Lifecycle")}</p><p class="mt-2 text-sm text-muted-foreground">{summarize_order_timeline(&order)}</p></div>
                                        <div class="rounded-xl border border-border p-4"><p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale.as_deref(), "order.section.customer", "Customer")}</p><p class="mt-2 text-sm text-muted-foreground">{text_or_dash(order.customer_id.as_deref())}</p><p class="mt-2 text-xs text-muted-foreground">{format!("channel {}", text_or_dash(order.channel_slug.as_deref()))}</p></div>
                                    </div>
                                </div>
                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex items-center justify-between gap-3"><h4 class="text-base font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.section.lines", "Line items")}</h4><span class="text-xs text-muted-foreground">{format!("{} items", order.line_items.len())}</span></div>
                                    <div class="mt-4 space-y-3">
                                        {order.line_items.into_iter().map(|line| view! { <div class="rounded-xl border border-border p-4"><div class="flex flex-wrap items-start justify-between gap-3"><div><p class="font-medium text-card-foreground">{line.title.clone()}</p><p class="mt-1 text-xs text-muted-foreground">{format!("{} · qty {} · profile {}", text_or_dash(line.sku.as_deref()), line.quantity, line.shipping_profile_slug)}</p></div><div class="text-right text-sm text-muted-foreground"><p>{format!("{} {}", line.total_price, line.currency_code)}</p><p class="text-xs">{format!("unit {}", line.unit_price)}</p></div></div></div> }).collect_view()}
                                    </div>
                                </div>
                                <div class="grid gap-4 lg:grid-cols-2">
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-base font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.section.payment", "Payment collection")}</h4>
                                        {match payment_collection {
                                            Some(payment) => view! { <div class="mt-4 space-y-2 text-sm text-muted-foreground"><p>{format!("status: {}", localized_order_status(ui_locale_for_payment.as_deref(), payment.status.as_str()))}</p><p>{format!("provider: {}", text_or_dash(payment.provider_id.as_deref()))}</p><p>{format!("authorized: {} {}", payment.authorized_amount, payment.currency_code)}</p><p>{format!("captured: {} {}", payment.captured_amount, payment.currency_code)}</p><p>{format!("payments: {}", payment.payments.len())}</p></div> }.into_any(),
                                            None => view! { <p class="mt-4 text-sm text-muted-foreground">{load_related_empty_label.clone()}</p> }.into_any(),
                                        }}
                                    </div>
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-base font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.section.fulfillment", "Fulfillment")}</h4>
                                        {match fulfillment {
                                            Some(item) => view! { <div class="mt-4 space-y-2 text-sm text-muted-foreground"><p>{format!("status: {}", localized_order_status(ui_locale_for_fulfillment.as_deref(), item.status.as_str()))}</p><p>{format!("carrier: {}", text_or_dash(item.carrier.as_deref()))}</p><p>{format!("tracking: {}", text_or_dash(item.tracking_number.as_deref()))}</p><p>{format!("delivered note: {}", text_or_dash(item.delivered_note.as_deref()))}</p></div> }.into_any(),
                                            None => view! { <p class="mt-4 text-sm text-muted-foreground">{load_related_empty_label.clone()}</p> }.into_any(),
                                        }}
                                    </div>
                                </div>
                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="space-y-2"><h4 class="text-base font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.section.actions", "Lifecycle actions")}</h4><p class="text-sm text-muted-foreground">{action_hint(ui_locale_for_actions.as_deref(), order.status.as_str())}</p></div>
                                    <div class="mt-5 grid gap-4 xl:grid-cols-2">
                                        <form class="space-y-3 rounded-xl border border-border p-4" on:submit=move |ev| mark_paid_order.run(ev)><p class="text-sm font-medium text-card-foreground">{mark_paid_label.clone()}</p><input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=payment_id_placeholder.clone() prop:value=move || payment_id.get() on:input=move |ev| set_payment_id.set(event_target_value(&ev)) /><input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=payment_method_placeholder.clone() prop:value=move || payment_method.get() on:input=move |ev| set_payment_method.set(event_target_value(&ev)) /><button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || mark_paid_disabled>{mark_paid_label.clone()}</button></form>
                                        <form class="space-y-3 rounded-xl border border-border p-4" on:submit=move |ev| ship_order.run(ev)><p class="text-sm font-medium text-card-foreground">{ship_label.clone()}</p><input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=tracking_number_placeholder.clone() prop:value=move || tracking_number.get() on:input=move |ev| set_tracking_number.set(event_target_value(&ev)) /><input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=carrier_placeholder.clone() prop:value=move || carrier.get() on:input=move |ev| set_carrier.set(event_target_value(&ev)) /><button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || ship_disabled>{ship_label.clone()}</button></form>
                                        <form class="space-y-3 rounded-xl border border-border p-4" on:submit=move |ev| deliver_order.run(ev)><p class="text-sm font-medium text-card-foreground">{deliver_label.clone()}</p><input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=delivered_signature_placeholder.clone() prop:value=move || delivered_signature.get() on:input=move |ev| set_delivered_signature.set(event_target_value(&ev)) /><button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || deliver_disabled>{deliver_label.clone()}</button></form>
                                        <form class="space-y-3 rounded-xl border border-border p-4" on:submit=move |ev| cancel_order.run(ev)><p class="text-sm font-medium text-card-foreground">{cancel_label.clone()}</p><textarea class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=cancel_reason_placeholder.clone() prop:value=move || cancel_reason.get() on:input=move |ev| set_cancel_reason.set(event_target_value(&ev)) /><button type="submit" class="inline-flex rounded-xl bg-destructive px-4 py-2 text-sm font-medium text-destructive-foreground transition hover:bg-destructive/90 disabled:opacity-50" disabled=move || cancel_disabled>{cancel_label.clone()}</button></form>
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <div class="rounded-2xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">{empty_state_label.clone()}</div> }.into_any())}
                </section>
            </div>
        </section>
    }
}
