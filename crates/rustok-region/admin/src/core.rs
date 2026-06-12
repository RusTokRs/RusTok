use rustok_api::normalize_ui_text;

use crate::model::{RegionDetail, RegionDraft};

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

pub const DEFAULT_TAX_RATE_INPUT: &str = "0";
pub const DEFAULT_COUNTRY_TAX_POLICIES_INPUT: &str = "[]";
pub const DEFAULT_METADATA_INPUT: &str = "{}";
pub const DEFAULT_TAX_PROVIDER_ID: &str = "region_default";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorFormState {
    pub editing_id: Option<String>,
    pub name: String,
    pub currency_code: String,
    pub tax_provider_id: String,
    pub tax_rate: String,
    pub tax_included: bool,
    pub country_tax_policies: String,
    pub countries: String,
    pub metadata: String,
}

impl RegionAdminEditorFormState {
    pub fn empty() -> Self {
        Self {
            editing_id: None,
            name: String::new(),
            currency_code: String::new(),
            tax_provider_id: String::new(),
            tax_rate: DEFAULT_TAX_RATE_INPUT.to_string(),
            tax_included: false,
            country_tax_policies: DEFAULT_COUNTRY_TAX_POLICIES_INPUT.to_string(),
            countries: String::new(),
            metadata: DEFAULT_METADATA_INPUT.to_string(),
        }
    }

    pub fn from_detail(detail: &RegionDetail) -> Self {
        Self {
            editing_id: Some(detail.region.id.clone()),
            name: detail.region.name.clone(),
            currency_code: detail.region.currency_code.clone(),
            tax_provider_id: detail.region.tax_provider_id.clone().unwrap_or_default(),
            tax_rate: detail.region.tax_rate.clone(),
            tax_included: detail.region.tax_included,
            country_tax_policies: detail.region.country_tax_policies_pretty.clone(),
            countries: detail.region.countries.join(", "),
            metadata: detail.region.metadata_pretty.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminPolicyLabels {
    pub currency: String,
    pub tax_provider: String,
    pub tax_rate: String,
    pub tax_included: String,
    pub tax_excluded: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminPolicyRowViewModel {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminPolicySectionViewModel {
    pub rows: Vec<RegionAdminPolicyRowViewModel>,
}

pub fn region_admin_countries_summary(detail: &RegionDetail) -> String {
    detail.region.countries.join(", ")
}

pub fn build_region_admin_policy_section_view_model(
    detail: &RegionDetail,
    labels: &RegionAdminPolicyLabels,
) -> RegionAdminPolicySectionViewModel {
    let tax_mode = if detail.region.tax_included {
        labels.tax_included.clone()
    } else {
        labels.tax_excluded.clone()
    };

    RegionAdminPolicySectionViewModel {
        rows: vec![
            RegionAdminPolicyRowViewModel {
                text: format!("{}: {}", labels.currency, detail.region.currency_code),
            },
            RegionAdminPolicyRowViewModel {
                text: format!(
                    "{}: {}",
                    labels.tax_provider,
                    detail
                        .region
                        .tax_provider_id
                        .clone()
                        .unwrap_or_else(|| DEFAULT_TAX_PROVIDER_ID.to_string())
                ),
            },
            RegionAdminPolicyRowViewModel {
                text: format!("{}: {}", labels.tax_rate, detail.region.tax_rate),
            },
            RegionAdminPolicyRowViewModel { text: tax_mode },
        ],
    }
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminListLabels {
    pub tax_included: String,
    pub tax_excluded: String,
    pub countries: String,
    pub tax_rate: String,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminListItemViewModel {
    pub id: String,
    pub name: String,
    pub badge_label: String,
    pub summary: String,
    pub meta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminDetailLabels {
    pub tax_included: String,
    pub tax_excluded: String,
    pub countries: String,
    pub tax_rate: String,
}

pub fn build_region_admin_list_item_view_model(
    region: &crate::model::RegionListItem,
    labels: &RegionAdminListLabels,
) -> RegionAdminListItemViewModel {
    RegionAdminListItemViewModel {
        id: region.id.clone(),
        name: region.name.clone(),
        badge_label: if region.tax_included {
            labels.tax_included.clone()
        } else {
            labels.tax_excluded.clone()
        },
        summary: format!("{} | {}", region.currency_code, region.countries_preview),
        meta: format!(
            "{} {} | {} {} | {} {}",
            region.country_count,
            labels.countries,
            labels.tax_rate,
            region.tax_rate,
            labels.updated,
            region.updated_at
        ),
    }
}

pub fn region_admin_list_item_class(is_selected: bool) -> &'static str {
    if is_selected {
        "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
    } else {
        "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40"
    }
}

pub fn build_region_admin_detail_meta(
    detail: &crate::model::RegionDetail,
    labels: &RegionAdminDetailLabels,
) -> String {
    let tax_state = if detail.region.tax_included {
        labels.tax_included.as_str()
    } else {
        labels.tax_excluded.as_str()
    };
    format!(
        "{} {} | {} {} ({tax_state})",
        detail.region.countries.len(),
        labels.countries,
        labels.tax_rate,
        detail.region.tax_rate
    )
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

    #[test]
    fn admin_editor_form_state_uses_core_owned_empty_defaults() {
        let state = RegionAdminEditorFormState::empty();

        assert_eq!(state.editing_id, None);
        assert_eq!(state.tax_rate, DEFAULT_TAX_RATE_INPUT);
        assert!(!state.tax_included);
        assert_eq!(
            state.country_tax_policies,
            DEFAULT_COUNTRY_TAX_POLICIES_INPUT
        );
        assert_eq!(state.metadata, DEFAULT_METADATA_INPUT);
    }

    #[test]
    fn admin_editor_form_state_maps_loaded_detail_without_ui_runtime() {
        let detail = crate::model::RegionDetail {
            region: crate::model::RegionRecord {
                id: "region-eu".to_string(),
                tenant_id: "tenant".to_string(),
                name: "Europe".to_string(),
                currency_code: "EUR".to_string(),
                tax_provider_id: Some("vat".to_string()),
                tax_rate: "20.0".to_string(),
                tax_included: true,
                country_tax_policies_pretty: "[{\"country\":\"DE\"}]".to_string(),
                countries: vec!["DE".to_string(), "FR".to_string()],
                metadata_pretty: "{\"tier\":\"eu\"}".to_string(),
                created_at: "2026-06-12".to_string(),
                updated_at: "2026-06-12".to_string(),
            },
        };

        let state = RegionAdminEditorFormState::from_detail(&detail);

        assert_eq!(state.editing_id.as_deref(), Some("region-eu"));
        assert_eq!(state.name, "Europe");
        assert_eq!(state.tax_provider_id, "vat");
        assert_eq!(state.countries, "DE, FR");
        assert_eq!(state.country_tax_policies, "[{\"country\":\"DE\"}]");
        assert_eq!(state.metadata, "{\"tier\":\"eu\"}");
    }

    #[test]
    fn admin_policy_section_view_model_applies_tax_provider_fallback() {
        let detail = crate::model::RegionDetail {
            region: crate::model::RegionRecord {
                id: "region-eu".to_string(),
                tenant_id: "tenant".to_string(),
                name: "Europe".to_string(),
                currency_code: "EUR".to_string(),
                tax_provider_id: None,
                tax_rate: "20.0".to_string(),
                tax_included: false,
                country_tax_policies_pretty: "[]".to_string(),
                countries: vec!["DE".to_string(), "FR".to_string()],
                metadata_pretty: "{}".to_string(),
                created_at: "2026-06-12".to_string(),
                updated_at: "2026-06-12".to_string(),
            },
        };
        let labels = RegionAdminPolicyLabels {
            currency: "currency".to_string(),
            tax_provider: "tax provider".to_string(),
            tax_rate: "tax rate".to_string(),
            tax_included: "tax included".to_string(),
            tax_excluded: "tax excluded".to_string(),
        };

        let view_model = build_region_admin_policy_section_view_model(&detail, &labels);

        assert_eq!(region_admin_countries_summary(&detail), "DE, FR");
        assert_eq!(view_model.rows[0].text, "currency: EUR");
        assert_eq!(
            view_model.rows[1].text,
            format!("tax provider: {DEFAULT_TAX_PROVIDER_ID}")
        );
        assert_eq!(view_model.rows[2].text, "tax rate: 20.0");
        assert_eq!(view_model.rows[3].text, "tax excluded");
    }

    #[test]
    fn admin_list_item_view_model_formats_policy_state_without_ui_runtime() {
        let region = crate::model::RegionListItem {
            id: "region-eu".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("vat".to_string()),
            country_count: 2,
            tax_rate: "20.0".to_string(),
            tax_included: true,
            countries_preview: "DE, FR".to_string(),
            updated_at: "2026-06-12".to_string(),
        };
        let labels = RegionAdminListLabels {
            tax_included: "tax included".to_string(),
            tax_excluded: "tax excluded".to_string(),
            countries: "countries".to_string(),
            tax_rate: "tax rate".to_string(),
            updated: "updated".to_string(),
        };

        let view_model = build_region_admin_list_item_view_model(&region, &labels);

        assert_eq!(view_model.id, "region-eu");
        assert_eq!(view_model.badge_label, "tax included");
        assert_eq!(view_model.summary, "EUR | DE, FR");
        assert_eq!(
            view_model.meta,
            "2 countries | tax rate 20.0 | updated 2026-06-12"
        );
        assert_eq!(
            region_admin_list_item_class(true),
            "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
        );
    }

    #[test]
    fn admin_detail_meta_uses_core_owned_tax_mode_policy() {
        let detail = crate::model::RegionDetail {
            region: crate::model::RegionRecord {
                id: "region-eu".to_string(),
                tenant_id: "tenant".to_string(),
                name: "Europe".to_string(),
                currency_code: "EUR".to_string(),
                tax_provider_id: None,
                tax_rate: "20.0".to_string(),
                tax_included: false,
                country_tax_policies_pretty: "[]".to_string(),
                countries: vec!["DE".to_string(), "FR".to_string()],
                metadata_pretty: "{}".to_string(),
                created_at: "2026-06-12".to_string(),
                updated_at: "2026-06-12".to_string(),
            },
        };
        let labels = RegionAdminDetailLabels {
            tax_included: "tax included".to_string(),
            tax_excluded: "tax excluded".to_string(),
            countries: "countries".to_string(),
            tax_rate: "tax rate".to_string(),
        };

        assert_eq!(
            build_region_admin_detail_meta(&detail, &labels),
            "2 countries | tax rate 20.0 (tax excluded)"
        );
    }
}
