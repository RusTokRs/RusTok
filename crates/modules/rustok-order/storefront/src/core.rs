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
pub struct OrderCheckoutResultViewModel {
    pub order_id: String,
    pub order_status_label: String,
    pub order_status: String,
    pub module_ownership: String,
}

pub fn build_order_checkout_result_view_model(
    data: OrderCheckoutResultData,
    labels: &OrderCheckoutResultLabels,
) -> OrderCheckoutResultViewModel {
    OrderCheckoutResultViewModel {
        order_id: data.order_id.trim().to_string(),
        order_status: data.order_status.trim().to_string(),
        order_status_label: labels.order_status_label.clone(),
        module_ownership: labels.module_ownership.clone(),
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
