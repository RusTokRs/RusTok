/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

fn currency_minor_exponent(currency_upper: &str) -> u32 {
    match currency_upper {
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        _ => 2,
    }
}

/// Formats integer minor currency units (cents/kopecks/yen/etc.) and a currency code
/// into a standard display string without floating-point arithmetic.
///
/// Example: `format_ui_price(1250, "USD")` -> `"12.50 USD"`
pub fn format_ui_price(minor_units: i64, currency: &str) -> String {
    let currency_upper = currency.trim().to_uppercase();
    let is_negative = minor_units < 0;
    let absolute = minor_units.unsigned_abs();
    let exponent = currency_minor_exponent(&currency_upper);

    let sign = if is_negative { "-" } else { "" };
    let amount = match exponent {
        0 => format!("{sign}{absolute}"),
        exp => {
            let divisor = 10_u64.pow(exp);
            let major = absolute / divisor;
            let minor = absolute % divisor;
            let width = exp as usize;
            format!("{sign}{major}.{minor:0width$}")
        }
    };

    if currency_upper.is_empty() {
        amount
    } else {
        format!("{amount} {currency_upper}")
    }
}

/// Computes a human-readable discount percentage badge string.
///
/// Returns `Some("-20%")` when `0 <= discounted < original`, or `None` if no valid discount applies.
pub fn format_ui_discount_badge(original_minor: i64, discounted_minor: i64) -> Option<String> {
    if original_minor <= 0 || discounted_minor < 0 || discounted_minor >= original_minor {
        return None;
    }

    let original = original_minor as i128;
    let diff = original - (discounted_minor as i128);
    let percent = (diff * 100 + original / 2) / original;
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
        assert_eq!(format_ui_price(1500, "JPY"), "1500 JPY");
        assert_eq!(format_ui_price(1250, "KWD"), "1.250 KWD");
    }

    #[test]
    fn test_format_ui_discount_badge() {
        assert_eq!(
            format_ui_discount_badge(1000, 800),
            Some("-20%".to_string())
        );
        assert_eq!(format_ui_discount_badge(1000, 1000), None);
        assert_eq!(format_ui_discount_badge(1000, 1200), None);
        assert_eq!(format_ui_discount_badge(1000, -100), None);
        assert_eq!(format_ui_discount_badge(0, 0), None);
    }
}
