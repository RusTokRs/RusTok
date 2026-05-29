use rustok_api::normalize_ui_text;

use crate::model::RegionDraft;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionFormInput<'a> {
    pub name: &'a str,
    pub locale: &'a str,
    pub currency_code: &'a str,
    pub tax_provider_id: &'a str,
    pub tax_rate: &'a str,
    pub tax_included: bool,
    pub country_tax_policies: &'a str,
    pub countries: &'a str,
    pub metadata: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionRequiredField {
    Name,
    CurrencyCode,
    Countries,
}

pub fn optional_ui_text(value: &str) -> Option<String> {
    normalize_ui_text(value)
}

pub fn ui_text_or_default(value: &str) -> String {
    normalize_ui_text(value).unwrap_or_default()
}

pub fn build_region_draft(input: RegionFormInput<'_>) -> RegionDraft {
    RegionDraft {
        name: ui_text_or_default(input.name),
        locale: ui_text_or_default(input.locale),
        currency_code: ui_text_or_default(input.currency_code),
        tax_provider_id: ui_text_or_default(input.tax_provider_id),
        tax_rate: ui_text_or_default(input.tax_rate),
        tax_included: input.tax_included,
        country_tax_policies: ui_text_or_default(input.country_tax_policies),
        countries: ui_text_or_default(input.countries),
        metadata: ui_text_or_default(input.metadata),
    }
}

pub fn missing_required_region_field(input: &RegionDraft) -> Option<RegionRequiredField> {
    if input.name.is_empty() {
        Some(RegionRequiredField::Name)
    } else if input.currency_code.is_empty() {
        Some(RegionRequiredField::CurrencyCode)
    } else if input.countries.is_empty() {
        Some(RegionRequiredField::Countries)
    } else {
        None
    }
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_draft_builder_normalizes_form_state_without_ui_runtime() {
        let draft = build_region_draft(RegionFormInput {
            name: "  Europe  ",
            locale: " en ",
            currency_code: " eur ",
            tax_provider_id: " default ",
            tax_rate: " 20.0 ",
            tax_included: true,
            country_tax_policies: " [] ",
            countries: " DE, FR ",
            metadata: " {} ",
        });

        assert_eq!(draft.name, "Europe");
        assert_eq!(draft.locale, "en");
        assert_eq!(draft.currency_code, "eur");
        assert_eq!(draft.tax_provider_id, "default");
        assert_eq!(draft.tax_rate, "20.0");
        assert!(draft.tax_included);
        assert_eq!(draft.country_tax_policies, "[]");
        assert_eq!(draft.countries, "DE, FR");
        assert_eq!(draft.metadata, "{}");
        assert_eq!(missing_required_region_field(&draft), None);
    }

    #[test]
    fn required_region_fields_reject_blank_values() {
        let draft = build_region_draft(RegionFormInput {
            name: " ",
            locale: "en",
            currency_code: "EUR",
            tax_provider_id: "",
            tax_rate: "0",
            tax_included: false,
            country_tax_policies: "[]",
            countries: "DE",
            metadata: "{}",
        });

        assert_eq!(
            missing_required_region_field(&draft),
            Some(RegionRequiredField::Name)
        );
    }
}
