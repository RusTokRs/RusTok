use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rustok_index::{FieldName, FieldPath, FilterExpr, IndexValue, LocaleKey};
use uuid::Uuid;

pub(crate) use rustok_product::ProductAttributeTermError;

pub(crate) const PRODUCT_ATTRIBUTE_TERMS_FIELD: &str = "attribute_terms";

/// PostgreSQL CTE fragment used by the replacement Product source to materialize every active,
/// Product-scoped, filterable EAV value into the Product-owned canonical term grammar.
///
/// The fragment is tenant-scoped by `$1` and intentionally aggregates terms by Product rather than by
/// Product translation locale. Localized text terms carry their own canonical locale identity, so one
/// localized Product record can express the exact requested-locale/fallback-locale predicate without a
/// dynamic Index schema field per attribute code.
pub(crate) const PRODUCT_ATTRIBUTE_TERMS_CTE: &str = r#"
product_filterable_attribute_values AS (
    SELECT
        pav.id AS value_id,
        pav.product_id,
        pa.id AS attribute_id,
        pa.value_type,
        pa.is_localized,
        pav.value_text,
        pav.value_integer,
        pav.value_decimal,
        pav.value_boolean,
        pav.value_date,
        pav.value_datetime
    FROM product_attribute_values pav
    JOIN product_attributes pa
      ON pa.id = pav.attribute_id
     AND pa.tenant_id = pav.tenant_id
    WHERE pav.tenant_id = $1
      AND pav.detached_at IS NULL
      AND pa.archived_at IS NULL
      AND pa.is_filterable = TRUE
      AND pa.scope IN ('product', 'both')
),
product_attribute_term_rows AS (
    SELECT
        value.product_id,
        value.attribute_id::text || '|text||'
            || encode(convert_to(value.value_text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type IN ('text', 'textarea', 'richtext')
      AND value.is_localized = FALSE
      AND value.value_text IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|localized_text|'
            || encode(convert_to(translation.locale, 'UTF8'), 'hex') || '|'
            || encode(convert_to(translation.value_text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    JOIN product_attribute_value_translations translation
      ON translation.value_id = value.value_id
    WHERE value.value_type IN ('text', 'textarea', 'richtext')
      AND value.is_localized = TRUE
      AND translation.value_text IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|localized_present|'
            || encode(convert_to(translation.locale, 'UTF8'), 'hex') || '|' AS term
    FROM product_filterable_attribute_values value
    JOIN product_attribute_value_translations translation
      ON translation.value_id = value.value_id
    WHERE value.value_type IN ('text', 'textarea', 'richtext')
      AND value.is_localized = TRUE

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|integer||'
            || encode(convert_to(value.value_integer::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'integer'
      AND value.value_integer IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|decimal||'
            || encode(convert_to(trim_scale(value.value_decimal)::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'decimal'
      AND value.value_decimal IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|boolean||'
            || encode(
                convert_to(CASE WHEN value.value_boolean THEN 'true' ELSE 'false' END, 'UTF8'),
                'hex'
            ) AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'boolean'
      AND value.value_boolean IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|date||'
            || encode(convert_to(value.value_date::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'date'
      AND value.value_date IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|datetime||'
            || encode(
                convert_to(
                    ((extract(epoch FROM value.value_datetime) * 1000000)::bigint)::text,
                    'UTF8'
                ),
                'hex'
            ) AS term
    FROM product_filterable_attribute_values value
    WHERE value.value_type = 'datetime'
      AND value.value_datetime IS NOT NULL

    UNION ALL

    SELECT
        value.product_id,
        value.attribute_id::text || '|option||'
            || encode(convert_to(option_value.option_id::text, 'UTF8'), 'hex') AS term
    FROM product_filterable_attribute_values value
    JOIN product_attribute_value_options option_value
      ON option_value.value_id = value.value_id
     AND option_value.tenant_id = $1
    WHERE value.value_type IN ('select', 'multiselect')
),
product_attribute_terms AS (
    SELECT
        product_id,
        jsonb_agg(term ORDER BY term) AS attribute_terms
    FROM (
        SELECT DISTINCT product_id, term
        FROM product_attribute_term_rows
    ) canonical_term
    GROUP BY product_id
)
"#;

pub(crate) fn text_term(attribute_id: Uuid, value: &str) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_text_term(attribute_id, value)
}

pub(crate) fn localized_text_term(
    attribute_id: Uuid,
    locale: &LocaleKey,
    value: &str,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_localized_text_term(attribute_id, locale.as_str(), value)
}

pub(crate) fn localized_presence_term(
    attribute_id: Uuid,
    locale: &LocaleKey,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_localized_presence_term(attribute_id, locale.as_str())
}

pub(crate) fn integer_term(attribute_id: Uuid, value: i64) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_integer_term(attribute_id, value)
}

pub(crate) fn decimal_term(
    attribute_id: Uuid,
    value: Decimal,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_decimal_term(attribute_id, value)
}

pub(crate) fn boolean_term(attribute_id: Uuid, value: bool) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_boolean_term(attribute_id, value)
}

pub(crate) fn date_term(
    attribute_id: Uuid,
    value: NaiveDate,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_date_term(attribute_id, value)
}

pub(crate) fn datetime_term(
    attribute_id: Uuid,
    value: DateTime<Utc>,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_datetime_term(attribute_id, value)
}

pub(crate) fn option_term(
    attribute_id: Uuid,
    option_id: Uuid,
) -> Result<String, ProductAttributeTermError> {
    rustok_product::product_attribute_option_term(attribute_id, option_id)
}

pub(crate) fn contains_term_filter(term: String) -> FilterExpr {
    FilterExpr::Contains(attribute_terms_path(), IndexValue::String(term))
}

/// Reproduces the owner localized-text predicate exactly:
/// requested-value OR (requested-locale-absent AND fallback-value).
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

pub(crate) fn attribute_terms_path() -> FieldPath {
    FieldPath::new(
        FieldName::new(PRODUCT_ATTRIBUTE_TERMS_FIELD)
            .expect("static Product attribute term field name must be valid"),
    )
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
