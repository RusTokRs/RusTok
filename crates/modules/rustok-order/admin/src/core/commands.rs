use super::requests::text_or_none;

pub const MAX_PAYMENT_ID_LEN: usize = 100;
pub const MAX_PAYMENT_METHOD_LEN: usize = 100;
pub const MAX_TRACKING_NUMBER_LEN: usize = 100;
pub const MAX_CARRIER_LEN: usize = 100;
pub const MAX_DELIVERED_SIGNATURE_LEN: usize = 255;
pub const MAX_CANCEL_REASON_LEN: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderMarkPaidCommand {
    pub payment_id: String,
    pub payment_method: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderShipCommand {
    pub tracking_number: String,
    pub carrier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDeliverCommand {
    pub delivered_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderCancelCommand {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderActionEligibility {
    pub can_mark_paid: bool,
    pub can_ship: bool,
    pub can_deliver: bool,
    pub can_cancel: bool,
}

/// Evaluates operational action eligibility based on order lifecycle status.
pub fn order_action_eligibility(status: &str) -> OrderActionEligibility {
    let s = status.trim().to_ascii_lowercase();
    OrderActionEligibility {
        can_mark_paid: s == "confirmed",
        can_ship: s == "paid",
        can_deliver: s == "shipped",
        can_cancel: !matches!(s.as_str(), "delivered" | "cancelled"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderCommandError {
    MissingRequiredField(String),
    FieldTooLong { field: &'static str, max: usize },
}

impl std::fmt::Display for OrderCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequiredField(msg) => write!(f, "{msg}"),
            Self::FieldTooLong { field, max } => {
                write!(f, "{field} must not exceed {max} characters")
            }
        }
    }
}

impl std::error::Error for OrderCommandError {}

pub fn prepare_mark_paid_command(
    payment_id: impl AsRef<str>,
    payment_method: impl AsRef<str>,
    requirements_message: String,
) -> Result<OrderMarkPaidCommand, OrderCommandError> {
    let Some(payment_id) = text_or_none(payment_id) else {
        return Err(OrderCommandError::MissingRequiredField(
            requirements_message,
        ));
    };
    if payment_id.chars().count() > MAX_PAYMENT_ID_LEN {
        return Err(OrderCommandError::FieldTooLong {
            field: "Payment ID",
            max: MAX_PAYMENT_ID_LEN,
        });
    }

    let Some(payment_method) = text_or_none(payment_method) else {
        return Err(OrderCommandError::MissingRequiredField(
            requirements_message,
        ));
    };
    if payment_method.chars().count() > MAX_PAYMENT_METHOD_LEN {
        return Err(OrderCommandError::FieldTooLong {
            field: "Payment method",
            max: MAX_PAYMENT_METHOD_LEN,
        });
    }

    Ok(OrderMarkPaidCommand {
        payment_id,
        payment_method,
    })
}

pub fn prepare_ship_order_command(
    tracking_number: impl AsRef<str>,
    carrier: impl AsRef<str>,
    requirements_message: String,
) -> Result<OrderShipCommand, OrderCommandError> {
    let Some(tracking_number) = text_or_none(tracking_number) else {
        return Err(OrderCommandError::MissingRequiredField(
            requirements_message,
        ));
    };
    if tracking_number.chars().count() > MAX_TRACKING_NUMBER_LEN {
        return Err(OrderCommandError::FieldTooLong {
            field: "Tracking number",
            max: MAX_TRACKING_NUMBER_LEN,
        });
    }

    let Some(carrier) = text_or_none(carrier) else {
        return Err(OrderCommandError::MissingRequiredField(
            requirements_message,
        ));
    };
    if carrier.chars().count() > MAX_CARRIER_LEN {
        return Err(OrderCommandError::FieldTooLong {
            field: "Carrier",
            max: MAX_CARRIER_LEN,
        });
    }

    Ok(OrderShipCommand {
        tracking_number,
        carrier,
    })
}

pub fn prepare_deliver_order_command(
    delivered_signature: impl AsRef<str>,
) -> Result<OrderDeliverCommand, OrderCommandError> {
    let delivered_signature = text_or_none(delivered_signature);
    if let Some(ref sig) = delivered_signature
        && sig.chars().count() > MAX_DELIVERED_SIGNATURE_LEN
    {
        return Err(OrderCommandError::FieldTooLong {
            field: "Delivered signature",
            max: MAX_DELIVERED_SIGNATURE_LEN,
        });
    }

    Ok(OrderDeliverCommand {
        delivered_signature,
    })
}

pub fn prepare_cancel_order_command(
    reason: impl AsRef<str>,
) -> Result<OrderCancelCommand, OrderCommandError> {
    let reason = text_or_none(reason);
    if let Some(ref r) = reason
        && r.chars().count() > MAX_CANCEL_REASON_LEN
    {
        return Err(OrderCommandError::FieldTooLong {
            field: "Cancellation reason",
            max: MAX_CANCEL_REASON_LEN,
        });
    }

    Ok(OrderCancelCommand { reason })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_paid_command_trims_required_fields() {
        let command = prepare_mark_paid_command(" pay_1 ", " manual ", "required".to_string())
            .expect("valid mark-paid command");

        assert_eq!(command.payment_id, "pay_1");
        assert_eq!(command.payment_method, "manual");
    }

    #[test]
    fn mark_paid_command_rejects_missing_required_fields() {
        let error = prepare_mark_paid_command(" ", "manual", "Payment fields required".to_string())
            .expect_err("blank payment id must fail before transport");

        assert_eq!(
            error,
            OrderCommandError::MissingRequiredField("Payment fields required".to_string())
        );
    }

    #[test]
    fn mark_paid_command_validates_bounds() {
        let long_id = "a".repeat(MAX_PAYMENT_ID_LEN + 1);
        let err = prepare_mark_paid_command(long_id, "manual", "required".to_string())
            .expect_err("long payment id must be rejected");
        assert_eq!(
            err,
            OrderCommandError::FieldTooLong {
                field: "Payment ID",
                max: MAX_PAYMENT_ID_LEN
            }
        );

        let long_method = "m".repeat(MAX_PAYMENT_METHOD_LEN + 1);
        let err = prepare_mark_paid_command("pay_1", long_method, "required".to_string())
            .expect_err("long payment method must be rejected");
        assert_eq!(
            err,
            OrderCommandError::FieldTooLong {
                field: "Payment method",
                max: MAX_PAYMENT_METHOD_LEN
            }
        );

        // Exact bound should succeed
        let exact_id = "a".repeat(MAX_PAYMENT_ID_LEN);
        let exact_method = "m".repeat(MAX_PAYMENT_METHOD_LEN);
        let cmd = prepare_mark_paid_command(exact_id.clone(), exact_method.clone(), "req".to_string())
            .expect("exact length bound must pass");
        assert_eq!(cmd.payment_id, exact_id);
        assert_eq!(cmd.payment_method, exact_method);
    }

    #[test]
    fn ship_order_command_trims_required_fields() {
        let command = prepare_ship_order_command(" track_1 ", " dhl ", "required".to_string())
            .expect("valid ship command");

        assert_eq!(command.tracking_number, "track_1");
        assert_eq!(command.carrier, "dhl");
    }

    #[test]
    fn ship_order_command_rejects_missing_required_fields() {
        let error =
            prepare_ship_order_command("track", " ", "Shipping fields required".to_string())
                .expect_err("blank carrier must fail before transport");

        assert_eq!(
            error,
            OrderCommandError::MissingRequiredField("Shipping fields required".to_string())
        );
    }

    #[test]
    fn ship_order_command_validates_bounds() {
        let long_tracking = "t".repeat(MAX_TRACKING_NUMBER_LEN + 1);
        let err = prepare_ship_order_command(long_tracking, "dhl", "required".to_string())
            .expect_err("long tracking number must be rejected");
        assert_eq!(
            err,
            OrderCommandError::FieldTooLong {
                field: "Tracking number",
                max: MAX_TRACKING_NUMBER_LEN
            }
        );

        let long_carrier = "c".repeat(MAX_CARRIER_LEN + 1);
        let err = prepare_ship_order_command("track_1", long_carrier, "required".to_string())
            .expect_err("long carrier must be rejected");
        assert_eq!(
            err,
            OrderCommandError::FieldTooLong {
                field: "Carrier",
                max: MAX_CARRIER_LEN
            }
        );

        let exact_tracking = "t".repeat(MAX_TRACKING_NUMBER_LEN);
        let exact_carrier = "c".repeat(MAX_CARRIER_LEN);
        let cmd = prepare_ship_order_command(exact_tracking.clone(), exact_carrier.clone(), "req".to_string())
            .expect("exact length bound must pass");
        assert_eq!(cmd.tracking_number, exact_tracking);
        assert_eq!(cmd.carrier, exact_carrier);
    }

    #[test]
    fn deliver_order_command_normalizes_and_validates_bounds() {
        let command = prepare_deliver_order_command(" signed by Alex ")
            .expect("valid deliver command");
        assert_eq!(
            command.delivered_signature.as_deref(),
            Some("signed by Alex")
        );

        let blank = prepare_deliver_order_command(" ")
            .expect("blank signature normalizes to None");
        assert_eq!(blank.delivered_signature, None);

        let long_sig = "s".repeat(MAX_DELIVERED_SIGNATURE_LEN + 1);
        let err = prepare_deliver_order_command(long_sig)
            .expect_err("long signature must be rejected");
        assert_eq!(
            err,
            OrderCommandError::FieldTooLong {
                field: "Delivered signature",
                max: MAX_DELIVERED_SIGNATURE_LEN
            }
        );

        let exact_sig = "s".repeat(MAX_DELIVERED_SIGNATURE_LEN);
        let cmd = prepare_deliver_order_command(&exact_sig)
            .expect("exact length signature must pass");
        assert_eq!(cmd.delivered_signature.as_deref(), Some(exact_sig.as_str()));
    }

    #[test]
    fn cancel_order_command_normalizes_and_validates_bounds() {
        let command = prepare_cancel_order_command(" customer request ")
            .expect("valid cancel command");
        assert_eq!(command.reason.as_deref(), Some("customer request"));

        let blank = prepare_cancel_order_command(" ")
            .expect("blank reason normalizes to None");
        assert_eq!(blank.reason, None);

        let long_reason = "r".repeat(MAX_CANCEL_REASON_LEN + 1);
        let err = prepare_cancel_order_command(long_reason)
            .expect_err("long cancellation reason must be rejected");
        assert_eq!(
            err,
            OrderCommandError::FieldTooLong {
                field: "Cancellation reason",
                max: MAX_CANCEL_REASON_LEN
            }
        );

        let exact_reason = "r".repeat(MAX_CANCEL_REASON_LEN);
        let cmd = prepare_cancel_order_command(&exact_reason)
            .expect("exact length reason must pass");
        assert_eq!(cmd.reason.as_deref(), Some(exact_reason.as_str()));
    }

    #[test]
    fn order_action_eligibility_matches_lifecycle_transitions() {
        // Pending: cannot mark paid, ship, or deliver; can cancel
        let pending = order_action_eligibility("pending");
        assert_eq!(
            pending,
            OrderActionEligibility {
                can_mark_paid: false,
                can_ship: false,
                can_deliver: false,
                can_cancel: true,
            }
        );

        // Confirmed: can mark paid, can cancel; cannot ship or deliver
        let confirmed = order_action_eligibility(" Confirmed ");
        assert_eq!(
            confirmed,
            OrderActionEligibility {
                can_mark_paid: true,
                can_ship: false,
                can_deliver: false,
                can_cancel: true,
            }
        );

        // Paid: can ship, can cancel; cannot mark paid or deliver
        let paid = order_action_eligibility("PAID");
        assert_eq!(
            paid,
            OrderActionEligibility {
                can_mark_paid: false,
                can_ship: true,
                can_deliver: false,
                can_cancel: true,
            }
        );

        // Shipped: can deliver, can cancel; cannot mark paid or ship
        let shipped = order_action_eligibility("shipped");
        assert_eq!(
            shipped,
            OrderActionEligibility {
                can_mark_paid: false,
                can_ship: false,
                can_deliver: true,
                can_cancel: true,
            }
        );

        // Delivered (terminal): no actions allowed
        let delivered = order_action_eligibility("delivered");
        assert_eq!(
            delivered,
            OrderActionEligibility {
                can_mark_paid: false,
                can_ship: false,
                can_deliver: false,
                can_cancel: false,
            }
        );

        // Cancelled (terminal): no actions allowed
        let cancelled = order_action_eligibility("CANCELLED");
        assert_eq!(
            cancelled,
            OrderActionEligibility {
                can_mark_paid: false,
                can_ship: false,
                can_deliver: false,
                can_cancel: false,
            }
        );
    }
}
