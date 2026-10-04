use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::prelude::*;
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::core::{
    action_hint, filter_orders, localized_order_status, order_grid_columns,
    order_list_request, order_status_badge, prepare_cancel_order_command,
    prepare_deliver_order_command, prepare_mark_paid_command, prepare_ship_order_command,
    short_order_id, summarize_order_header, summarize_order_lines, summarize_order_timeline,
    text_or_dash,
};
use crate::helpers::{apply_order_detail, clear_order_detail, handle_action_result};
use crate::i18n::t;
use crate::model::{OrderAdminBootstrap, OrderDetailEnvelope, OrderListItem};
use crate::transport;

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
    let (search_query, set_search_query) = signal(String::new());
    let (payment_id, set_payment_id) = signal(String::new());
    let (payment_method, set_payment_method) = signal("manual".to_string());
    let (tracking_number, set_tracking_number) = signal(String::new());
    let (carrier, set_carrier) = signal("manual".to_string());
    let (delivered_signature, set_delivered_signature) = signal(String::new());
    let (cancel_reason, set_cancel_reason) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let columns = order_grid_columns(ui_locale.as_deref());
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 10, 0));

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let orders = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            let bootstrap =
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            let request = order_list_request(String::new());
            transport::fetch_orders(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                request.status,
                request.page,
                request.per_page,
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
    let refresh_label = t(ui_locale.as_deref(), "order.action.refresh", "Refresh");
    let open_label = t(ui_locale.as_deref(), "order.action.open", "Open");
    let mark_paid_label = t(ui_locale.as_deref(), "order.action.markPaid", "Mark paid");
    let ship_label = t(ui_locale.as_deref(), "order.action.ship", "Ship");
    let deliver_label = t(ui_locale.as_deref(), "order.action.deliver", "Deliver");
    let cancel_label = t(ui_locale.as_deref(), "order.action.cancel", "Cancel");
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
            match transport::fetch_order_detail(
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
        let mark_paid_command = match prepare_mark_paid_command(
            payment_id.get_untracked(),
            payment_method.get_untracked(),
            mark_paid_requirements_error_label.clone(),
        ) {
            Ok(command) => command,
            Err(error) => {
                set_error.set(Some(error.to_string()));
                return;
            }
        };
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
            let result = transport::mark_order_paid(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                mark_paid_command.payment_id,
                mark_paid_command.payment_method,
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
        let ship_command = match prepare_ship_order_command(
            tracking_number.get_untracked(),
            carrier.get_untracked(),
            ship_requirements_error_label.clone(),
        ) {
            Ok(command) => command,
            Err(error) => {
                set_error.set(Some(error.to_string()));
                return;
            }
        };
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
            let result = transport::ship_order(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                ship_command.tracking_number,
                ship_command.carrier,
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
        let deliver_command = prepare_deliver_order_command(delivered_signature.get_untracked());
        let submit_error_label = deliver_submit_error_label.clone();
        let load_error_label = deliver_load_order_error_label.clone();
        let not_found_label = deliver_order_not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = transport::deliver_order(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                deliver_command.delivered_signature,
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
        let cancel_command = prepare_cancel_order_command(cancel_reason.get_untracked());
        let submit_error_label = cancel_submit_error_label.clone();
        let load_error_label = cancel_load_order_error_label.clone();
        let not_found_label = cancel_order_not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = transport::cancel_order(
                token_value.clone(),
                tenant_value.clone(),
                tenant_id.clone(),
                user_id,
                order_id.clone(),
                cancel_command.reason,
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

    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_payment = ui_locale.clone();
    let ui_locale_for_fulfillment = ui_locale.clone();
    let ui_locale_for_actions = ui_locale.clone();
    let initial_open_order = open_order;
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

    let filtered_orders = Memo::new(move |_| {
        let raw = orders.get().and_then(Result::ok).map(|l| l.items).unwrap_or_default();
        let query = search_query.get().trim().to_lowercase();
        let current_filters = filters.get();
        let filtered = filter_orders(&raw, &current_filters);
        if query.is_empty() {
            filtered
        } else {
            filtered
                .into_iter()
                .filter(|item| {
                    item.id.to_lowercase().contains(&query)
                        || item.customer_id.as_deref().map(|c| c.to_lowercase().contains(&query)).unwrap_or(false)
                        || item.status.to_lowercase().contains(&query)
                        || item.line_items.iter().any(|li| li.title.to_lowercase().contains(&query))
                })
                .collect()
        }
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
    });

    let cell_locale = ui_locale.clone();
    let cell_selected_id = selected_id;
    let cell_action_writer = query_writer.clone();
    let cell_open_label = open_label.clone();
    let cell_renderer = Callback::new(move |(item, col_id): (OrderListItem, String)| {
        match col_id.as_str() {
            "id" => {
                let id_short = short_order_id(&item.id);
                let is_sel = cell_selected_id.get().as_deref() == Some(&item.id);
                view! {
                    <div class="flex items-center gap-1.5">
                        <span class=if is_sel {
                            "font-mono text-xs font-semibold text-primary underline"
                        } else {
                            "font-mono text-xs font-medium text-foreground hover:text-primary transition"
                        }>
                            {id_short}
                        </span>
                    </div>
                }
                .into_any()
            }
            "created_at" => {
                let date_str = item.created_at.split('T').next().unwrap_or(&item.created_at);
                view! {
                    <span class="text-xs text-muted-foreground whitespace-nowrap">
                        {date_str.to_string()}
                    </span>
                }
                .into_any()
            }
            "customer" => {
                let display = item
                    .customer_id
                    .as_deref()
                    .map(short_order_id)
                    .unwrap_or_else(|| "—".to_string());
                view! {
                    <span class="text-xs text-foreground/90 font-mono truncate" title=item.customer_id.clone().unwrap_or_default()>
                        {display}
                    </span>
                }
                .into_any()
            }
            "status" => {
                let badge_cls = order_status_badge(item.status.as_str());
                let label = localized_order_status(cell_locale.as_deref(), item.status.as_str());
                view! {
                    <span class=format!("inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold border {badge_cls}")>
                        {label}
                    </span>
                }
                .into_any()
            }
            "items" => {
                let summary = summarize_order_lines(cell_locale.as_deref(), item.line_items.as_slice());
                let summary_title = summary.clone();
                view! {
                    <span class="text-xs text-muted-foreground truncate block max-w-[220px]" title=summary_title>
                        {summary}
                    </span>
                }
                .into_any()
            }
            "total" => {
                view! {
                    <span class="text-xs font-semibold text-foreground whitespace-nowrap">
                        {format!("{} {}", item.total_amount, item.currency_code)}
                    </span>
                }
                .into_any()
            }
            "actions" => {
                let open_id = item.id.clone();
                let item_writer = cell_action_writer.clone();
                let btn_label = cell_open_label.clone();
                view! {
                    <div class="flex items-center justify-center">
                        <button
                            type="button"
                            class="inline-flex items-center justify-center h-6 px-2.5 rounded-md text-[11px] font-medium bg-secondary text-secondary-foreground hover:bg-accent transition"
                            on:click=move |ev| {
                                ev.stop_propagation();
                                item_writer.push_value(AdminQueryKey::OrderId.as_str(), open_id.clone());
                            }
                        >
                            {btn_label}
                        </button>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    let on_row_click = {
        let click_writer = query_writer.clone();
        Callback::new(move |item: OrderListItem| {
            click_writer.push_value(AdminQueryKey::OrderId.as_str(), item.id);
        })
    };

    let is_ru = ui_locale.as_deref() == Some("ru");

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-3">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale.as_deref(), "order.badge", "order")}</span>
                    <h2 class="text-2xl font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.title", "Order Operations")}</h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">{t(ui_locale.as_deref(), "order.subtitle", "Module-owned operator workspace for order lifecycle, payment state visibility and delivery progress.")}</p>
                </div>
            </header>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.25fr)_minmax(0,1fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm flex flex-col gap-4">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <div class="flex items-center gap-2">
                                <h3 class="text-lg font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.list.title", "Orders")}</h3>
                                <span class="text-xs font-normal px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20">
                                    {move || filtered_orders.get().len()}
                                </span>
                            </div>
                            <p class="text-sm text-muted-foreground">{t(ui_locale.as_deref(), "order.list.subtitle", "Inspect checkout-created orders and jump into operational state transitions.")}</p>
                        </div>
                        <div class="flex flex-wrap items-center gap-2.5">
                            <input
                                type="text"
                                placeholder=if is_ru { "Быстрый поиск..." } else { "Quick search..." }
                                prop:value=move || search_query.get()
                                on:input=move |ev| {
                                    set_search_query.set(event_target_value(&ev));
                                    pagination.update(|p| p.set_page(1));
                                }
                                class="min-w-44 rounded-xl border border-border bg-background px-3 py-1.5 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none transition focus:border-primary"
                            />
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=move |_| set_refresh_nonce.update(|value| *value += 1)
                            >
                                {refresh_label.clone()}
                            </button>
                        </div>
                    </div>

                    // Selection toolbar when items selected
                    <Show when=move || !selection.get().is_empty()>
                        <div class="flex items-center justify-between gap-3 bg-primary/5 border border-primary/20 rounded-xl px-4 py-2 animate-in fade-in duration-150">
                            <div class="flex items-center gap-2">
                                <span class="w-2 h-2 rounded-full bg-primary animate-pulse" />
                                <span class="text-xs font-semibold text-foreground">
                                    {move || format!("{} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" })}
                                </span>
                            </div>
                            <button
                                type="button"
                                class="h-6 px-2.5 rounded-lg text-xs text-muted-foreground hover:text-foreground transition border border-border bg-background"
                                on:click=move |_| selection.update(|s| s.clear())
                            >
                                {if is_ru { "Снять выбор" } else { "Clear" }}
                            </button>
                        </div>
                    </Show>

                    <DataGrid
                        columns=columns
                        data=Signal::derive(move || filtered_orders.get())
                        key_fn=|item: &OrderListItem| item.id.clone()
                        cell_renderer=cell_renderer
                        is_loading=Signal::derive(move || busy.get() || orders.get().is_none())
                        empty_message=no_orders_label.clone()
                        selection=selection
                        pagination=pagination
                        filters=filters
                        on_filter_change=on_filters_change
                        on_row_click=on_row_click
                    />
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
                                            <p class="text-sm text-muted-foreground">{summarize_order_header(ui_locale_for_detail.as_deref(), &order)}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{
                                                let created_args = rustok_ui_i18n::fluent_args!("date" => order.created_at.clone());
                                                crate::i18n::format(
                                                    ui_locale_for_detail.as_deref(),
                                                    "order.detail.created",
                                                    Some(&created_args),
                                                    &format!("created {}", order.created_at),
                                                )
                                            }</p>
                                            <p>{
                                                let updated_args = rustok_ui_i18n::fluent_args!("date" => order.updated_at.clone());
                                                crate::i18n::format(
                                                    ui_locale_for_detail.as_deref(),
                                                    "order.detail.updated",
                                                    Some(&updated_args),
                                                    &format!("updated {}", order.updated_at),
                                                )
                                            }</p>
                                        </div>
                                    </div>
                                    <div class="mt-4 grid gap-3 md:grid-cols-2">
                                        <div class="rounded-xl border border-border p-4"><p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale.as_deref(), "order.section.lifecycle", "Lifecycle")}</p><p class="mt-2 text-sm text-muted-foreground">{summarize_order_timeline(ui_locale_for_detail.as_deref(), &order)}</p></div>
                                        <div class="rounded-xl border border-border p-4">
                                            <p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale.as_deref(), "order.section.customer", "Customer")}</p>
                                            <p class="mt-2 text-sm text-muted-foreground">{text_or_dash(order.customer_id.as_deref())}</p>
                                            <p class="mt-2 text-xs text-muted-foreground">{
                                                let channel_name = text_or_dash(order.channel_slug.as_deref());
                                                let channel_args = rustok_ui_i18n::fluent_args!("channel" => channel_name.clone());
                                                crate::i18n::format(
                                                    ui_locale_for_detail.as_deref(),
                                                    "order.detail.channel",
                                                    Some(&channel_args),
                                                    &format!("channel {channel_name}"),
                                                )
                                            }</p>
                                        </div>
                                    </div>
                                </div>
                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex items-center justify-between gap-3">
                                        <h4 class="text-base font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.section.lines", "Line items")}</h4>
                                        <span class="text-xs text-muted-foreground">{
                                            let count = order.line_items.len();
                                            let count_args = rustok_ui_i18n::fluent_args!("count" => count);
                                            crate::i18n::format(
                                                ui_locale_for_detail.as_deref(),
                                                "order.lines.itemsCount",
                                                Some(&count_args),
                                                &format!("{count} items"),
                                            )
                                        }</span>
                                    </div>
                                    <div class="mt-4 space-y-3">
                                        {order.line_items.into_iter().map(|line| {
                                            let sku_str = text_or_dash(line.sku.as_deref());
                                            let line_details_args = rustok_ui_i18n::fluent_args!(
                                                "sku" => sku_str.clone(),
                                                "quantity" => line.quantity,
                                                "profile" => line.shipping_profile_slug.clone()
                                            );
                                            let line_details = crate::i18n::format(
                                                ui_locale_for_detail.as_deref(),
                                                "order.lines.lineDetails",
                                                Some(&line_details_args),
                                                &format!("{sku_str} · qty {} · profile {}", line.quantity, line.shipping_profile_slug),
                                            );
                                            let unit_args = rustok_ui_i18n::fluent_args!("price" => line.unit_price.clone());
                                            let unit_price_str = crate::i18n::format(
                                                ui_locale_for_detail.as_deref(),
                                                "order.lines.unitPrice",
                                                Some(&unit_args),
                                                &format!("unit {}", line.unit_price),
                                            );
                                            view! {
                                                <div class="rounded-xl border border-border p-4">
                                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                                        <div>
                                                            <p class="font-medium text-card-foreground">{line.title.clone()}</p>
                                                            <p class="mt-1 text-xs text-muted-foreground">{line_details}</p>
                                                        </div>
                                                        <div class="text-right text-sm text-muted-foreground">
                                                            <p>{format!("{} {}", line.total_price, line.currency_code)}</p>
                                                            <p class="text-xs">{unit_price_str}</p>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                                <div class="grid gap-4 lg:grid-cols-2">
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-base font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.section.payment", "Payment collection")}</h4>
                                        {match payment_collection {
                                            Some(payment) => {
                                                let status_str = localized_order_status(ui_locale_for_payment.as_deref(), payment.status.as_str());
                                                let status_args = rustok_ui_i18n::fluent_args!("status" => status_str.clone());
                                                let status_line = crate::i18n::format(
                                                    ui_locale_for_payment.as_deref(),
                                                    "order.payment.status",
                                                    Some(&status_args),
                                                    &format!("status: {status_str}"),
                                                );
                                                let provider_str = text_or_dash(payment.provider_id.as_deref());
                                                let provider_args = rustok_ui_i18n::fluent_args!("provider" => provider_str.clone());
                                                let provider_line = crate::i18n::format(
                                                    ui_locale_for_payment.as_deref(),
                                                    "order.payment.provider",
                                                    Some(&provider_args),
                                                    &format!("provider: {provider_str}"),
                                                );
                                                let authorized_amount_str = format!("{} {}", payment.authorized_amount, payment.currency_code);
                                                let auth_args = rustok_ui_i18n::fluent_args!("amount" => authorized_amount_str.clone());
                                                let authorized_line = crate::i18n::format(
                                                    ui_locale_for_payment.as_deref(),
                                                    "order.payment.authorized",
                                                    Some(&auth_args),
                                                    &format!("authorized: {authorized_amount_str}"),
                                                );
                                                let captured_amount_str = format!("{} {}", payment.captured_amount, payment.currency_code);
                                                let cap_args = rustok_ui_i18n::fluent_args!("amount" => captured_amount_str.clone());
                                                let captured_line = crate::i18n::format(
                                                    ui_locale_for_payment.as_deref(),
                                                    "order.payment.captured",
                                                    Some(&cap_args),
                                                    &format!("captured: {captured_amount_str}"),
                                                );
                                                let pcount = payment.payments.len();
                                                let count_args = rustok_ui_i18n::fluent_args!("count" => pcount);
                                                let count_line = crate::i18n::format(
                                                    ui_locale_for_payment.as_deref(),
                                                    "order.payment.paymentsCount",
                                                    Some(&count_args),
                                                    &format!("payments: {pcount}"),
                                                );
                                                view! {
                                                    <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                                        <p>{status_line}</p>
                                                        <p>{provider_line}</p>
                                                        <p>{authorized_line}</p>
                                                        <p>{captured_line}</p>
                                                        <p>{count_line}</p>
                                                    </div>
                                                }.into_any()
                                            }
                                            None => view! { <p class="mt-4 text-sm text-muted-foreground">{load_related_empty_label.clone()}</p> }.into_any(),
                                        }}
                                    </div>
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-base font-semibold text-card-foreground">{t(ui_locale.as_deref(), "order.section.fulfillment", "Fulfillment")}</h4>
                                        {match fulfillment {
                                            Some(item) => {
                                                let status_str = localized_order_status(ui_locale_for_fulfillment.as_deref(), item.status.as_str());
                                                let status_args = rustok_ui_i18n::fluent_args!("status" => status_str.clone());
                                                let status_line = crate::i18n::format(
                                                    ui_locale_for_fulfillment.as_deref(),
                                                    "order.fulfillment.status",
                                                    Some(&status_args),
                                                    &format!("status: {status_str}"),
                                                );
                                                let carrier_str = text_or_dash(item.carrier.as_deref());
                                                let carrier_args = rustok_ui_i18n::fluent_args!("carrier" => carrier_str.clone());
                                                let carrier_line = crate::i18n::format(
                                                    ui_locale_for_fulfillment.as_deref(),
                                                    "order.fulfillment.carrier",
                                                    Some(&carrier_args),
                                                    &format!("carrier: {carrier_str}"),
                                                );
                                                let tracking_str = text_or_dash(item.tracking_number.as_deref());
                                                let tracking_args = rustok_ui_i18n::fluent_args!("tracking" => tracking_str.clone());
                                                let tracking_line = crate::i18n::format(
                                                    ui_locale_for_fulfillment.as_deref(),
                                                    "order.fulfillment.tracking",
                                                    Some(&tracking_args),
                                                    &format!("tracking: {tracking_str}"),
                                                );
                                                let note_str = text_or_dash(item.delivered_note.as_deref());
                                                let note_args = rustok_ui_i18n::fluent_args!("note" => note_str.clone());
                                                let note_line = crate::i18n::format(
                                                    ui_locale_for_fulfillment.as_deref(),
                                                    "order.fulfillment.deliveredNote",
                                                    Some(&note_args),
                                                    &format!("delivered note: {note_str}"),
                                                );
                                                view! {
                                                    <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                                        <p>{status_line}</p>
                                                        <p>{carrier_line}</p>
                                                        <p>{tracking_line}</p>
                                                        <p>{note_line}</p>
                                                    </div>
                                                }.into_any()
                                            }
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
