use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};
use rustok_ui_core::normalize_ui_text;

use crate::i18n::t;
use crate::model::{ShippingOption, ShippingOptionDraft, ShippingProfile};

pub const DEFAULT_SHIPPING_OPTION_PAGE: u64 = 1;
pub const DEFAULT_SHIPPING_OPTION_PER_PAGE: u64 = 24;
pub const DEFAULT_SHIPPING_PROFILE_PAGE: u64 = 1;
pub const DEFAULT_SHIPPING_PROFILE_PER_PAGE: u64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippingOptionListRequest {
    pub search: Option<String>,
    pub currency_code: Option<String>,
    pub provider_id: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippingProfileListRequest {
    pub page: u64,
    pub per_page: u64,
}

pub fn text_or_none(value: impl AsRef<str>) -> Option<String> {
    normalize_ui_text(value.as_ref())
}

pub fn shipping_option_list_request(
    search: impl AsRef<str>,
    currency_code: impl AsRef<str>,
    provider_id: impl AsRef<str>,
) -> ShippingOptionListRequest {
    ShippingOptionListRequest {
        search: text_or_none(search),
        currency_code: text_or_none(currency_code),
        provider_id: text_or_none(provider_id),
        page: DEFAULT_SHIPPING_OPTION_PAGE,
        per_page: DEFAULT_SHIPPING_OPTION_PER_PAGE,
    }
}

pub fn shipping_profile_list_request() -> ShippingProfileListRequest {
    ShippingProfileListRequest {
        page: DEFAULT_SHIPPING_PROFILE_PAGE,
        per_page: DEFAULT_SHIPPING_PROFILE_PER_PAGE,
    }
}

pub fn shipping_option_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);

    vec![
        GridColumnDef::new(
            "name",
            if is_ru {
                "Название"
            } else {
                "Option Name"
            },
        )
        .width(220)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(if is_ru {
                "Поиск по названию...".to_string()
            } else {
                "Search name...".to_string()
            }),
        }),
        GridColumnDef::new(
            "provider_id",
            if is_ru {
                "Провайдер"
            } else {
                "Provider"
            },
        )
        .width(130)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(if is_ru {
                "Провайдер...".to_string()
            } else {
                "Provider...".to_string()
            }),
        }),
        GridColumnDef::new("price", if is_ru { "Стоимость" } else { "Price" })
            .width(130)
            .align(ColumnAlign::Right),
        GridColumnDef::new("profiles", if is_ru { "Профили" } else { "Profiles" })
            .width(160)
            .align(ColumnAlign::Left)
            .not_sortable(),
        GridColumnDef::new("status", if is_ru { "Статус" } else { "Status" })
            .width(110)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "active".to_string(),
                        label: if is_ru {
                            "Активен".to_string()
                        } else {
                            "Active".to_string()
                        },
                    },
                    FilterOption {
                        value: "inactive".to_string(),
                        label: if is_ru {
                            "Неактивен".to_string()
                        } else {
                            "Inactive".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все статусы".to_string()
                } else {
                    "All statuses".to_string()
                }),
            }),
        GridColumnDef::new(
            "updated_at",
            if is_ru {
                "Обновлено"
            } else {
                "Updated"
            },
        )
        .width(140)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::DateRange {
            from_placeholder: Some(if is_ru {
                "С".to_string()
            } else {
                "From".to_string()
            }),
            to_placeholder: Some(if is_ru {
                "По".to_string()
            } else {
                "To".to_string()
            }),
        }),
        GridColumnDef::new("actions", "")
            .width(140)
            .align(ColumnAlign::Center)
            .not_sortable()
            .not_resizable()
            .not_filterable(),
    ]
}

pub fn matches_shipping_option_filter(
    item: &ShippingOption,
    col_id: &str,
    filter_val: &FilterValue,
) -> bool {
    match (col_id, filter_val) {
        ("name", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty() || item.name.to_lowercase().contains(&q)
        }
        ("provider_id", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty() || item.provider_id.to_lowercase().contains(&q)
        }
        ("status", FilterValue::Select(val)) => {
            let val = val.trim();
            if val.is_empty() {
                true
            } else if val.eq_ignore_ascii_case("active") {
                item.active
            } else if val.eq_ignore_ascii_case("inactive") {
                !item.active
            } else {
                true
            }
        }
        ("updated_at", FilterValue::DateRange { from, to }) => {
            let date = item
                .updated_at
                .split('T')
                .next()
                .unwrap_or(&item.updated_at)
                .trim();
            if let Some(f) = from {
                let f = f.trim();
                if !f.is_empty() && date < f {
                    return false;
                }
            }
            if let Some(t) = to {
                let t = t.trim();
                if !t.is_empty() && date > t {
                    return false;
                }
            }
            true
        }
        _ => true,
    }
}

pub fn validate_shipping_option_draft(draft: &ShippingOptionDraft) -> Result<(), &'static str> {
    if draft.name.trim().is_empty() {
        return Err("fulfillment.error.shippingOptionNameRequired");
    }
    Ok(())
}

pub fn summarize_shipping_option(locale: Option<&str>, option: &ShippingOption) -> String {
    format!(
        "{} | {} {} | {} {} | {} {}",
        option.name,
        option.currency_code,
        option.amount,
        t(
            locale,
            "fulfillment.summary.shippingOption.provider",
            "provider"
        ),
        option.provider_id,
        t(
            locale,
            "fulfillment.summary.shippingOption.profiles",
            "profiles"
        ),
        format_allowed_profiles(locale, option.allowed_shipping_profile_slugs.as_ref())
    )
}

pub fn format_allowed_profiles(locale: Option<&str>, profiles: Option<&Vec<String>>) -> String {
    match profiles {
        Some(values) if !values.is_empty() => values.join(", "),
        _ => t(locale, "fulfillment.common.all", "all"),
    }
}

pub fn format_selected_profiles(locale: Option<&str>, values: &[String]) -> String {
    let slugs = normalize_slug_list(values);
    if slugs.is_empty() {
        t(locale, "fulfillment.common.allCarts", "all carts")
    } else {
        slugs.join(", ")
    }
}

pub fn profile_choice_label(locale: Option<&str>, profile: &ShippingProfile) -> String {
    if profile.active {
        format!("{} ({})", profile.name, profile.slug)
    } else {
        format!(
            "{} ({}, {})",
            profile.name,
            profile.slug,
            t(locale, "fulfillment.common.inactive", "inactive")
        )
    }
}

pub fn localized_active_label(locale: Option<&str>, active: bool) -> String {
    if active {
        t(locale, "fulfillment.common.active", "ACTIVE")
    } else {
        t(locale, "fulfillment.common.inactive", "INACTIVE")
    }
}

pub fn profile_chip_class(selected: bool, inactive: bool) -> &'static str {
    match (selected, inactive) {
        (true, false) => {
            "inline-flex rounded-full border border-primary bg-primary/10 px-3 py-2 text-xs font-medium text-primary transition hover:bg-primary/15"
        }
        (true, true) => {
            "inline-flex rounded-full border border-amber-300 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-700 transition hover:bg-amber-100"
        }
        (false, true) => {
            "inline-flex rounded-full border border-border bg-muted px-3 py-2 text-xs font-medium text-muted-foreground opacity-60"
        }
        (false, false) => {
            "inline-flex rounded-full border border-border bg-background px-3 py-2 text-xs font-medium text-foreground transition hover:bg-accent"
        }
    }
}

pub fn toggle_slug_selection(current: &mut Vec<String>, slug: &str) {
    let slug = slug.trim();
    if slug.is_empty() {
        return;
    }
    let mut values = normalize_slug_list(current);
    if let Some(position) = values.iter().position(|value| value == slug) {
        values.remove(position);
    } else {
        values.push(slug.to_string());
        values.sort();
        values.dedup();
    }
    *current = values;
}

pub fn slug_selected(current: &[String], slug: &str) -> bool {
    let slug = slug.trim();
    if slug.is_empty() {
        return false;
    }
    normalize_slug_list(current)
        .iter()
        .any(|value| value == slug)
}

pub fn normalize_slug_list(current: &[String]) -> Vec<String> {
    current
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn active_badge(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}

pub fn filter_shipping_options(
    items: &[ShippingOption],
    filters: &ColumnFilters,
) -> Vec<ShippingOption> {
    items
        .iter()
        .filter(|item| {
            filters.iter().all(|(col_id, filter_val)| {
                matches_shipping_option_filter(item, col_id, filter_val)
            })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_shipping_option(
        name: &str,
        provider: &str,
        active: bool,
        updated_at: &str,
    ) -> ShippingOption {
        ShippingOption {
            id: format!("opt-{}", name.to_lowercase().replace(' ', "-")),
            tenant_id: "tenant-1".to_string(),
            name: name.to_string(),
            currency_code: "USD".to_string(),
            amount: "15.00".to_string(),
            provider_id: provider.to_string(),
            active,
            allowed_shipping_profile_slugs: Some(vec!["standard".to_string()]),
            metadata: "{}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: updated_at.to_string(),
            translations: Vec::new(),
            translation_revision: "1".to_string(),
        }
    }

    #[test]
    fn shipping_option_request_trims_filters_and_uses_defaults() {
        let request = shipping_option_list_request(" express ", " USD ", " manual ");

        assert_eq!(request.search.as_deref(), Some("express"));
        assert_eq!(request.currency_code.as_deref(), Some("USD"));
        assert_eq!(request.provider_id.as_deref(), Some("manual"));
        assert_eq!(request.page, DEFAULT_SHIPPING_OPTION_PAGE);
        assert_eq!(request.per_page, DEFAULT_SHIPPING_OPTION_PER_PAGE);
    }

    #[test]
    fn blank_filter_normalizes_to_none() {
        assert_eq!(text_or_none("  "), None);
    }

    #[test]
    fn shipping_profile_request_uses_registry_defaults() {
        let request = shipping_profile_list_request();

        assert_eq!(request.page, DEFAULT_SHIPPING_PROFILE_PAGE);
        assert_eq!(request.per_page, DEFAULT_SHIPPING_PROFILE_PER_PAGE);
    }

    #[test]
    fn shipping_option_grid_columns_return_canonical_columns() {
        let cols_en = shipping_option_grid_columns(Some("en"));
        assert_eq!(cols_en.len(), 7);
        assert_eq!(cols_en[0].id, "name");
        assert_eq!(cols_en[0].title, "Option Name");
        assert_eq!(cols_en[4].id, "status");

        let cols_ru = shipping_option_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Название");
        assert_eq!(cols_ru[4].title, "Статус");
    }

    #[test]
    fn filter_shipping_options_by_name_and_status() {
        let options = vec![
            sample_shipping_option("Express Air", "dhl", true, "2026-03-01T12:00:00Z"),
            sample_shipping_option("Ground Freight", "fedex", false, "2026-03-05T12:00:00Z"),
            sample_shipping_option("Standard Post", "manual", true, "2026-03-10T12:00:00Z"),
        ];

        let mut filters = ColumnFilters::new();
        filters.set("name", FilterValue::Text("express".to_string()));
        let filtered = filter_shipping_options(&options, &filters);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Express Air");

        let mut status_filters = ColumnFilters::new();
        status_filters.set("status", FilterValue::Select("active".to_string()));
        let filtered_status = filter_shipping_options(&options, &status_filters);
        assert_eq!(filtered_status.len(), 2);
    }

    #[test]
    fn filter_shipping_options_handles_whitespace_in_status_and_date_range() {
        let options = vec![
            sample_shipping_option("Express Air", "dhl", true, "2026-03-01T12:00:00Z"),
            sample_shipping_option("Ground Freight", "fedex", false, "2026-03-05T12:00:00Z"),
        ];

        let mut filters = ColumnFilters::new();
        filters.set("status", FilterValue::Select("  ACTIVE  ".to_string()));
        assert_eq!(filter_shipping_options(&options, &filters).len(), 1);

        let mut date_filters = ColumnFilters::new();
        date_filters.set(
            "updated_at",
            FilterValue::DateRange {
                from: Some("  2026-03-02  ".to_string()),
                to: Some("  2026-03-06  ".to_string()),
            },
        );
        let date_matched = filter_shipping_options(&options, &date_filters);
        assert_eq!(date_matched.len(), 1);
        assert_eq!(date_matched[0].name, "Ground Freight");
    }

    #[test]
    fn draft_validation_checks_name_presence() {
        let draft = ShippingOptionDraft {
            name: "   ".to_string(),
            currency_code: "USD".to_string(),
            amount: "10.00".to_string(),
            provider_id: "manual".to_string(),
            allowed_shipping_profile_slugs: vec![],
            metadata_json: "{}".to_string(),
            locale: "en".to_string(),
            existing_translations: vec![],
            expected_translation_revision: None,
        };
        assert!(validate_shipping_option_draft(&draft).is_err());

        let valid_draft = ShippingOptionDraft {
            name: "Express".to_string(),
            ..draft
        };
        assert!(validate_shipping_option_draft(&valid_draft).is_ok());
    }

    #[test]
    fn slug_helpers_normalize_and_toggle() {
        let mut list = vec!["  express  ".to_string(), "ground".to_string()];
        assert_eq!(
            normalize_slug_list(&list),
            vec!["express".to_string(), "ground".to_string()]
        );
        assert!(slug_selected(&list, "express"));
        assert!(!slug_selected(&list, "freight"));

        toggle_slug_selection(&mut list, "freight");
        assert!(slug_selected(&list, "freight"));
        toggle_slug_selection(&mut list, "freight");
        assert!(!slug_selected(&list, "freight"));
    }

    #[test]
    fn presentation_helpers_render_correctly() {
        assert_eq!(
            active_badge(true),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );
        assert_eq!(
            active_badge(false),
            "border-slate-200 bg-slate-100 text-slate-700"
        );
        assert!(profile_chip_class(true, false).contains("border-primary"));

        let profile = ShippingProfile {
            id: "p1".to_string(),
            tenant_id: "t1".to_string(),
            slug: "standard".to_string(),
            name: "Standard".to_string(),
            description: None,
            active: true,
        };
        assert_eq!(profile_choice_label(Some("en"), &profile), "Standard (standard)");

        let option = sample_shipping_option("Standard Post", "manual", true, "2026-03-10T12:00:00Z");
        let summary = summarize_shipping_option(Some("en"), &option);
        assert!(summary.contains("Standard Post"));
        assert!(summary.contains("manual"));
    }
}
