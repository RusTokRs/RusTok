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
            if is_ru { "Оформлен" } else { "Order Placed" },
            if is_ru { "Заказ получен системой" } else { "Order received" },
        ),
        (
            2,
            if is_ru { "Оплачен" } else { "Payment Confirmed" },
            if is_ru { "Оплата подтверждена" } else { "Payment processed" },
        ),
        (
            3,
            if is_ru { "Сборка" } else { "Processing" },
            if is_ru { "Комплектуется на складе" } else { "Packing items" },
        ),
        (
            4,
            if is_ru { "В пути" } else { "In Transit" },
            if is_ru { "Передан в службу доставки" } else { "Handed to courier" },
        ),
        (
            5,
            if is_ru { "Доставлен" } else { "Delivered" },
            if is_ru { "Вручен получателю" } else { "Successfully delivered" },
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

pub fn build_order_checkout_result_view_model(
    data: OrderCheckoutResultData,
    labels: &OrderCheckoutResultLabels,
) -> OrderCheckoutResultViewModel {
    let order_status = data.order_status.trim().to_string();
    let is_ru = labels.order_status_label.contains("Статус") || labels.badge.contains("Заказ");
    let steps = build_order_tracking_steps(&order_status, is_ru);

    OrderCheckoutResultViewModel {
        order_id: data.order_id.trim().to_string(),
        order_status: order_status.clone(),
        order_status_label: labels.order_status_label.clone(),
        module_ownership: labels.module_ownership.clone(),
        steps,
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
