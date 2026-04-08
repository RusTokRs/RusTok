use leptos::prelude::*;

use crate::i18n::t;
use crate::model::{OrderDetail, OrderDetailEnvelope, OrderLineItem, OrderListItem};

pub async fn handle_action_result(
    result: Result<(), crate::api::ApiError>,
    token_value: Option<String>,
    tenant_value: Option<String>,
    tenant_id: String,
    order_id: String,
    action_error_label: String,
    load_order_error_label: String,
    order_not_found_label: String,
    set_refresh_nonce: WriteSignal<u64>,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_selected_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<OrderDetailEnvelope>>,
    set_payment_id: WriteSignal<String>,
    set_payment_method: WriteSignal<String>,
    set_tracking_number: WriteSignal<String>,
    set_carrier: WriteSignal<String>,
    set_delivered_signature: WriteSignal<String>,
    set_cancel_reason: WriteSignal<String>,
) {
    match result {
        Ok(()) => {
            match crate::api::fetch_order_detail(token_value, tenant_value, tenant_id, order_id)
                .await
            {
                Ok(Some(detail)) => {
                    apply_order_detail(
                        &detail,
                        set_selected_id,
                        set_selected,
                        set_payment_id,
                        set_payment_method,
                        set_tracking_number,
                        set_carrier,
                        set_delivered_signature,
                        set_cancel_reason,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Ok(None) => set_error.set(Some(order_not_found_label)),
                Err(err) => set_error.set(Some(format!("{load_order_error_label}: {err}"))),
            }
        }
        Err(err) => set_error.set(Some(format!("{action_error_label}: {err}"))),
    }

    set_busy.set(false);
}

pub fn apply_order_detail(
    detail: &OrderDetailEnvelope,
    set_selected_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<OrderDetailEnvelope>>,
    set_payment_id: WriteSignal<String>,
    set_payment_method: WriteSignal<String>,
    set_tracking_number: WriteSignal<String>,
    set_carrier: WriteSignal<String>,
    set_delivered_signature: WriteSignal<String>,
    set_cancel_reason: WriteSignal<String>,
) {
    set_selected_id.set(Some(detail.order.id.clone()));
    set_selected.set(Some(detail.clone()));
    set_payment_id.set(detail.order.payment_id.clone().unwrap_or_default());
    set_payment_method.set(
        detail
            .order
            .payment_method
            .clone()
            .unwrap_or_else(|| "manual".to_string()),
    );
    set_tracking_number.set(
        detail
            .order
            .tracking_number
            .clone()
            .or_else(|| {
                detail
                    .fulfillment
                    .as_ref()
                    .and_then(|item| item.tracking_number.clone())
            })
            .unwrap_or_default(),
    );
    set_carrier.set(
        detail
            .order
            .carrier
            .clone()
            .or_else(|| {
                detail
                    .fulfillment
                    .as_ref()
                    .and_then(|item| item.carrier.clone())
            })
            .unwrap_or_else(|| "manual".to_string()),
    );
    set_delivered_signature.set(detail.order.delivered_signature.clone().unwrap_or_default());
    set_cancel_reason.set(
        detail
            .order
            .cancellation_reason
            .clone()
            .or_else(|| {
                detail
                    .fulfillment
                    .as_ref()
                    .and_then(|item| item.cancellation_reason.clone())
            })
            .unwrap_or_default(),
    );
}

pub fn localized_order_status(locale: Option<&str>, status: &str) -> String {
    match status {
        "pending" => t(locale, "order.status.pending", "Pending"),
        "confirmed" => t(locale, "order.status.confirmed", "Confirmed"),
        "paid" => t(locale, "order.status.paid", "Paid"),
        "shipped" => t(locale, "order.status.shipped", "Shipped"),
        "delivered" => t(locale, "order.status.delivered", "Delivered"),
        "cancelled" => t(locale, "order.status.cancelled", "Cancelled"),
        _ => status.to_string(),
    }
}

pub fn order_status_badge(status: &str) -> &'static str {
    match status {
        "delivered" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "paid" => "border-blue-200 bg-blue-50 text-blue-700",
        "shipped" => "border-amber-200 bg-amber-50 text-amber-700",
        "cancelled" => "border-rose-200 bg-rose-50 text-rose-700",
        _ => "border-slate-200 bg-slate-100 text-slate-700",
    }
}

pub fn summarize_order_lines(lines: &[OrderLineItem]) -> String {
    let preview = lines
        .iter()
        .take(2)
        .map(|line| format!("{} x{}", line.title, line.quantity))
        .collect::<Vec<_>>();
    if preview.is_empty() {
        "no line items".to_string()
    } else if lines.len() > 2 {
        format!("{} +{} more", preview.join(", "), lines.len() - 2)
    } else {
        preview.join(", ")
    }
}

pub fn format_order_caption(order: &OrderListItem) -> String {
    let mut parts = vec![format!("{} {}", order.total_amount, order.currency_code)];
    if let Some(customer_id) = order.customer_id.as_deref() {
        parts.push(format!("customer {}", short_order_id(customer_id)));
    }
    parts.push(format!("created {}", order.created_at));
    parts.join(" · ")
}

pub fn summarize_order_header(order: &OrderDetail) -> String {
    [
        Some(format!("{} {}", order.total_amount, order.currency_code)),
        order
            .payment_id
            .as_ref()
            .map(|payment_id| format!("payment {payment_id}")),
        order
            .tracking_number
            .as_ref()
            .map(|tracking| format!("tracking {tracking}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

pub fn summarize_order_timeline(order: &OrderDetail) -> String {
    let mut steps = vec![format!("created {}", order.created_at)];
    if let Some(value) = order.confirmed_at.as_deref() {
        steps.push(format!("confirmed {value}"));
    }
    if let Some(value) = order.paid_at.as_deref() {
        steps.push(format!("paid {value}"));
    }
    if let Some(value) = order.shipped_at.as_deref() {
        steps.push(format!("shipped {value}"));
    }
    if let Some(value) = order.delivered_at.as_deref() {
        steps.push(format!("delivered {value}"));
    }
    if let Some(value) = order.cancelled_at.as_deref() {
        steps.push(format!("cancelled {value}"));
    }
    steps.join(" · ")
}

pub fn action_hint(locale: Option<&str>, status: &str) -> String {
    match status {
        "confirmed" => t(
            locale,
            "order.actionHint.confirmed",
            "The next operational step is marking the order as paid.",
        ),
        "paid" => t(
            locale,
            "order.actionHint.paid",
            "The order is paid and ready for shipment.",
        ),
        "shipped" => t(
            locale,
            "order.actionHint.shipped",
            "The order is in transit and can be marked as delivered.",
        ),
        "delivered" => t(
            locale,
            "order.actionHint.delivered",
            "The order is complete; only inspection remains.",
        ),
        "cancelled" => t(
            locale,
            "order.actionHint.cancelled",
            "The order is cancelled; lifecycle buttons stay read-only.",
        ),
        _ => t(
            locale,
            "order.actionHint.pending",
            "This order is waiting for the next lifecycle event from checkout or operations.",
        ),
    }
}

pub fn short_order_id(value: &str) -> String {
    value.chars().take(8).collect()
}

pub fn text_or_dash(value: Option<&str>) -> String {
    value
        .filter(|item| !item.trim().is_empty())
        .unwrap_or("—")
        .to_string()
}

pub fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
