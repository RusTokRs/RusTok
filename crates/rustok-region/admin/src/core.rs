use rustok_api::normalize_ui_text;

use crate::model::{RegionAdminBootstrap, RegionDetail, RegionDraft, RegionList};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminShellLabels {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminShellViewModel {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
}

pub fn build_region_admin_shell_view_model(
    labels: &RegionAdminShellLabels,
) -> RegionAdminShellViewModel {
    RegionAdminShellViewModel {
        badge: labels.badge.clone(),
        title: labels.title.clone(),
        subtitle: labels.subtitle.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminListHeaderLabels {
    pub title: String,
    pub subtitle_template: String,
    pub subtitle_fallback: String,
    pub refresh_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminListHeaderViewModel {
    pub title: String,
    pub subtitle: String,
    pub refresh_action: String,
}

pub fn build_region_admin_list_header_view_model(
    bootstrap: Option<&RegionAdminBootstrap>,
    labels: &RegionAdminListHeaderLabels,
) -> RegionAdminListHeaderViewModel {
    let tenant_name =
        bootstrap.and_then(|payload| optional_ui_text(payload.current_tenant.name.as_str()));
    let subtitle = tenant_name
        .map(|tenant| {
            labels
                .subtitle_template
                .replace("{tenant}", tenant.as_str())
        })
        .unwrap_or_else(|| labels.subtitle_fallback.clone());

    RegionAdminListHeaderViewModel {
        title: labels.title.clone(),
        subtitle,
        refresh_action: labels.refresh_action.clone(),
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
pub struct RegionAdminEditorLabels {
    pub create_title: String,
    pub edit_title: String,
    pub create_action: String,
    pub save_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorViewModel {
    pub is_editing: bool,
    pub title: String,
    pub submit_label: String,
}

pub fn build_region_admin_editor_view_model(
    editing_id: Option<&str>,
    labels: &RegionAdminEditorLabels,
) -> RegionAdminEditorViewModel {
    let is_editing = editing_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();

    RegionAdminEditorViewModel {
        is_editing,
        title: if is_editing {
            labels.edit_title.clone()
        } else {
            labels.create_title.clone()
        },
        submit_label: if is_editing {
            labels.save_action.clone()
        } else {
            labels.create_action.clone()
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorFieldLabels {
    pub name_placeholder: String,
    pub currency_code_placeholder: String,
    pub tax_provider_id_placeholder: String,
    pub tax_rate_placeholder: String,
    pub tax_included_label: String,
    pub country_tax_policies_placeholder: String,
    pub countries_placeholder: String,
    pub metadata_placeholder: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminEditorFieldViewModel {
    pub name_placeholder: String,
    pub currency_code_placeholder: String,
    pub tax_provider_id_placeholder: String,
    pub tax_rate_placeholder: String,
    pub tax_included_label: String,
    pub country_tax_policies_placeholder: String,
    pub countries_placeholder: String,
    pub metadata_placeholder: String,
}

pub fn build_region_admin_editor_field_view_model(
    labels: &RegionAdminEditorFieldLabels,
) -> RegionAdminEditorFieldViewModel {
    RegionAdminEditorFieldViewModel {
        name_placeholder: labels.name_placeholder.clone(),
        currency_code_placeholder: labels.currency_code_placeholder.clone(),
        tax_provider_id_placeholder: labels.tax_provider_id_placeholder.clone(),
        tax_rate_placeholder: labels.tax_rate_placeholder.clone(),
        tax_included_label: labels.tax_included_label.clone(),
        country_tax_policies_placeholder: labels.country_tax_policies_placeholder.clone(),
        countries_placeholder: labels.countries_placeholder.clone(),
        metadata_placeholder: labels.metadata_placeholder.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminRawSectionLabels {
    pub country_tax_policies_title: String,
    pub metadata_title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminRawSectionViewModel {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminRawSectionsViewModel {
    pub country_tax_policies: RegionAdminRawSectionViewModel,
    pub metadata: RegionAdminRawSectionViewModel,
}

pub fn build_region_admin_raw_sections_view_model(
    detail: &RegionDetail,
    labels: &RegionAdminRawSectionLabels,
) -> RegionAdminRawSectionsViewModel {
    RegionAdminRawSectionsViewModel {
        country_tax_policies: RegionAdminRawSectionViewModel {
            title: labels.country_tax_policies_title.clone(),
            body: detail.region.country_tax_policies_pretty.clone(),
        },
        metadata: RegionAdminRawSectionViewModel {
            title: labels.metadata_title.clone(),
            body: detail.region.metadata_pretty.clone(),
        },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminDetailHeaderLabels {
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminDetailHeaderViewModel {
    pub name: String,
    pub summary: String,
    pub meta: String,
    pub created: String,
    pub updated: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminListStateLabels {
    pub loading: String,
    pub empty: String,
    pub load_error_context: String,
    pub open_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionAdminListStateViewModel {
    Loading {
        message: String,
    },
    Error {
        message: String,
    },
    Empty {
        message: String,
    },
    Ready {
        items: Vec<RegionAdminListItemViewModel>,
        open_action: String,
    },
}

pub fn build_region_admin_list_state_view_model(
    regions: Option<Result<&RegionList, String>>,
    state_labels: &RegionAdminListStateLabels,
    item_labels: &RegionAdminListLabels,
) -> RegionAdminListStateViewModel {
    match regions {
        None => RegionAdminListStateViewModel::Loading {
            message: state_labels.loading.clone(),
        },
        Some(Err(err)) => RegionAdminListStateViewModel::Error {
            message: error_with_context(state_labels.load_error_context.as_str(), err.as_str()),
        },
        Some(Ok(list)) if list.items.is_empty() => RegionAdminListStateViewModel::Empty {
            message: state_labels.empty.clone(),
        },
        Some(Ok(list)) => RegionAdminListStateViewModel::Ready {
            items: list
                .items
                .iter()
                .map(|region| build_region_admin_list_item_view_model(region, item_labels))
                .collect(),
            open_action: state_labels.open_action.clone(),
        },
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

pub fn build_region_admin_detail_header_view_model(
    detail: &crate::model::RegionDetail,
    detail_labels: &RegionAdminDetailLabels,
    header_labels: &RegionAdminDetailHeaderLabels,
) -> RegionAdminDetailHeaderViewModel {
    RegionAdminDetailHeaderViewModel {
        name: detail.region.name.clone(),
        summary: format!(
            "{} | {}",
            detail.region.currency_code,
            region_admin_countries_summary(detail)
        ),
        meta: build_region_admin_detail_meta(detail, detail_labels),
        created: format!("{} {}", header_labels.created, detail.region.created_at),
        updated: format!("{} {}", header_labels.updated, detail.region.updated_at),
    }
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
    fn admin_shell_view_model_keeps_header_copy_outside_ui_runtime() {
        let labels = RegionAdminShellLabels {
            badge: "region".to_string(),
            title: "Region Operations".to_string(),
            subtitle: "Module-owned region workspace".to_string(),
        };

        let view_model = build_region_admin_shell_view_model(&labels);

        assert_eq!(view_model.badge, "region");
        assert_eq!(view_model.title, "Region Operations");
        assert_eq!(view_model.subtitle, "Module-owned region workspace");
    }

    #[test]
    fn admin_list_header_view_model_formats_tenant_subtitle_without_ui_runtime() {
        let labels = RegionAdminListHeaderLabels {
            title: "Regions".to_string(),
            subtitle_template: "Tenant {tenant} region policy owned by the region module."
                .to_string(),
            subtitle_fallback: "Tenant-scoped region policy owned by the region module."
                .to_string(),
            refresh_action: "Refresh".to_string(),
        };
        let bootstrap = RegionAdminBootstrap {
            current_tenant: crate::model::CurrentTenant {
                id: "tenant-1".to_string(),
                slug: "tenant".to_string(),
                name: "  Demo Tenant  ".to_string(),
            },
        };

        let view_model = build_region_admin_list_header_view_model(Some(&bootstrap), &labels);

        assert_eq!(view_model.title, "Regions");
        assert_eq!(
            view_model.subtitle,
            "Tenant Demo Tenant region policy owned by the region module."
        );
        assert_eq!(view_model.refresh_action, "Refresh");

        let fallback = build_region_admin_list_header_view_model(None, &labels);
        assert_eq!(
            fallback.subtitle,
            "Tenant-scoped region policy owned by the region module."
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
    fn admin_raw_sections_view_model_keeps_pretty_json_payloads() {
        let detail = crate::model::RegionDetail {
            region: crate::model::RegionRecord {
                id: "region-eu".to_string(),
                tenant_id: "tenant".to_string(),
                name: "Europe".to_string(),
                currency_code: "EUR".to_string(),
                tax_provider_id: None,
                tax_rate: "20.0".to_string(),
                tax_included: false,
                country_tax_policies_pretty: "[{\"country\":\"DE\"}]".to_string(),
                countries: vec!["DE".to_string(), "FR".to_string()],
                metadata_pretty: "{\"tier\":\"eu\"}".to_string(),
                created_at: "2026-06-12".to_string(),
                updated_at: "2026-06-12".to_string(),
            },
        };
        let labels = RegionAdminRawSectionLabels {
            country_tax_policies_title: "Country Tax Policies".to_string(),
            metadata_title: "Metadata".to_string(),
        };

        let view_model = build_region_admin_raw_sections_view_model(&detail, &labels);

        assert_eq!(
            view_model.country_tax_policies.title,
            "Country Tax Policies"
        );
        assert_eq!(
            view_model.country_tax_policies.body,
            "[{\"country\":\"DE\"}]"
        );
        assert_eq!(view_model.metadata.title, "Metadata");
        assert_eq!(view_model.metadata.body, "{\"tier\":\"eu\"}");
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
    fn admin_list_state_view_model_maps_loading_error_empty_and_ready_states() {
        let state_labels = RegionAdminListStateLabels {
            loading: "Loading regions...".to_string(),
            empty: "No regions have been created for this tenant yet.".to_string(),
            load_error_context: "Failed to load regions".to_string(),
            open_action: "Open".to_string(),
        };
        let item_labels = RegionAdminListLabels {
            tax_included: "tax included".to_string(),
            tax_excluded: "tax excluded".to_string(),
            countries: "countries".to_string(),
            tax_rate: "tax rate".to_string(),
            updated: "updated".to_string(),
        };

        assert_eq!(
            build_region_admin_list_state_view_model(None, &state_labels, &item_labels),
            RegionAdminListStateViewModel::Loading {
                message: "Loading regions...".to_string()
            }
        );

        assert_eq!(
            build_region_admin_list_state_view_model(
                Some(Err("network unavailable".to_string())),
                &state_labels,
                &item_labels,
            ),
            RegionAdminListStateViewModel::Error {
                message: "Failed to load regions: network unavailable".to_string()
            }
        );

        let empty_list = crate::model::RegionList { items: vec![] };
        assert_eq!(
            build_region_admin_list_state_view_model(
                Some(Ok(&empty_list)),
                &state_labels,
                &item_labels,
            ),
            RegionAdminListStateViewModel::Empty {
                message: "No regions have been created for this tenant yet.".to_string()
            }
        );

        let ready_list = crate::model::RegionList {
            items: vec![crate::model::RegionListItem {
                id: "region-eu".to_string(),
                name: "Europe".to_string(),
                currency_code: "EUR".to_string(),
                tax_provider_id: Some("vat".to_string()),
                country_count: 2,
                tax_rate: "20.0".to_string(),
                tax_included: true,
                countries_preview: "DE, FR".to_string(),
                updated_at: "2026-06-12".to_string(),
            }],
        };

        assert_eq!(
            build_region_admin_list_state_view_model(
                Some(Ok(&ready_list)),
                &state_labels,
                &item_labels,
            ),
            RegionAdminListStateViewModel::Ready {
                items: vec![RegionAdminListItemViewModel {
                    id: "region-eu".to_string(),
                    name: "Europe".to_string(),
                    badge_label: "tax included".to_string(),
                    summary: "EUR | DE, FR".to_string(),
                    meta: "2 countries | tax rate 20.0 | updated 2026-06-12".to_string(),
                }],
                open_action: "Open".to_string(),
            }
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

    #[test]
    fn admin_detail_header_view_model_formats_summary_and_timestamps() {
        let detail = crate::model::RegionDetail {
            region: crate::model::RegionRecord {
                id: "region-eu".to_string(),
                tenant_id: "tenant".to_string(),
                name: "Europe".to_string(),
                currency_code: "EUR".to_string(),
                tax_provider_id: Some("vat".to_string()),
                tax_rate: "20.0".to_string(),
                tax_included: true,
                country_tax_policies_pretty: "[]".to_string(),
                countries: vec!["DE".to_string(), "FR".to_string()],
                metadata_pretty: "{}".to_string(),
                created_at: "2026-06-11".to_string(),
                updated_at: "2026-06-12".to_string(),
            },
        };
        let detail_labels = RegionAdminDetailLabels {
            tax_included: "tax included".to_string(),
            tax_excluded: "tax excluded".to_string(),
            countries: "countries".to_string(),
            tax_rate: "tax rate".to_string(),
        };
        let header_labels = RegionAdminDetailHeaderLabels {
            created: "created".to_string(),
            updated: "updated".to_string(),
        };

        let view_model =
            build_region_admin_detail_header_view_model(&detail, &detail_labels, &header_labels);

        assert_eq!(view_model.name, "Europe");
        assert_eq!(view_model.summary, "EUR | DE, FR");
        assert_eq!(
            view_model.meta,
            "2 countries | tax rate 20.0 (tax included)"
        );
        assert_eq!(view_model.created, "created 2026-06-11");
        assert_eq!(view_model.updated, "updated 2026-06-12");
    }

    #[test]
    fn admin_editor_field_view_model_keeps_form_copy_outside_ui_runtime() {
        let labels = RegionAdminEditorFieldLabels {
            name_placeholder: "Region name".to_string(),
            currency_code_placeholder: "Currency code".to_string(),
            tax_provider_id_placeholder: "Tax provider ID (optional)".to_string(),
            tax_rate_placeholder: "Tax rate".to_string(),
            tax_included_label: "Prices already include tax".to_string(),
            country_tax_policies_placeholder: "Country tax policies JSON".to_string(),
            countries_placeholder: "Countries (BY, RU, KZ)".to_string(),
            metadata_placeholder: "Metadata JSON".to_string(),
        };

        let view_model = build_region_admin_editor_field_view_model(&labels);

        assert_eq!(view_model.name_placeholder, "Region name");
        assert_eq!(view_model.currency_code_placeholder, "Currency code");
        assert_eq!(
            view_model.tax_provider_id_placeholder,
            "Tax provider ID (optional)"
        );
        assert_eq!(view_model.tax_rate_placeholder, "Tax rate");
        assert_eq!(view_model.tax_included_label, "Prices already include tax");
        assert_eq!(
            view_model.country_tax_policies_placeholder,
            "Country tax policies JSON"
        );
        assert_eq!(view_model.countries_placeholder, "Countries (BY, RU, KZ)");
        assert_eq!(view_model.metadata_placeholder, "Metadata JSON");
    }

    #[test]
    fn admin_editor_view_model_selects_create_or_edit_copy_without_ui_runtime() {
        let labels = RegionAdminEditorLabels {
            create_title: "Create Region".to_string(),
            edit_title: "Edit Region".to_string(),
            create_action: "Create region".to_string(),
            save_action: "Save region".to_string(),
        };

        let create_view_model = build_region_admin_editor_view_model(None, &labels);
        assert!(!create_view_model.is_editing);
        assert_eq!(create_view_model.title, "Create Region");
        assert_eq!(create_view_model.submit_label, "Create region");

        let edit_view_model = build_region_admin_editor_view_model(Some("region-eu"), &labels);
        assert!(edit_view_model.is_editing);
        assert_eq!(edit_view_model.title, "Edit Region");
        assert_eq!(edit_view_model.submit_label, "Save region");

        let blank_view_model = build_region_admin_editor_view_model(Some("  "), &labels);
        assert!(!blank_view_model.is_editing);
        assert_eq!(blank_view_model.title, "Create Region");
        assert_eq!(blank_view_model.submit_label, "Create region");
    }
}
