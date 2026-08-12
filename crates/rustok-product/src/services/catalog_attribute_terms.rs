use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rustok_api::normalize_locale_tag;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use super::catalog_schema::AttributeValueType;
use crate::error::{CommerceError, CommerceResult};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProductAttributeFilterValue {
    Text(String),
    Integer(i64),
    Decimal(Decimal),
    Boolean(bool),
    Date(NaiveDate),
    Datetime(DateTime<Utc>),
    Option(String),
}

pub(crate) fn parse_product_attribute_filter_value(
    code: &str,
    value_type: AttributeValueType,
    raw_value: &str,
) -> CommerceResult<ProductAttributeFilterValue> {
    match value_type {
        AttributeValueType::Text | AttributeValueType::Textarea | AttributeValueType::Richtext => {
            Ok(ProductAttributeFilterValue::Text(raw_value.to_string()))
        }
        AttributeValueType::Integer => raw_value
            .parse::<i64>()
            .map(ProductAttributeFilterValue::Integer)
            .map_err(|_| invalid_typed_value(code, "integer", raw_value)),
        AttributeValueType::Decimal => raw_value
            .parse::<Decimal>()
            .map(ProductAttributeFilterValue::Decimal)
            .map_err(|_| invalid_typed_value(code, "decimal", raw_value)),
        AttributeValueType::Boolean => match raw_value.to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(ProductAttributeFilterValue::Boolean(true)),
            "false" | "0" => Ok(ProductAttributeFilterValue::Boolean(false)),
            _ => Err(invalid_typed_value(code, "boolean", raw_value)),
        },
        AttributeValueType::Date => NaiveDate::parse_from_str(raw_value, "%Y-%m-%d")
            .map(ProductAttributeFilterValue::Date)
            .map_err(|_| invalid_typed_value(code, "date (YYYY-MM-DD)", raw_value)),
        AttributeValueType::Datetime => DateTime::parse_from_rfc3339(raw_value)
            .map(|value| ProductAttributeFilterValue::Datetime(value.with_timezone(&Utc)))
            .map_err(|_| invalid_typed_value(code, "RFC3339 datetime", raw_value)),
        AttributeValueType::Select | AttributeValueType::Multiselect => {
            Ok(ProductAttributeFilterValue::Option(raw_value.to_string()))
        }
        AttributeValueType::Json => Err(CommerceError::Validation(format!(
            "attribute {code} uses json and cannot be used in attribute_filters"
        ))),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductAttributeTermExpr {
    Term(String),
    And(Vec<ProductAttributeTermExpr>),
    Or(Vec<ProductAttributeTermExpr>),
    Not(Box<ProductAttributeTermExpr>),
    Never,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductResolvedAttributeFilter {
    pub code: String,
    pub predicate: ProductAttributeTermExpr,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProductAttributeTermError {
    #[error("Product attribute term attribute id must not be nil")]
    NilAttributeId,
    #[error("Product attribute term option id must not be nil")]
    NilOptionId,
    #[error("Product attribute term locale must be canonical")]
    InvalidLocale,
}

pub fn product_attribute_text_term(
    attribute_id: Uuid,
    value: &str,
) -> Result<String, ProductAttributeTermError> {
    product_attribute_term(attribute_id, "text", None, value)
}

pub fn product_attribute_localized_text_term(
    attribute_id: Uuid,
    locale: &str,
    value: &str,
) -> Result<String, ProductAttributeTermError> {
    let locale = canonical_locale(locale)?;
    product_attribute_term(attribute_id, "localized_text", Some(locale.as_str()), value)
}

pub fn product_attribute_localized_presence_term(
    attribute_id: Uuid,
    locale: &str,
) -> Result<String, ProductAttributeTermError> {
    let locale = canonical_locale(locale)?;
    product_attribute_term(attribute_id, "localized_present", Some(locale.as_str()), "")
}

pub fn product_attribute_integer_term(
    attribute_id: Uuid,
    value: i64,
) -> Result<String, ProductAttributeTermError> {
    product_attribute_term(attribute_id, "integer", None, value.to_string().as_str())
}

pub fn product_attribute_decimal_term(
    attribute_id: Uuid,
    value: Decimal,
) -> Result<String, ProductAttributeTermError> {
    product_attribute_term(
        attribute_id,
        "decimal",
        None,
        value.normalize().to_string().as_str(),
    )
}

pub fn product_attribute_boolean_term(
    attribute_id: Uuid,
    value: bool,
) -> Result<String, ProductAttributeTermError> {
    product_attribute_term(
        attribute_id,
        "boolean",
        None,
        if value { "true" } else { "false" },
    )
}

pub fn product_attribute_date_term(
    attribute_id: Uuid,
    value: NaiveDate,
) -> Result<String, ProductAttributeTermError> {
    product_attribute_term(
        attribute_id,
        "date",
        None,
        value.format("%Y-%m-%d").to_string().as_str(),
    )
}

pub fn product_attribute_datetime_term(
    attribute_id: Uuid,
    value: DateTime<Utc>,
) -> Result<String, ProductAttributeTermError> {
    product_attribute_term(
        attribute_id,
        "datetime",
        None,
        value.timestamp_micros().to_string().as_str(),
    )
}

pub fn product_attribute_option_term(
    attribute_id: Uuid,
    option_id: Uuid,
) -> Result<String, ProductAttributeTermError> {
    if option_id.is_nil() {
        return Err(ProductAttributeTermError::NilOptionId);
    }
    product_attribute_term(attribute_id, "option", None, option_id.to_string().as_str())
}

/// Canonical term expression for owner localized-text semantics:
/// requested-value OR (requested-locale-absent AND fallback-value).
pub fn product_attribute_localized_text_expr(
    attribute_id: Uuid,
    requested_locale: &str,
    fallback_locale: &str,
    value: &str,
) -> Result<ProductAttributeTermExpr, ProductAttributeTermError> {
    let requested_locale = canonical_locale(requested_locale)?;
    let fallback_locale = canonical_locale(fallback_locale)?;
    let requested = ProductAttributeTermExpr::Term(product_attribute_localized_text_term(
        attribute_id,
        requested_locale.as_str(),
        value,
    )?);
    if requested_locale == fallback_locale {
        return Ok(requested);
    }

    let requested_present = ProductAttributeTermExpr::Term(
        product_attribute_localized_presence_term(attribute_id, requested_locale.as_str())?,
    );
    let fallback = ProductAttributeTermExpr::Term(product_attribute_localized_text_term(
        attribute_id,
        fallback_locale.as_str(),
        value,
    )?);
    Ok(ProductAttributeTermExpr::Or(vec![
        requested,
        ProductAttributeTermExpr::And(vec![
            ProductAttributeTermExpr::Not(Box::new(requested_present)),
            fallback,
        ]),
    ]))
}

fn canonical_locale(locale: &str) -> Result<String, ProductAttributeTermError> {
    let trimmed = locale.trim();
    let normalized =
        normalize_locale_tag(trimmed).ok_or(ProductAttributeTermError::InvalidLocale)?;
    if normalized != trimmed {
        return Err(ProductAttributeTermError::InvalidLocale);
    }
    Ok(normalized)
}

fn product_attribute_term(
    attribute_id: Uuid,
    kind: &str,
    locale: Option<&str>,
    value: &str,
) -> Result<String, ProductAttributeTermError> {
    if attribute_id.is_nil() {
        return Err(ProductAttributeTermError::NilAttributeId);
    }
    let locale = locale.map(hex_encode).unwrap_or_default();
    Ok(format!(
        "{}|{}|{}|{}",
        attribute_id,
        kind,
        locale,
        hex_encode(value)
    ))
}

fn hex_encode(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = value.as_bytes();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn invalid_typed_value(code: &str, expected: &str, value: &str) -> CommerceError {
    CommerceError::Validation(format!(
        "attribute filter {code} expects {expected}, received `{value}`"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_scalar_terms_match_materialized_grammar() {
        let attribute_id = Uuid::from_u128(1);
        assert_eq!(
            product_attribute_text_term(attribute_id, "A|b").unwrap(),
            "00000000-0000-0000-0000-000000000001|text||417c62"
        );
        assert_eq!(
            product_attribute_decimal_term(attribute_id, Decimal::new(12_300, 3)).unwrap(),
            "00000000-0000-0000-0000-000000000001|decimal||31322e33"
        );
        assert_eq!(
            product_attribute_boolean_term(attribute_id, true).unwrap(),
            "00000000-0000-0000-0000-000000000001|boolean||74727565"
        );
    }

    #[test]
    fn shared_value_parser_keeps_owner_boolean_and_datetime_rules() {
        assert_eq!(
            parse_product_attribute_filter_value("flag", AttributeValueType::Boolean, "1").unwrap(),
            ProductAttributeFilterValue::Boolean(true)
        );
        assert!(matches!(
            parse_product_attribute_filter_value(
                "released",
                AttributeValueType::Datetime,
                "2026-08-08T06:00:00+03:00"
            )
            .unwrap(),
            ProductAttributeFilterValue::Datetime(_)
        ));
    }

    #[test]
    fn localized_expression_preserves_requested_presence_fallback() {
        let expr =
            product_attribute_localized_text_expr(Uuid::from_u128(1), "de-DE", "en-US", "Red")
                .unwrap();
        let ProductAttributeTermExpr::Or(branches) = expr else {
            panic!("localized fallback must be an OR expression");
        };
        assert_eq!(branches.len(), 2);
        let ProductAttributeTermExpr::And(fallback) = &branches[1] else {
            panic!("fallback branch must be an AND expression");
        };
        assert!(matches!(fallback[0], ProductAttributeTermExpr::Not(_)));
        assert!(matches!(fallback[1], ProductAttributeTermExpr::Term(_)));
    }

    #[test]
    fn noncanonical_locale_fails_closed() {
        assert_eq!(
            product_attribute_localized_text_term(Uuid::from_u128(1), "de_de", "x"),
            Err(ProductAttributeTermError::InvalidLocale)
        );
    }
}
