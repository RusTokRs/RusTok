use crate::model::OrderDetailEnvelope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderAdminDetailFormState {
    pub selected_id: Option<String>,
    pub payment_id: String,
    pub payment_method: String,
    pub tracking_number: String,
    pub carrier: String,
    pub delivered_signature: String,
    pub cancel_reason: String,
}

impl Default for OrderAdminDetailFormState {
    fn default() -> Self {
        Self {
            selected_id: None,
            payment_id: String::new(),
            payment_method: "manual".to_string(),
            tracking_number: String::new(),
            carrier: "manual".to_string(),
            delivered_signature: String::new(),
            cancel_reason: String::new(),
        }
    }
}

pub fn order_detail_form_state(detail: &OrderDetailEnvelope) -> OrderAdminDetailFormState {
    OrderAdminDetailFormState {
        selected_id: Some(detail.order.id.clone()),
        payment_id: detail.order.payment_id.clone().unwrap_or_default(),
        payment_method: detail
            .order
            .payment_method
            .clone()
            .unwrap_or_else(|| "manual".to_string()),
        tracking_number: detail
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
        carrier: detail
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
        delivered_signature: detail.order.delivered_signature.clone().unwrap_or_default(),
        cancel_reason: detail
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Fulfillment, OrderDetail};

    fn order_detail_fixture() -> OrderDetail {
        OrderDetail {
            id: "ord_123456789".to_string(),
            tenant_id: "tenant_1".to_string(),
            channel_id: None,
            channel_slug: Some("web".to_string()),
            customer_id: Some("cus_123".to_string()),
            status: "paid".to_string(),
            currency_code: "USD".to_string(),
            total_amount: "120.00".to_string(),
            metadata: "{}".to_string(),
            payment_id: Some("pay_1".to_string()),
            payment_method: Some("card".to_string()),
            tracking_number: None,
            carrier: None,
            cancellation_reason: None,
            delivered_signature: Some("signed".to_string()),
            created_at: "2026-06-01T00:00:00Z".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
            confirmed_at: None,
            paid_at: Some("2026-06-01T01:00:00Z".to_string()),
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
            line_items: Vec::new(),
        }
    }

    fn fulfillment_fixture() -> Fulfillment {
        Fulfillment {
            id: "ful_1".to_string(),
            tenant_id: "tenant_1".to_string(),
            order_id: "ord_123456789".to_string(),
            shipping_option_id: None,
            customer_id: Some("cus_123".to_string()),
            status: "shipped".to_string(),
            carrier: Some("dhl".to_string()),
            tracking_number: Some("track_1".to_string()),
            delivered_note: None,
            cancellation_reason: Some("fulfillment cancelled".to_string()),
            metadata: "{}".to_string(),
            created_at: "2026-06-01T00:00:00Z".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn order_detail_form_state_prefers_order_values_and_falls_back_to_fulfillment() {
        let detail = OrderDetailEnvelope {
            order: order_detail_fixture(),
            payment_collection: None,
            fulfillment: Some(fulfillment_fixture()),
        };

        let state = order_detail_form_state(&detail);

        assert_eq!(state.selected_id.as_deref(), Some("ord_123456789"));
        assert_eq!(state.payment_id, "pay_1");
        assert_eq!(state.payment_method, "card");
        assert_eq!(state.tracking_number, "track_1");
        assert_eq!(state.carrier, "dhl");
        assert_eq!(state.delivered_signature, "signed");
        assert_eq!(state.cancel_reason, "fulfillment cancelled");
    }

    #[test]
    fn empty_order_detail_form_state_resets_to_manual_defaults() {
        let state = OrderAdminDetailFormState::default();

        assert_eq!(state.selected_id, None);
        assert_eq!(state.payment_id, "");
        assert_eq!(state.payment_method, "manual");
        assert_eq!(state.tracking_number, "");
        assert_eq!(state.carrier, "manual");
        assert_eq!(state.delivered_signature, "");
        assert_eq!(state.cancel_reason, "");
    }
}
