use rustok_ui_core::normalize_ui_text;

use crate::model::{RegionAdminBootstrap, RegionDetail, RegionList};

use super::editor::error_with_context;

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
        bootstrap.and_then(|payload| normalize_ui_text(payload.current_tenant.name.as_str()));
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

pub const DEFAULT_TAX_PROVIDER_ID: &str = "region_default";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminDetailPanelLabels {
    pub title: String,
    pub subtitle: String,
    pub policy_title: String,
    pub countries_title: String,
    pub empty: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAdminDetailPanelReadyViewModel {
    pub title: String,
    pub subtitle: String,
    pub policy_title: String,
    pub countries_title: String,
    pub countries_summary: String,
    pub header: RegionAdminDetailHeaderViewModel,
    pub policy: RegionAdminPolicySectionViewModel,
    pub raw_sections: RegionAdminRawSectionsViewModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionAdminDetailPanelViewModel {
    Empty {
        message: String,
    },
    Ready {
        detail: Box<RegionAdminDetailPanelReadyViewModel>,
    },
}

#[allow(clippy::too_many_arguments)]
pub fn build_region_admin_detail_panel_view_model(
    detail: Option<&RegionDetail>,
    panel_labels: &RegionAdminDetailPanelLabels,
    detail_labels: &RegionAdminDetailLabels,
    header_labels: &RegionAdminDetailHeaderLabels,
    policy_labels: &RegionAdminPolicyLabels,
    raw_section_labels: &RegionAdminRawSectionLabels,
) -> RegionAdminDetailPanelViewModel {
    detail
        .map(|detail| RegionAdminDetailPanelViewModel::Ready {
            detail: Box::new(RegionAdminDetailPanelReadyViewModel {
                title: panel_labels.title.clone(),
                subtitle: panel_labels.subtitle.clone(),
                policy_title: panel_labels.policy_title.clone(),
                countries_title: panel_labels.countries_title.clone(),
                countries_summary: region_admin_countries_summary(detail),
                header: build_region_admin_detail_header_view_model(
                    detail,
                    detail_labels,
                    header_labels,
                ),
                policy: build_region_admin_policy_section_view_model(detail, policy_labels),
                raw_sections: build_region_admin_raw_sections_view_model(
                    detail,
                    raw_section_labels,
                ),
            }),
        })
        .unwrap_or_else(|| RegionAdminDetailPanelViewModel::Empty {
            message: panel_labels.empty.clone(),
        })
}
