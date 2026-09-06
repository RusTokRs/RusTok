use leptos::prelude::*;

use crate::core::{OrderAdminDetailFormState, order_detail_form_state};
use crate::model::OrderDetailEnvelope;

#[allow(clippy::too_many_arguments)]
pub async fn handle_action_result(
    result: Result<(), crate::transport::ApiError>,
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
            match crate::transport::fetch_order_detail(
                token_value,
                tenant_value,
                tenant_id,
                order_id,
            )
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
                    set_error.set(Some(order_not_found_label));
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
                    set_error.set(Some(format!("{load_order_error_label}: {err}")));
                }
            }
        }
        Err(err) => set_error.set(Some(format!("{action_error_label}: {err}"))),
    }

    set_busy.set(false);
}

#[allow(clippy::too_many_arguments)]
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
    set_selected.set(Some(detail.clone()));
    apply_order_detail_form_state(
        order_detail_form_state(detail),
        set_selected_id,
        set_payment_id,
        set_payment_method,
        set_tracking_number,
        set_carrier,
        set_delivered_signature,
        set_cancel_reason,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn clear_order_detail(
    set_selected_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<OrderDetailEnvelope>>,
    set_payment_id: WriteSignal<String>,
    set_payment_method: WriteSignal<String>,
    set_tracking_number: WriteSignal<String>,
    set_carrier: WriteSignal<String>,
    set_delivered_signature: WriteSignal<String>,
    set_cancel_reason: WriteSignal<String>,
) {
    set_selected.set(None);
    apply_order_detail_form_state(
        OrderAdminDetailFormState::default(),
        set_selected_id,
        set_payment_id,
        set_payment_method,
        set_tracking_number,
        set_carrier,
        set_delivered_signature,
        set_cancel_reason,
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_order_detail_form_state(
    state: OrderAdminDetailFormState,
    set_selected_id: WriteSignal<Option<String>>,
    set_payment_id: WriteSignal<String>,
    set_payment_method: WriteSignal<String>,
    set_tracking_number: WriteSignal<String>,
    set_carrier: WriteSignal<String>,
    set_delivered_signature: WriteSignal<String>,
    set_cancel_reason: WriteSignal<String>,
) {
    set_selected_id.set(state.selected_id);
    set_payment_id.set(state.payment_id);
    set_payment_method.set(state.payment_method);
    set_tracking_number.set(state.tracking_number);
    set_carrier.set(state.carrier);
    set_delivered_signature.set(state.delivered_signature);
    set_cancel_reason.set(state.cancel_reason);
}
