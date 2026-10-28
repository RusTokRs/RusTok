#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderCheckoutResultData {
    pub order_id: String,
    pub order_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderCheckoutResultLabels {
    pub badge: String,
    pub module_ownership: String,
    pub order_status_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderTrackingStep {
    pub step: u8,
    pub title: String,
    pub description: String,
    pub completed: bool,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderCheckoutResultViewModel {
    pub order_id: String,
    pub order_status_label: String,
    pub order_status: String,
    pub module_ownership: String,
    pub steps: Vec<OrderTrackingStep>,
}

pub fn calculate_order_step(status: &str) -> u8 {
    match status.trim().to_uppercase().as_str() {
        "PENDING" => 1,
        "CONFIRMED" | "PAID" => 2,
        "PROCESSING" => 3,
        "SHIPPED" => 4,
        "DELIVERED" => 5,
        _ => 1,
    }
}

pub fn build_order_tracking_steps(status: &str, is_ru: bool) -> Vec<OrderTrackingStep> {
    let current_step = calculate_order_step(status);
    let is_cancelled = status.trim().eq_ignore_ascii_case("cancelled");

    let step_defs = [
        (
            1,
            if is_ru {
                "Оформлен"
            } else {
                "Order Placed"
            },
            if is_ru {
                "Заказ получен системой"
            } else {
                "Order received"
            },
        ),
        (
            2,
            if is_ru {
                "Оплачен"
            } else {
                "Payment Confirmed"
            },
            if is_ru {
                "Оплата подтверждена"
            } else {
                "Payment processed"
            },
        ),
        (
            3,
            if is_ru { "Сборка" } else { "Processing" },
            if is_ru {
                "Комплектуется на складе"
            } else {
                "Packing items"
            },
        ),
        (
            4,
            if is_ru { "В пути" } else { "In Transit" },
            if is_ru {
                "Передан в службу доставки"
            } else {
                "Handed to courier"
            },
        ),
        (
            5,
            if is_ru {
                "Доставлен"
            } else {
                "Delivered"
            },
            if is_ru {
                "Вручен получателю"
            } else {
                "Successfully delivered"
            },
        ),
    ];

    step_defs
        .iter()
        .map(|&(num, title, desc)| OrderTrackingStep {
            step: num,
            title: title.to_string(),
            description: desc.to_string(),
            completed: !is_cancelled && current_step > num,
            active: !is_cancelled && current_step == num,
        })
        .collect()
}

pub fn build_order_checkout_result_view_model_with_locale(
    data: OrderCheckoutResultData,
    labels: &OrderCheckoutResultLabels,
    locale: Option<&str>,
) -> OrderCheckoutResultViewModel {
    let order_status = data.order_status.trim().to_string();
    let is_ru = locale
        .map(|l| l.trim().to_lowercase().starts_with("ru"))
        .unwrap_or_else(|| {
            labels.order_status_label.contains("Статус") || labels.badge.contains("Заказ")
        });
    let steps = build_order_tracking_steps(&order_status, is_ru);

    OrderCheckoutResultViewModel {
        order_id: data.order_id.trim().to_string(),
        order_status: order_status.clone(),
        order_status_label: labels.order_status_label.clone(),
        module_ownership: labels.module_ownership.clone(),
        steps,
    }
}

pub fn build_order_checkout_result_view_model(
    data: OrderCheckoutResultData,
    labels: &OrderCheckoutResultLabels,
) -> OrderCheckoutResultViewModel {
    build_order_checkout_result_view_model_with_locale(data, labels, None)
}

pub fn order_status_badge_style(status: &str) -> &'static str {
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

pub fn localize_status(locale: Option<&str>, status: &str) -> String {
    let trimmed = status.trim();
    let is_ru = locale
        .map(|l| l.trim().to_lowercase().starts_with("ru"))
        .unwrap_or(false);
    match trimmed.to_uppercase().as_str() {
        "PENDING" => if is_ru {
            "В обработке"
        } else {
            "Pending"
        }
        .to_string(),
        "CONFIRMED" => if is_ru {
            "Подтвержден"
        } else {
            "Confirmed"
        }
        .to_string(),
        "PAID" => if is_ru { "Оплачен" } else { "Paid" }.to_string(),
        "PROCESSING" => if is_ru { "Сборка" } else { "Processing" }.to_string(),
        "SHIPPED" => if is_ru {
            "Отправлен"
        } else {
            "Shipped"
        }
        .to_string(),
        "DELIVERED" => if is_ru {
            "Доставлен"
        } else {
            "Delivered"
        }
        .to_string(),
        "CANCELLED" => if is_ru { "Отменен" } else { "Cancelled" }.to_string(),
        _ => trimmed.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderCheckoutActionLabels {
    pub pending: String,
    pub complete: String,
}

pub fn order_checkout_action_label(busy: bool, labels: &OrderCheckoutActionLabels) -> String {
    if busy {
        labels.pending.clone()
    } else {
        labels.complete.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_order_checkout_result_identity_and_status() {
        let view_model = build_order_checkout_result_view_model(
            OrderCheckoutResultData {
                order_id: " order_1 ".into(),
                order_status: " completed ".into(),
            },
            &OrderCheckoutResultLabels {
                badge: "checkout result".into(),
                module_ownership: "Order details remain order-owned".into(),
                order_status_label: "Order status".into(),
            },
        );

        assert_eq!(view_model.order_id, "order_1");
        assert_eq!(view_model.order_status, "completed");
        assert_eq!(view_model.order_status_label, "Order status");
        assert_eq!(view_model.steps.len(), 5);
    }

    #[test]
    fn calculates_tracking_steps_for_paid_order() {
        let steps = build_order_tracking_steps("PAID", false);
        assert_eq!(steps.len(), 5);
        assert!(steps[0].completed);
        assert!(steps[1].active);
        assert!(!steps[2].active);
    }

    #[test]
    fn tracking_steps_for_cancelled_order_are_not_active_nor_completed() {
        let steps = build_order_tracking_steps("cancelled", true);
        assert_eq!(steps.len(), 5);
        for step in steps {
            assert!(!step.completed);
            assert!(!step.active);
        }
    }

    #[test]
    fn build_order_checkout_result_with_locale() {
        let labels = OrderCheckoutResultLabels {
            badge: "Order Status".into(),
            module_ownership: "Order".into(),
            order_status_label: "Status".into(),
        };
        let vm_ru = build_order_checkout_result_view_model_with_locale(
            OrderCheckoutResultData {
                order_id: "ord-1".into(),
                order_status: "PAID".into(),
            },
            &labels,
            Some("ru-RU"),
        );
        assert_eq!(vm_ru.steps[0].title, "Оформлен");

        let vm_en = build_order_checkout_result_view_model_with_locale(
            OrderCheckoutResultData {
                order_id: "ord-1".into(),
                order_status: "PAID".into(),
            },
            &labels,
            Some("en-US"),
        );
        assert_eq!(vm_en.steps[0].title, "Order Placed");
    }

    #[test]
    fn order_status_badge_styles_match_design_tokens() {
        assert_eq!(
            order_status_badge_style("PENDING"),
            "bg-amber-500/10 text-amber-600 border-amber-500/25"
        );
        assert_eq!(
            order_status_badge_style(" paid "),
            "bg-blue-500/10 text-blue-600 border-blue-500/25"
        );
        assert_eq!(
            order_status_badge_style("CONFIRMED"),
            "bg-blue-500/10 text-blue-600 border-blue-500/25"
        );
        assert_eq!(
            order_status_badge_style("processing"),
            "bg-indigo-500/10 text-indigo-600 border-indigo-500/25"
        );
        assert_eq!(
            order_status_badge_style("SHIPPED"),
            "bg-purple-500/10 text-purple-600 border-purple-500/25"
        );
        assert_eq!(
            order_status_badge_style("Delivered"),
            "bg-emerald-500/10 text-emerald-600 border-emerald-500/25"
        );
        assert_eq!(
            order_status_badge_style("cancelled"),
            "bg-rose-500/10 text-rose-600 border-rose-500/25"
        );
        assert_eq!(
            order_status_badge_style("UNKNOWN_STATUS"),
            "bg-primary/10 text-primary border-primary/25"
        );
    }

    #[test]
    fn localizes_order_statuses_correctly() {
        assert_eq!(localize_status(Some("ru"), "PENDING"), "В обработке");
        assert_eq!(localize_status(Some("ru-RU"), "PAID"), "Оплачен");
        assert_eq!(localize_status(Some("en"), "PAID"), "Paid");
        assert_eq!(localize_status(None, "CANCELLED"), "Cancelled");
        assert_eq!(localize_status(Some("ru"), "cancelled"), "Отменен");
        assert_eq!(
            localize_status(Some("fr"), "custom_status"),
            "custom_status"
        );
    }

    #[test]
    fn checkout_action_label_depends_on_busy_state() {
        let labels = OrderCheckoutActionLabels {
            pending: "Processing...".into(),
            complete: "Complete Order".into(),
        };
        assert_eq!(order_checkout_action_label(true, &labels), "Processing...");
        assert_eq!(
            order_checkout_action_label(false, &labels),
            "Complete Order"
        );
    }
}
