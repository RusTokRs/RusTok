#[cfg(test)]
use chrono::{DateTime, NaiveDate, Utc};
#[cfg(test)]
use rust_decimal::Decimal;
#[cfg(test)]
use rustok_index::{FieldName, FieldPath, FilterExpr, IndexValue, LocaleKey};
#[cfg(test)]
use uuid::Uuid;

#[cfg(test)]
pub(crate) use rustok_product::ProductAttributeTermError;

#[cfg(test)]
pub(crate) const PRODUCT_ATTRIBUTE_TERMS_FIELD: &str = "attribute_terms";

#[cfg(test)]
pub(crate) fn text_term(
    attribute_id: Uuid,
    value: &str,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_text_term(attribute_id, value)
}

#[cfg(test)]
pub(crate) fn localized_text_term(
    attribute_id: Uuid,
    locale: &LocaleKey,
    value: &str,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_localized_text_term(attribute_id, locale.as_str(), value)
}

#[cfg(test)]
pub(crate) fn localized_presence_term(
    attribute_id: Uuid,
    locale: &LocaleKey,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_localized_presence_term(attribute_id, locale.as_str())
}

#[cfg(test)]
pub(crate) fn integer_term(
    attribute_id: Uuid,
    value: i64,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_integer_term(attribute_id, value)
}

#[cfg(test)]
pub(crate) fn decimal_term(
    attribute_id: Uuid,
    value: Decimal,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_decimal_term(attribute_id, value)
}

#[cfg(test)]
pub(crate) fn boolean_term(
    attribute_id: Uuid,
    value: bool,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_boolean_term(attribute_id, value)
}

#[cfg(test)]
pub(crate) fn date_term(
    attribute_id: Uuid,
    value: NaiveDate,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_date_term(attribute_id, value)
}

#[cfg(test)]
pub(crate) fn datetime_term(
    attribute_id: Uuid,
    value: DateTime<Utc>,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_datetime_term(attribute_id, value)
}

#[cfg(test)]
pub(crate) fn option_term(
    attribute_id: Uuid,
    option_id: Uuid,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_option_term(attribute_id, option_id)
}

#[cfg(test)]
pub(crate) fn contains_term_filter(term: String) -> FilterExpr {
    FilterExpr::Contains(attribute_terms_path(), IndexValue::String(term))
}

/// Reproduces the owner localized-text predicate exactly:
/// requested-value OR (requested-locale-absent AND fallback-value).
#[cfg(test)]
pub(crate) fn localized_text_filter(
    attribute_id: Uuid,
    requested_locale: &LocaleKey,
    fallback_locale: &LocaleKey,
    value: &str,
) -> Result<FilterExpr, ProductAttributeTermError> {
    let requested =
        contains_term_filter(localized_text_term(attribute_id, requested_locale, value)?);
    if requested_locale == fallback_locale {
        return Ok(requested);
    }

    let requested_present =
        contains_term_filter(localized_presence_term(attribute_id, requested_locale)?);
    let fallback = contains_term_filter(localized_text_term(attribute_id, fallback_locale, value)?);
    Ok(FilterExpr::Or(vec![
        requested,
        FilterExpr::And(vec![FilterExpr::Not(Box::new(requested_present)), fallback]),
    ]))
}

#[cfg(test)]
pub(crate) fn attribute_terms_path() -> FieldPath {
    let field = match FieldName::new(PRODUCT_ATTRIBUTE_TERMS_FIELD) {
        Ok(name) => name,
        Err(_) => unreachable!("static Product attribute term field name must be valid"),
    };
    FieldPath::new(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_terms_are_product_owned_and_materialization_compatible() {
        let attribute_id = Uuid::from_u128(1);
        assert_eq!(
            text_term(attribute_id, "A|b").unwrap(),
            "00000000-0000-0000-0000-000000000001|text||417c62"
        );
        assert_eq!(
            integer_term(attribute_id, 42).unwrap(),
            "00000000-0000-0000-0000-000000000001|integer||3432"
        );
        assert_eq!(
            date_term(attribute_id, NaiveDate::from_ymd_opt(2026, 8, 8).unwrap()).unwrap(),
            "00000000-0000-0000-0000-000000000001|date||323032362d30382d3038"
        );
        assert_eq!(
            decimal_term(attribute_id, Decimal::new(12_300, 3)).unwrap(),
            "00000000-0000-0000-0000-000000000001|decimal||31322e33"
        );
        assert_eq!(
            boolean_term(attribute_id, true).unwrap(),
            "00000000-0000-0000-0000-000000000001|boolean||74727565"
        );
    }

    #[test]
    fn localized_filter_preserves_requested_presence_fallback_semantics() {
        let attribute_id = Uuid::from_u128(1);
        let requested = LocaleKey::new("de-DE").unwrap();
        let fallback = LocaleKey::new("en-US").unwrap();
        let filter = localized_text_filter(attribute_id, &requested, &fallback, "Red").unwrap();
        let FilterExpr::Or(branches) = filter else {
            panic!("localized fallback must be an OR expression");
        };
        assert_eq!(branches.len(), 2);
        let FilterExpr::And(fallback_branch) = &branches[1] else {
            panic!("fallback branch must combine absence and fallback value");
        };
        assert_eq!(fallback_branch.len(), 2);
        assert!(matches!(fallback_branch[0], FilterExpr::Not(_)));
        assert!(matches!(fallback_branch[1], FilterExpr::Contains(_, _)));
    }

    #[test]
    fn option_and_datetime_terms_use_stable_storage_identities() {
        let attribute_id = Uuid::from_u128(1);
        let option_id = Uuid::from_u128(2);
        let timestamp = DateTime::<Utc>::from_timestamp_micros(1_725_000_123_456_789).unwrap();
        assert!(option_term(attribute_id, option_id).unwrap().ends_with(
            "|option||30303030303030302d303030302d303030302d303030302d303030303030303030303032"
        ));
        assert!(
            datetime_term(attribute_id, timestamp)
                .unwrap()
                .ends_with("|datetime||31373235303030313233343536373839")
        );
    }

    #[test]
    fn nil_storage_identities_fail_closed() {
        assert_eq!(
            text_term(Uuid::nil(), "x"),
            Err(ProductAttributeTermError::NilAttributeId)
        );
        assert_eq!(
            option_term(Uuid::from_u128(1), Uuid::nil()),
            Err(ProductAttributeTermError::NilOptionId)
        );
    }
}
