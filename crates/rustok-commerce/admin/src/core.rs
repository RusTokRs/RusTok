pub const DEFAULT_PROMOTION_KIND: &str = "fixed_discount";
pub const DEFAULT_PROMOTION_SCOPE: &str = "shipping";
pub const DEFAULT_PROMOTION_SOURCE_ID: &str = "promo-operator";
pub const DEFAULT_PROMOTION_AMOUNT: &str = "4.99";
pub const DEFAULT_ORDER_CHANGE_STATUS: &str = "pending";

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_defaults_are_stable_for_admin_form() {
        assert_eq!(DEFAULT_PROMOTION_KIND, "fixed_discount");
        assert_eq!(DEFAULT_PROMOTION_SCOPE, "shipping");
        assert_eq!(DEFAULT_PROMOTION_SOURCE_ID, "promo-operator");
        assert_eq!(DEFAULT_PROMOTION_AMOUNT, "4.99");
    }
}
