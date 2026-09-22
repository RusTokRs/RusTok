/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/// Formats integer minor currency units (cents/kopecks/etc.) and a currency code
/// into a standard display string without floating-point arithmetic.
///
/// Example: `format_ui_price(1250, "USD")` -> `"12.50 USD"`
pub fn format_ui_price(minor_units: i64, currency: &str) -> String {
    let currency_upper = currency.trim().to_uppercase();
    let is_negative = minor_units < 0;
    let absolute = minor_units.unsigned_abs();
    let major = absolute / 100;
    let minor = absolute % 100;

    let sign = if is_negative { "-" } else { "" };
    if currency_upper.is_empty() {
        format!("{sign}{major}.{minor:02}")
    } else {
        format!("{sign}{major}.{minor:02} {currency_upper}")
    }
}

/// Computes a human-readable discount percentage badge string.
///
/// Returns `Some("-20%")` when `discounted < original`, or `None` if no valid discount applies.
pub fn format_ui_discount_badge(original_minor: i64, discounted_minor: i64) -> Option<String> {
    if original_minor <= 0 || discounted_minor >= original_minor {
        return None;
    }

    let diff = original_minor - discounted_minor;
    let percent = (diff as f64 / original_minor as f64 * 100.0).round() as i64;
    if percent <= 0 {
        None
    } else {
        Some(format!("-{percent}%"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ui_price() {
        assert_eq!(format_ui_price(1250, "USD"), "12.50 USD");
        assert_eq!(format_ui_price(99, "EUR"), "0.99 EUR");
        assert_eq!(format_ui_price(0, "RUB"), "0.00 RUB");
        assert_eq!(format_ui_price(-500, "usd"), "-5.00 USD");
        assert_eq!(format_ui_price(10000, ""), "100.00");
    }

    #[test]
    fn test_format_ui_discount_badge() {
        assert_eq!(format_ui_discount_badge(1000, 800), Some("-20%".to_string()));
        assert_eq!(format_ui_discount_badge(1000, 1000), None);
        assert_eq!(format_ui_discount_badge(1000, 1200), None);
        assert_eq!(format_ui_discount_badge(0, 0), None);
    }
}
