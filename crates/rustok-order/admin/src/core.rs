pub const DEFAULT_ORDER_PAGE: u64 = 1;
pub const DEFAULT_ORDER_PER_PAGE: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderListRequest {
    pub status: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

pub fn text_or_none(value: impl AsRef<str>) -> Option<String> {
    let trimmed = value.as_ref().trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn order_list_request(status: impl AsRef<str>) -> OrderListRequest {
    OrderListRequest {
        status: text_or_none(status),
        page: DEFAULT_ORDER_PAGE,
        per_page: DEFAULT_ORDER_PER_PAGE,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_list_request_trims_status_and_uses_defaults() {
        let request = order_list_request(" paid ");

        assert_eq!(request.status.as_deref(), Some("paid"));
        assert_eq!(request.page, DEFAULT_ORDER_PAGE);
        assert_eq!(request.per_page, DEFAULT_ORDER_PER_PAGE);
    }

    #[test]
    fn blank_text_normalizes_to_none() {
        assert_eq!(text_or_none("  "), None);
    }

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
}
