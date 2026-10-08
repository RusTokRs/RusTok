use crate::i18n::{format, t};
use crate::model::{OrderDetail, OrderLineItem, OrderListItem};
use rustok_ui_i18n::fluent_args;

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

pub fn summarize_order_lines(locale: Option<&str>, lines: &[OrderLineItem]) -> String {
    let preview = lines
        .iter()
        .take(2)
        .map(|line| format!("{} x{}", line.title, line.quantity))
        .collect::<Vec<_>>();
    if preview.is_empty() {
        t(locale, "order.lines.empty", "no line items")
    } else if lines.len() > 2 {
        let more_count = lines.len() - 2;
        let args = fluent_args!("count" => more_count);
        let more_label = format(
            locale,
            "order.lines.more",
            Some(&args),
            &format!("+{more_count} more"),
        );
        format!("{} {}", preview.join(", "), more_label)
    } else {
        preview.join(", ")
    }
}

pub fn format_order_caption(locale: Option<&str>, order: &OrderListItem) -> String {
    let mut parts = vec![format!("{} {}", order.total_amount, order.currency_code)];
    if let Some(customer_id) = order.customer_id.as_deref() {
        let cid = short_order_id(customer_id);
        let args = fluent_args!("customer" => cid.clone());
        parts.push(format(
            locale,
            "order.caption.customer",
            Some(&args),
            &format!("customer {cid}"),
        ));
    }
    let date = order.created_at.clone();
    let args = fluent_args!("date" => date.clone());
    parts.push(format(
        locale,
        "order.caption.created",
        Some(&args),
        &format!("created {date}"),
    ));
    parts.join(" · ")
}

pub fn summarize_order_header(locale: Option<&str>, order: &OrderDetail) -> String {
    let mut parts = vec![format!("{} {}", order.total_amount, order.currency_code)];
    if let Some(payment_id) = order.payment_id.as_deref() {
        let args = fluent_args!("id" => payment_id.to_string());
        parts.push(format(
            locale,
            "order.header.payment",
            Some(&args),
            &format!("payment {payment_id}"),
        ));
    }
    if let Some(tracking) = order.tracking_number.as_deref() {
        let args = fluent_args!("tracking" => tracking.to_string());
        parts.push(format(
            locale,
            "order.header.tracking",
            Some(&args),
            &format!("tracking {tracking}"),
        ));
    }
    parts.join(" · ")
}

pub fn summarize_order_timeline(locale: Option<&str>, order: &OrderDetail) -> String {
    let mut steps = Vec::new();
    let created_args = fluent_args!("date" => order.created_at.clone());
    steps.push(format(
        locale,
        "order.timeline.created",
        Some(&created_args),
        &format!("created {}", order.created_at),
    ));
    if let Some(value) = order.confirmed_at.as_deref() {
        let args = fluent_args!("date" => value.to_string());
        steps.push(format(
            locale,
            "order.timeline.confirmed",
            Some(&args),
            &format!("confirmed {value}"),
        ));
    }
    if let Some(value) = order.paid_at.as_deref() {
        let args = fluent_args!("date" => value.to_string());
        steps.push(format(
            locale,
            "order.timeline.paid",
            Some(&args),
            &format!("paid {value}"),
        ));
    }
    if let Some(value) = order.shipped_at.as_deref() {
        let args = fluent_args!("date" => value.to_string());
        steps.push(format(
            locale,
            "order.timeline.shipped",
            Some(&args),
            &format!("shipped {value}"),
        ));
    }
    if let Some(value) = order.delivered_at.as_deref() {
        let args = fluent_args!("date" => value.to_string());
        steps.push(format(
            locale,
            "order.timeline.delivered",
            Some(&args),
            &format!("delivered {value}"),
        ));
    }
    if let Some(value) = order.cancelled_at.as_deref() {
        let args = fluent_args!("date" => value.to_string());
        steps.push(format(
            locale,
            "order.timeline.cancelled",
            Some(&args),
            &format!("cancelled {value}"),
        ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_status_badge_maps_lifecycle_states() {
        assert!(order_status_badge("paid").contains("text-blue-700"));
        assert!(order_status_badge("cancelled").contains("text-rose-700"));
        assert!(order_status_badge("pending").contains("text-slate-700"));
    }

    #[test]
    fn text_or_dash_normalizes_blank_optional_display_values() {
        assert_eq!(text_or_dash(Some(" value ")), " value ");
        assert_eq!(text_or_dash(Some("   ")), "—");
        assert_eq!(text_or_dash(None), "—");
    }

    #[test]
    fn summarize_order_lines_localizes_correctly() {
        let empty_en = summarize_order_lines(Some("en"), &[]);
        assert_eq!(empty_en, "no line items");

        let empty_ru = summarize_order_lines(Some("ru"), &[]);
        assert_eq!(empty_ru, "нет позиций");

        let lines = vec![
            OrderLineItem {
                id: "1".into(),
                order_id: "o1".into(),
                product_id: None,
                variant_id: Some("v1".into()),
                title: "T-Shirt".into(),
                sku: Some("TSHIRT-1".into()),
                quantity: 2,
                unit_price: "20.00".into(),
                total_price: "40.00".into(),
                currency_code: "USD".into(),
                shipping_profile_slug: "standard".into(),
                metadata: None,
                created_at: "2026-10-02".into(),
            },
            OrderLineItem {
                id: "2".into(),
                order_id: "o1".into(),
                product_id: None,
                variant_id: Some("v2".into()),
                title: "Cap".into(),
                sku: None,
                quantity: 1,
                unit_price: "15.00".into(),
                total_price: "15.00".into(),
                currency_code: "USD".into(),
                shipping_profile_slug: "standard".into(),
                metadata: None,
                created_at: "2026-10-02".into(),
            },
            OrderLineItem {
                id: "3".into(),
                order_id: "o1".into(),
                product_id: None,
                variant_id: Some("v3".into()),
                title: "Socks".into(),
                sku: None,
                quantity: 3,
                unit_price: "5.00".into(),
                total_price: "15.00".into(),
                currency_code: "USD".into(),
                shipping_profile_slug: "standard".into(),
                metadata: None,
                created_at: "2026-10-02".into(),
            },
        ];

        fn strip_bidi(s: &str) -> String {
            s.replace(['\u{2068}', '\u{2069}'], "")
        }

        let summary_en = strip_bidi(&summarize_order_lines(Some("en"), &lines));
        assert!(summary_en.contains("+1 more"));

        let summary_ru = strip_bidi(&summarize_order_lines(Some("ru"), &lines));
        assert!(summary_ru.contains("+ ещё 1"));
    }

    #[test]
    fn format_order_caption_localizes_correctly() {
        fn strip_bidi(s: &str) -> String {
            s.replace(['\u{2068}', '\u{2069}'], "")
        }

        let order = OrderListItem {
            id: "12345678-abcd".into(),
            customer_id: Some("cust-87654321".into()),
            status: "pending".into(),
            currency_code: "USD".into(),
            total_amount: "100.00".into(),
            tracking_number: None,
            carrier: None,
            created_at: "2026-10-02".into(),
            confirmed_at: None,
            paid_at: None,
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
            line_items: vec![],
        };

        let caption_en = strip_bidi(&format_order_caption(Some("en"), &order));
        assert!(caption_en.contains("customer cust-876"));
        assert!(caption_en.contains("created 2026-10-02"));

        let caption_ru = strip_bidi(&format_order_caption(Some("ru"), &order));
        assert!(caption_ru.contains("клиент cust-876"));
        assert!(caption_ru.contains("создан 2026-10-02"));
    }
}
