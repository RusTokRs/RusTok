use rustok_ui_core::{UiRouteQueryUpdate, normalize_ui_text};

use crate::model::{StorefrontRegion, StorefrontRegionCountryTaxPolicy, StorefrontRegionsData};

pub const DEFAULT_ROUTE_SEGMENT: &str = "regions";
pub const DEFAULT_TAX_PROVIDER_ID: &str = "region_default";
pub const REGION_ERROR_STATUS_DOM_ATTRIBUTE: &str = "data-region-error-status";
pub const REGION_ERROR_LOCALE_KEY_DOM_ATTRIBUTE: &str = "data-region-error-locale-key";
pub const SELECTED_REGION_QUERY_KEY: &str = "region";
pub const REGION_ROUTE_QUERY_KEY_DOM_ATTRIBUTE: &str = "data-region-route-query-key";
pub const REGION_ROUTE_QUERY_VALUE_DOM_ATTRIBUTE: &str = "data-region-route-query-value";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionStorefrontErrorPath {
    NativeServer,
    Graphql,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionErrorStatusCode {
    NativeUnavailable,
    GraphqlUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionErrorStatusDescriptor {
    pub code: RegionErrorStatusCode,
    pub stable_code: &'static str,
    pub locale_key: &'static str,
}

const REGION_ERROR_STATUS_DESCRIPTORS: [RegionErrorStatusDescriptor; 2] = [
    RegionErrorStatusDescriptor {
        code: RegionErrorStatusCode::NativeUnavailable,
        stable_code: "native_unavailable",
        locale_key: "region.error.status.nativeUnavailable",
    },
    RegionErrorStatusDescriptor {
        code: RegionErrorStatusCode::GraphqlUnavailable,
        stable_code: "graphql_unavailable",
        locale_key: "region.error.status.graphqlUnavailable",
    },
];

fn region_error_status_descriptor(
    code: RegionErrorStatusCode,
) -> &'static RegionErrorStatusDescriptor {
    match code {
        RegionErrorStatusCode::NativeUnavailable => &REGION_ERROR_STATUS_DESCRIPTORS[0],
        RegionErrorStatusCode::GraphqlUnavailable => &REGION_ERROR_STATUS_DESCRIPTORS[1],
    }
}

impl RegionErrorStatusCode {
    pub fn as_str(self) -> &'static str {
        region_error_status_descriptor(self).stable_code
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionErrorEvidence {
    pub failed_path: RegionStorefrontErrorPath,
    pub fallback_attempted: bool,
    pub native_error: Option<String>,
    pub graphql_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionErrorViewModel {
    pub status_code: RegionErrorStatusCode,
    pub status_locale_key: &'static str,
    pub status_label: String,
    pub title: String,
    pub body: String,
    pub technical_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionErrorCopy {
    pub title: String,
    pub native_unavailable_body: String,
    pub graphql_unavailable_body: String,
    pub native_status_label: String,
    pub graphql_status_label: String,
    pub native_label: String,
    pub graphql_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionErrorDomEvidence {
    pub status_attribute: &'static str,
    pub status_value: &'static str,
    pub locale_key_attribute: &'static str,
    pub locale_key_value: &'static str,
}

pub fn region_error_view_model(
    evidence: RegionErrorEvidence,
    copy: RegionErrorCopy,
) -> RegionErrorViewModel {
    let status_code = region_error_status_code(&evidence);
    let status_locale_key = region_error_status_descriptor(status_code).locale_key;
    let (status_label, body) = match status_code {
        RegionErrorStatusCode::NativeUnavailable => {
            (copy.native_status_label, copy.native_unavailable_body)
        }
        RegionErrorStatusCode::GraphqlUnavailable => {
            (copy.graphql_status_label, copy.graphql_unavailable_body)
        }
    };
    let technical_detail =
        region_error_technical_detail(&evidence, &copy.native_label, &copy.graphql_label);

    RegionErrorViewModel {
        status_code,
        status_locale_key,
        status_label,
        title: copy.title,
        body,
        technical_detail,
    }
}

pub fn region_error_dom_evidence(error: &RegionErrorViewModel) -> RegionErrorDomEvidence {
    RegionErrorDomEvidence {
        status_attribute: REGION_ERROR_STATUS_DOM_ATTRIBUTE,
        status_value: error.status_code.as_str(),
        locale_key_attribute: REGION_ERROR_LOCALE_KEY_DOM_ATTRIBUTE,
        locale_key_value: error.status_locale_key,
    }
}

pub fn region_error_status_code(evidence: &RegionErrorEvidence) -> RegionErrorStatusCode {
    if evidence.fallback_attempted || evidence.failed_path == RegionStorefrontErrorPath::Graphql {
        RegionErrorStatusCode::GraphqlUnavailable
    } else {
        RegionErrorStatusCode::NativeUnavailable
    }
}

fn region_error_technical_detail(
    evidence: &RegionErrorEvidence,
    native_label: &str,
    graphql_label: &str,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(native_error) = evidence
        .native_error
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("{native_label}: {native_error}"));
    }
    if let Some(graphql_error) = evidence
        .graphql_error
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("{graphql_label}: {graphql_error}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRouteState {
    pub route_segment: String,
    pub selected_region_id: Option<String>,
    pub selected_region_query_key: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRouteSelectionUpdate {
    pub query_key: &'static str,
    pub update: UiRouteQueryUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionMetric {
    pub title: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRegionCardViewModel {
    pub name: String,
    pub currency_code: String,
    pub tax_mode_label: String,
    pub countries_count_label: String,
    pub metrics: Vec<RegionMetric>,
    pub countries_summary: String,
    pub country_policy_summaries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRailLabels {
    pub title: String,
    pub total_template: String,
    pub empty_message: String,
    pub open_label: String,
    pub tax_included_label: String,
    pub tax_excluded_label: String,
    pub empty_countries_label: String,
    pub tax_rate_label: String,
    pub tax_provider_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRailItemViewModel {
    pub name: String,
    pub href: String,
    pub query_key_attribute: &'static str,
    pub query_value_attribute: &'static str,
    pub query_key: &'static str,
    pub query_value: Option<String>,
    pub tax_mode_label: String,
    pub country_summary: String,
    pub tax_summary: String,
    pub tax_provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRailViewModel {
    pub title: String,
    pub total_label: String,
    pub empty_message: String,
    pub open_label: String,
    pub items: Vec<RegionRailItemViewModel>,
}

pub fn normalize_selected_region_id(selected_region_id: Option<String>) -> Option<String> {
    selected_region_id.and_then(|value| normalize_ui_text(&value))
}

pub fn resolve_storefront_regions(
    regions: Vec<StorefrontRegion>,
    selected_region_id: Option<String>,
) -> StorefrontRegionsData {
    let resolved_selected_region_id = normalize_selected_region_id(selected_region_id)
        .or_else(|| regions.first().map(|item| item.id.clone()));
    let selected_region = resolved_selected_region_id
        .as_ref()
        .and_then(|selected_id| regions.iter().find(|item| &item.id == selected_id))
        .cloned();

    StorefrontRegionsData {
        regions,
        selected_region,
        selected_region_id: resolved_selected_region_id,
    }
}

pub fn route_segment_or_default(route_segment: Option<&str>) -> String {
    route_segment
        .and_then(normalize_ui_text)
        .unwrap_or_else(|| DEFAULT_ROUTE_SEGMENT.to_string())
}

pub fn region_route_state(
    route_segment: Option<&str>,
    selected_region_id: Option<String>,
) -> RegionRouteState {
    RegionRouteState {
        route_segment: route_segment_or_default(route_segment),
        selected_region_id: normalize_selected_region_id(selected_region_id),
        selected_region_query_key: SELECTED_REGION_QUERY_KEY,
    }
}

pub fn selected_region_query_update(region_id: &str) -> RegionRouteSelectionUpdate {
    let update = normalize_ui_text(region_id)
        .map(UiRouteQueryUpdate::Replace)
        .unwrap_or(UiRouteQueryUpdate::Clear);

    RegionRouteSelectionUpdate {
        query_key: SELECTED_REGION_QUERY_KEY,
        update,
    }
}

pub fn tax_mode_label(tax_included: bool, included_label: &str, excluded_label: &str) -> String {
    if tax_included {
        included_label.to_string()
    } else {
        excluded_label.to_string()
    }
}

pub fn tax_provider_id_or_default(tax_provider_id: Option<&str>) -> String {
    tax_provider_id
        .and_then(normalize_ui_text)
        .unwrap_or_else(|| DEFAULT_TAX_PROVIDER_ID.to_string())
}

pub fn count_label(template: &str, count: usize) -> String {
    template.replace("{count}", &count.to_string())
}

pub fn country_count_label(count: usize, countries_label: &str) -> String {
    format!("{count} {countries_label}")
}

pub fn countries_summary(countries: &[String], empty_label: &str) -> String {
    if countries.is_empty() {
        empty_label.to_string()
    } else {
        countries.join(", ")
    }
}

pub fn region_href(module_route_base: &str, region_id: &str) -> String {
    let selection_update = selected_region_query_update(region_id);
    region_href_from_selection_update(module_route_base, &selection_update)
}

pub fn region_href_from_selection_update(
    module_route_base: &str,
    selection_update: &RegionRouteSelectionUpdate,
) -> String {
    match selection_update.clone().update.into_query_value() {
        Some(region_id) => format!(
            "{module_route_base}?{}={region_id}",
            selection_update.query_key
        ),
        None => module_route_base.to_string(),
    }
}

pub fn rail_country_summary(region: &StorefrontRegion, empty_countries_label: &str) -> String {
    format!(
        "{} | {}",
        region.currency_code,
        countries_summary(&region.countries, empty_countries_label)
    )
}

pub fn rail_tax_summary(
    region: &StorefrontRegion,
    tax_rate_label: &str,
    tax_provider_label: &str,
) -> String {
    format!(
        "{} {} | {} {}",
        region.tax_rate,
        tax_rate_label,
        tax_provider_label,
        tax_provider_id_or_default(region.tax_provider_id.as_deref())
    )
}

pub fn country_policy_summary(
    policy: &StorefrontRegionCountryTaxPolicy,
    included_label: &str,
    excluded_label: &str,
) -> String {
    format!(
        "{} | {} | {}",
        policy.country_code,
        policy.tax_rate,
        tax_mode_label(policy.tax_included, included_label, excluded_label)
    )
}

pub fn selected_region_metrics(
    region: &StorefrontRegion,
    currency_title: String,
    tax_rate_title: String,
    tax_provider_title: String,
    coverage_title: String,
    country_policy_count_title: String,
) -> Vec<RegionMetric> {
    vec![
        RegionMetric {
            title: currency_title,
            value: region.currency_code.clone(),
        },
        RegionMetric {
            title: tax_rate_title,
            value: region.tax_rate.clone(),
        },
        RegionMetric {
            title: tax_provider_title,
            value: tax_provider_id_or_default(region.tax_provider_id.as_deref()),
        },
        RegionMetric {
            title: coverage_title,
            value: region.countries.len().to_string(),
        },
        RegionMetric {
            title: country_policy_count_title,
            value: region.country_tax_policies.len().to_string(),
        },
    ]
}

pub struct SelectedRegionCardCopy<'a> {
    pub tax_included_label: &'a str,
    pub tax_excluded_label: &'a str,
    pub countries_label: &'a str,
    pub no_countries_label: &'a str,
    pub currency_title: String,
    pub tax_rate_title: String,
    pub tax_provider_title: String,
    pub coverage_title: String,
    pub country_policy_count_title: String,
}

pub fn selected_region_card_view_model(
    region: &StorefrontRegion,
    copy: SelectedRegionCardCopy<'_>,
) -> SelectedRegionCardViewModel {
    SelectedRegionCardViewModel {
        name: region.name.clone(),
        currency_code: region.currency_code.clone(),
        tax_mode_label: tax_mode_label(
            region.tax_included,
            copy.tax_included_label,
            copy.tax_excluded_label,
        ),
        countries_count_label: country_count_label(region.countries.len(), copy.countries_label),
        metrics: selected_region_metrics(
            region,
            copy.currency_title,
            copy.tax_rate_title,
            copy.tax_provider_title,
            copy.coverage_title,
            copy.country_policy_count_title,
        ),
        countries_summary: countries_summary(&region.countries, copy.no_countries_label),
        country_policy_summaries: region
            .country_tax_policies
            .iter()
            .map(|policy| {
                country_policy_summary(policy, copy.tax_included_label, copy.tax_excluded_label)
            })
            .collect(),
    }
}

pub fn rail_item_view_model(
    module_route_base: &str,
    region: &StorefrontRegion,
    included_label: &str,
    excluded_label: &str,
    empty_countries_label: &str,
    tax_rate_label: &str,
    tax_provider_label: &str,
) -> RegionRailItemViewModel {
    let selection_update = selected_region_query_update(&region.id);
    let query_value = selection_update.clone().update.into_query_value();

    RegionRailItemViewModel {
        name: region.name.clone(),
        href: region_href(module_route_base, &region.id),
        query_key_attribute: REGION_ROUTE_QUERY_KEY_DOM_ATTRIBUTE,
        query_value_attribute: REGION_ROUTE_QUERY_VALUE_DOM_ATTRIBUTE,
        query_key: selection_update.query_key,
        query_value,
        tax_mode_label: tax_mode_label(region.tax_included, included_label, excluded_label),
        country_summary: rail_country_summary(region, empty_countries_label),
        tax_summary: rail_tax_summary(region, tax_rate_label, tax_provider_label),
        tax_provider_id: tax_provider_id_or_default(region.tax_provider_id.as_deref()),
    }
}

pub fn region_rail_view_model(
    module_route_base: &str,
    regions: &[StorefrontRegion],
    total: usize,
    labels: RegionRailLabels,
) -> RegionRailViewModel {
    let items = regions
        .iter()
        .map(|region| {
            rail_item_view_model(
                module_route_base,
                region,
                labels.tax_included_label.as_str(),
                labels.tax_excluded_label.as_str(),
                labels.empty_countries_label.as_str(),
                labels.tax_rate_label.as_str(),
                labels.tax_provider_label.as_str(),
            )
        })
        .collect();

    RegionRailViewModel {
        title: labels.title,
        total_label: count_label(&labels.total_template, total),
        empty_message: labels.empty_message,
        open_label: labels.open_label,
        items,
    }
}

#[cfg(test)]
mod tests;
