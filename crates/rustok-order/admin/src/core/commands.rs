use super::requests::text_or_none;

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

pub fn prepare_mark_paid_command(
    payment_id: impl AsRef<str>,
    payment_method: impl AsRef<str>,
    requirements_message: String,
) -> Result<OrderMarkPaidCommand, String> {
    let Some(payment_id) = text_or_none(payment_id) else {
        return Err(requirements_message);
    };
    let Some(payment_method) = text_or_none(payment_method) else {
        return Err(requirements_message);
    };

    Ok(OrderMarkPaidCommand {
        payment_id,
        payment_method,
    })
}

pub fn prepare_ship_order_command(
    tracking_number: impl AsRef<str>,
    carrier: impl AsRef<str>,
    requirements_message: String,
) -> Result<OrderShipCommand, String> {
    let Some(tracking_number) = text_or_none(tracking_number) else {
        return Err(requirements_message);
    };
    let Some(carrier) = text_or_none(carrier) else {
        return Err(requirements_message);
    };

    Ok(OrderShipCommand {
        tracking_number,
        carrier,
    })
}

pub fn prepare_deliver_order_command(delivered_signature: impl AsRef<str>) -> OrderDeliverCommand {
    OrderDeliverCommand {
        delivered_signature: text_or_none(delivered_signature),
    }
}

pub fn prepare_cancel_order_command(reason: impl AsRef<str>) -> OrderCancelCommand {
    OrderCancelCommand {
        reason: text_or_none(reason),
    }
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

        assert_eq!(error, "Payment fields required");
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

        assert_eq!(error, "Shipping fields required");
    }

    #[test]
    fn deliver_order_command_normalizes_optional_signature() {
        let command = prepare_deliver_order_command(" signed by Alex ");
        assert_eq!(
            command.delivered_signature.as_deref(),
            Some("signed by Alex")
        );

        let blank = prepare_deliver_order_command(" ");
        assert_eq!(blank.delivered_signature, None);
    }

    #[test]
    fn cancel_order_command_normalizes_optional_reason() {
        let command = prepare_cancel_order_command(" customer request ");
        assert_eq!(command.reason.as_deref(), Some("customer request"));

        let blank = prepare_cancel_order_command(" ");
        assert_eq!(blank.reason, None);
    }
}
