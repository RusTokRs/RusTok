/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical money rules for the platform.
//!
//! This module is the single owner of the three rules that every money path needs and that used to
//! be duplicated per crate (see `docs/audits/ecommerce-deep-review-2026-10-07.md`, ECOM-MONEY-02):
//!
//! 1. the currency-exponent table (`currency_exponent`),
//! 2. currency-aware rounding (`round_to_currency`, `round_to_fixed`),
//! 3. major-unit <-> minor-unit conversion (`to_minor_units*`, `from_minor_units`,
//!    `to_fixed_point_units*`, `from_fixed_point_units`).
//!
//! Platform rule: the exponent is a property of the currency, not of the caller, so a currency
//! code is normalized and validated here and every conversion either produces an exact, in-range
//! value or a typed [`MoneyError`] — never a silent zero, a silent 2-decimal truncation, or a
//! panic. Callers own their error type and map [`MoneyError`] into it.
//!
//! Payment-provider adapters own a second contract: a PSP may define its own minor units for some
//! currencies. Adapters MUST use the exponent-explicit primitives (`to_fixed_point_units_exact`,
//! `from_fixed_point_units`) with their own documented, per-currency override table instead of
//! keeping a private copy of the platform table.
//!
//! Table source: ISO 4217 (the currency list is the platform's, taken from the former
//! `rustok-cart::services::cart::helpers` table). `MGA` and `MRU` are non-decimal ratios in
//! ISO 4217 (1 ariary = 5 iraimbilanja) but are recorded there with exponent 2, which is what the
//! platform table does as well.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use thiserror::Error;

/// Largest minor-unit exponent the platform accepts for a stored exponent.
///
/// The marketplace persistence guards accept `0..=9`; ISO 4217 itself never exceeds four decimals
/// (`CLF`, `UYW`). Anything outside the range is a data bug and is rejected instead of being
/// truncated or rounded away.
pub const MAX_MINOR_UNIT_EXPONENT: u8 = 9;

/// Exponent applied to any well-formed currency code that is not listed in this module.
pub const DEFAULT_MINOR_UNIT_EXPONENT: u8 = 2;

/// ISO 4217 currencies whose minor unit is the major unit (exponent 0).
pub const ZERO_DECIMAL_CURRENCIES: &[&str] = &[
    "BIF", "CLP", "DJF", "GNF", "ISK", "JPY", "KMF", "KRW", "PYG", "RWF", "UGX", "VND", "VUV",
    "XAF", "XOF", "XPF",
];

/// ISO 4217 currencies with three decimals (1/1000 of the major unit).
pub const THREE_DECIMAL_CURRENCIES: &[&str] = &["BHD", "IQD", "JOD", "KWD", "LYD", "OMR", "TND"];

/// ISO 4217 currencies with four decimals.
pub const FOUR_DECIMAL_CURRENCIES: &[&str] = &["CLF", "UYW"];

/// ISO 4217 codes whose minor unit is not a decimal subdivision of the major unit.
///
/// Precious metals, special drawing rights, bond units and the testing/no-currency codes carry no
/// exponent at all; converting them through this module is a typed error instead of a guess.
pub const UNDEFINED_MINOR_UNIT_CURRENCIES: &[&str] = &[
    "XAU", "XAG", "XPT", "XPD", "XBA", "XBB", "XBC", "XBD", "XDR", "XSU", "XTS", "XXX",
];

/// Failure modes of a money conversion.
///
/// Every variant is a caller-visible contract violation; none of them is ever swallowed by this
/// module. `amount` fields carry the `Decimal` rendering so an operator can see the offending
/// value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MoneyError {
    /// The code is not a three-letter alphabetic ISO 4217 style code.
    #[error("currency code `{code}` must be a three-letter alphabetic code")]
    InvalidCurrencyCode { code: String },
    /// The code has no decimal minor unit in ISO 4217 (metals, SDR, testing codes).
    #[error("currency `{code}` has no decimal minor unit in ISO 4217")]
    UndefinedMinorUnit { code: String },
    /// The requested exponent is outside `0..=MAX_MINOR_UNIT_EXPONENT`.
    #[error("minor-unit exponent {exponent} is outside the supported range 0..=9")]
    UnsupportedExponent { exponent: u8 },
    /// The amount carries more precision than the exponent allows, and exactness was required.
    #[error("amount {amount} has more precision than minor-unit exponent {exponent} allows")]
    ExcessPrecision { amount: String, exponent: u8 },
    /// The scaled amount does not fit the platform's `i64` minor-unit representation.
    #[error("amount {amount} cannot be represented with minor-unit exponent {exponent}")]
    OutOfRange { amount: String, exponent: u8 },
}

/// Result alias for money conversions.
pub type MoneyResult<T> = Result<T, MoneyError>;

/// Validates and normalizes a currency code (`trim` + uppercase, three ASCII letters).
///
/// Normalization lives in the owner so that two callers can never disagree about whether `"usd "`
/// and `"USD"` are the same currency.
pub fn normalize_currency_code(currency_code: &str) -> MoneyResult<String> {
    let normalized = currency_code.trim().to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(MoneyError::InvalidCurrencyCode {
            code: currency_code.to_string(),
        });
    }
    Ok(normalized)
}

/// Returns the ISO 4217 minor-unit exponent of `currency_code`.
///
/// Anything that is not a well-formed code is rejected; a well-formed code that is not listed
/// falls back to [`DEFAULT_MINOR_UNIT_EXPONENT`] (two decimals), which is the ISO 4217 default.
pub fn currency_exponent(currency_code: &str) -> MoneyResult<u8> {
    let normalized = normalize_currency_code(currency_code)?;
    if ZERO_DECIMAL_CURRENCIES.contains(&normalized.as_str()) {
        return Ok(0);
    }
    if THREE_DECIMAL_CURRENCIES.contains(&normalized.as_str()) {
        return Ok(3);
    }
    if FOUR_DECIMAL_CURRENCIES.contains(&normalized.as_str()) {
        return Ok(4);
    }
    if UNDEFINED_MINOR_UNIT_CURRENCIES.contains(&normalized.as_str()) {
        return Err(MoneyError::UndefinedMinorUnit { code: normalized });
    }
    Ok(DEFAULT_MINOR_UNIT_EXPONENT)
}

/// Rounds `amount` to the currency's minor-unit precision (half away from zero).
///
/// This is the platform's rounding rule for discounts, promotions and previews: they must never
/// round a JPY amount to cents or truncate a KWD amount to two decimals.
pub fn round_to_currency(amount: Decimal, currency_code: &str) -> MoneyResult<Decimal> {
    round_to_fixed(amount, currency_exponent(currency_code)?)
}

/// Rounds `amount` to an explicit minor-unit exponent.
pub fn round_to_fixed(amount: Decimal, exponent: u8) -> MoneyResult<Decimal> {
    minor_unit_factor(exponent)?;
    Ok(amount.round_dp_with_strategy(
        u32::from(exponent),
        RoundingStrategy::MidpointAwayFromZero,
    ))
}

/// Converts a major-unit amount into the currency's minor units.
///
/// The amount is first rounded to the currency exponent, then scaled; a value that no longer fits
/// the platform's `i64` minor units is an [`MoneyError::OutOfRange`] instead of a wrapped number.
pub fn to_minor_units(amount: Decimal, currency_code: &str) -> MoneyResult<i64> {
    to_fixed_point_units(amount, currency_exponent(currency_code)?)
}

/// Converts a major-unit amount into the currency's minor units, requiring exact precision.
///
/// Unlike [`to_minor_units`] this never rounds: an amount with more decimals than the currency
/// allows is an [`MoneyError::ExcessPrecision`]. Provider adapters use this shape so a fractional
/// request can never be sent to a PSP as a rounded amount.
pub fn to_minor_units_exact(amount: Decimal, currency_code: &str) -> MoneyResult<i64> {
    to_fixed_point_units_exact(amount, currency_exponent(currency_code)?)
}

/// Converts a major-unit amount into minor units with an explicit exponent (rounding).
pub fn to_fixed_point_units(amount: Decimal, exponent: u8) -> MoneyResult<i64> {
    let rounded = round_to_fixed(amount, exponent)?;
    scale_to_i64(amount, rounded, exponent)
}

/// Converts a major-unit amount into minor units with an explicit exponent, requiring exactness.
pub fn to_fixed_point_units_exact(amount: Decimal, exponent: u8) -> MoneyResult<i64> {
    let scaled = amount
        .checked_mul(minor_unit_factor(exponent)?)
        .ok_or_else(|| MoneyError::OutOfRange {
            amount: amount.to_string(),
            exponent,
        })?;
    if !scaled.fract().is_zero() {
        return Err(MoneyError::ExcessPrecision {
            amount: amount.to_string(),
            exponent,
        });
    }
    scaled.to_i64().ok_or_else(|| MoneyError::OutOfRange {
        amount: amount.to_string(),
        exponent,
    })
}

/// Converts minor units back into a major-unit amount for `currency_code`.
pub fn from_minor_units(minor_units: i64, currency_code: &str) -> MoneyResult<Decimal> {
    from_fixed_point_units(minor_units, currency_exponent(currency_code)?)
}

/// Converts minor units back into a major-unit amount with an explicit exponent.
pub fn from_fixed_point_units(minor_units: i64, exponent: u8) -> MoneyResult<Decimal> {
    Ok(Decimal::from(minor_units) / minor_unit_factor(exponent)?)
}

fn minor_unit_factor(exponent: u8) -> MoneyResult<Decimal> {
    if exponent > MAX_MINOR_UNIT_EXPONENT {
        return Err(MoneyError::UnsupportedExponent { exponent });
    }
    let mut factor = Decimal::ONE;
    for _ in 0..exponent {
        factor *= Decimal::from(10_u64);
    }
    Ok(factor)
}

fn scale_to_i64(original: Decimal, rounded: Decimal, exponent: u8) -> MoneyResult<i64> {
    let scaled = rounded
        .checked_mul(minor_unit_factor(exponent)?)
        .ok_or_else(|| MoneyError::OutOfRange {
            amount: original.to_string(),
            exponent,
        })?;
    scaled.to_i64().ok_or_else(|| MoneyError::OutOfRange {
        amount: original.to_string(),
        exponent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponents_follow_iso_4217() {
        assert_eq!(currency_exponent("usd").expect("USD"), 2);
        assert_eq!(currency_exponent(" jpy ").expect("JPY"), 0);
        assert_eq!(currency_exponent("ISK").expect("ISK"), 0);
        assert_eq!(currency_exponent("KWD").expect("KWD"), 3);
        assert_eq!(currency_exponent("CLF").expect("CLF"), 4);
        assert_eq!(currency_exponent("MGA").expect("MGA"), 2);
        assert_eq!(currency_exponent("XBT").expect("synthetic code"), 2);
        assert!(matches!(
            currency_exponent("XAU"),
            Err(MoneyError::UndefinedMinorUnit { .. })
        ));
        assert!(matches!(
            currency_exponent("us"),
            Err(MoneyError::InvalidCurrencyCode { .. })
        ));
    }

    #[test]
    fn minor_units_use_the_currency_exponent() {
        assert_eq!(to_minor_units(Decimal::new(2500, 2), "USD").expect("USD"), 2500);
        assert_eq!(to_minor_units(Decimal::new(2500, 2), "JPY").expect("JPY"), 25);
        assert_eq!(
            to_minor_units(Decimal::new(15778, 3), "KWD").expect("KWD"),
            15778
        );
        assert_eq!(to_minor_units(Decimal::new(25, 0), "JPY").expect("JPY"), 25);
        assert_eq!(
            from_minor_units(15778, "KWD").expect("KWD"),
            Decimal::new(15778, 3)
        );
    }

    #[test]
    fn exact_conversion_rejects_excess_precision() {
        assert!(matches!(
            to_minor_units_exact(Decimal::new(251, 3), "USD"),
            Err(MoneyError::ExcessPrecision { .. })
        ));
        assert!(matches!(
            to_fixed_point_units_exact(Decimal::new(1, 1), 0),
            Err(MoneyError::ExcessPrecision { .. })
        ));
        assert_eq!(
            to_fixed_point_units_exact(Decimal::new(2500, 2), 2).expect("USD"),
            2500
        );
    }

    #[test]
    fn rounding_uses_the_currency_precision() {
        assert_eq!(
            round_to_currency(Decimal::new(1577, 2), "JPY").expect("JPY"),
            Decimal::from(16)
        );
        assert_eq!(
            round_to_currency(Decimal::new(15778, 3), "KWD").expect("KWD"),
            Decimal::new(15778, 3)
        );
        assert_eq!(
            round_to_currency(Decimal::new(1005, 3), "USD").expect("USD"),
            Decimal::new(101, 2)
        );
    }

    #[test]
    fn out_of_range_and_unsupported_exponents_error() {
        assert!(matches!(
            to_minor_units(Decimal::from(i64::MAX), "USD"),
            Err(MoneyError::OutOfRange { .. })
        ));
        assert!(matches!(
            minor_unit_factor(10),
            Err(MoneyError::UnsupportedExponent { exponent: 10 })
        ));
        assert!(matches!(
            from_fixed_point_units(1, 10),
            Err(MoneyError::UnsupportedExponent { exponent: 10 })
        ));
    }
}
