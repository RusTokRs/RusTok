use rustok_api::normalize_ui_text;

use crate::model::{StorefrontRegion, StorefrontRegionCountryTaxPolicy, StorefrontRegionsData};

pub const DEFAULT_ROUTE_SEGMENT: &str = "regions";
pub const DEFAULT_TAX_PROVIDER_ID: &str = "region_default";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionMetric {
    pub title: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRailItemViewModel {
    pub href: String,
    pub tax_mode_label: String,
    pub country_summary: String,
    pub tax_summary: String,
    pub tax_provider_id: String,
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
    format!("{module_route_base}?region={region_id}")
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

pub fn rail_item_view_model(
    module_route_base: &str,
    region: &StorefrontRegion,
    included_label: &str,
    excluded_label: &str,
    empty_countries_label: &str,
    tax_rate_label: &str,
    tax_provider_label: &str,
) -> RegionRailItemViewModel {
    RegionRailItemViewModel {
        href: region_href(module_route_base, &region.id),
        tax_mode_label: tax_mode_label(region.tax_included, included_label, excluded_label),
        country_summary: rail_country_summary(region, empty_countries_label),
        tax_summary: rail_tax_summary(region, tax_rate_label, tax_provider_label),
        tax_provider_id: tax_provider_id_or_default(region.tax_provider_id.as_deref()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_region() -> StorefrontRegion {
        StorefrontRegion {
            id: "eu".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some(" default ".to_string()),
            tax_rate: "20".to_string(),
            tax_included: true,
            country_tax_policies: vec![StorefrontRegionCountryTaxPolicy {
                country_code: "DE".to_string(),
                tax_rate: "19".to_string(),
                tax_included: false,
            }],
            countries: vec!["DE".to_string(), "FR".to_string()],
        }
    }

    #[test]
    fn resolve_storefront_regions_normalizes_selection_and_defaults_to_first_region() {
        let regions = vec![sample_region()];
        let data = resolve_storefront_regions(regions.clone(), Some(" eu ".to_string()));

        assert_eq!(data.selected_region_id.as_deref(), Some("eu"));
        assert_eq!(
            data.selected_region
                .as_ref()
                .map(|region| region.id.as_str()),
            Some("eu")
        );

        let defaulted = resolve_storefront_regions(regions, Some("   ".to_string()));
        assert_eq!(defaulted.selected_region_id.as_deref(), Some("eu"));
        assert_eq!(
            defaulted
                .selected_region
                .as_ref()
                .map(|region| region.id.as_str()),
            Some("eu")
        );
    }

    #[test]
    fn resolve_storefront_regions_keeps_missing_selection_visible_without_selecting_item() {
        let data = resolve_storefront_regions(vec![sample_region()], Some("missing".to_string()));

        assert_eq!(data.selected_region_id.as_deref(), Some("missing"));
        assert!(data.selected_region.is_none());
    }

    #[test]
    fn route_segment_falls_back_without_ui_runtime() {
        assert_eq!(route_segment_or_default(Some(" storefront ")), "storefront");
        assert_eq!(route_segment_or_default(Some("   ")), DEFAULT_ROUTE_SEGMENT);
        assert_eq!(route_segment_or_default(None), DEFAULT_ROUTE_SEGMENT);
    }

    #[test]
    fn rail_view_model_formats_route_and_region_summaries() {
        let region = sample_region();
        let view_model = rail_item_view_model(
            "/modules/regions",
            &region,
            "tax included",
            "tax excluded",
            "none",
            "tax rate",
            "tax provider",
        );

        assert_eq!(view_model.href, "/modules/regions?region=eu");
        assert_eq!(view_model.tax_mode_label, "tax included");
        assert_eq!(view_model.country_summary, "EUR | DE, FR");
        assert_eq!(view_model.tax_summary, "20 tax rate | tax provider default");
        assert_eq!(view_model.tax_provider_id, "default");
    }

    #[test]
    fn selected_metrics_use_stable_fallbacks() {
        let mut region = sample_region();
        region.tax_provider_id = Some(" ".to_string());
        let metrics = selected_region_metrics(
            &region,
            "Currency".to_string(),
            "Tax rate".to_string(),
            "Provider".to_string(),
            "Coverage".to_string(),
            "Country policies".to_string(),
        );

        assert_eq!(metrics[2].value, DEFAULT_TAX_PROVIDER_ID);
        assert_eq!(metrics[3].value, "2");
        assert_eq!(metrics[4].value, "1");
    }

    #[test]
    fn country_policy_summary_uses_tax_mode_label() {
        let policy = StorefrontRegionCountryTaxPolicy {
            country_code: "FR".to_string(),
            tax_rate: "20".to_string(),
            tax_included: true,
        };

        assert_eq!(
            country_policy_summary(&policy, "included", "excluded"),
            "FR | 20 | included"
        );
    }
}
