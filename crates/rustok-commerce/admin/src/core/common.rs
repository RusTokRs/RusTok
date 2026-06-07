pub const DEFAULT_PROMOTION_KIND: &str = "fixed_discount";
pub const DEFAULT_PROMOTION_SCOPE: &str = "shipping";
pub const DEFAULT_PROMOTION_SOURCE_ID: &str = "promo-operator";
pub const DEFAULT_PROMOTION_AMOUNT: &str = "4.99";
pub const DEFAULT_ORDER_CHANGE_STATUS: &str = "pending";

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

pub fn trimmed_non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn optional_value(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
        .to_string()
}

pub fn active_badge_class(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}

pub fn order_change_status_badge_class(status: &str) -> &'static str {
    match status {
        "applied" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "cancelled" => "border-rose-200 bg-rose-50 text-rose-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
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

    #[test]
    fn trimmed_non_empty_normalizes_optional_filters() {
        assert_eq!(trimmed_non_empty(" value ").as_deref(), Some("value"));
        assert!(trimmed_non_empty("   ").is_none());
    }

    #[test]
    fn badge_classes_are_stable_for_host_adapters() {
        assert_eq!(
            active_badge_class(true),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );
        assert_eq!(
            active_badge_class(false),
            "border-slate-200 bg-slate-100 text-slate-700"
        );
        assert_eq!(
            order_change_status_badge_class("applied"),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );
        assert_eq!(
            order_change_status_badge_class("cancelled"),
            "border-rose-200 bg-rose-50 text-rose-700"
        );
        assert_eq!(
            order_change_status_badge_class("pending"),
            "border-amber-200 bg-amber-50 text-amber-700"
        );
    }
}
