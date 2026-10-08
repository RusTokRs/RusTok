use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};
use rustok_ui_core::normalize_ui_text;

use crate::model::ShippingOption;

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
                .unwrap_or(&item.updated_at);
            if let Some(f) = from {
                if !f.is_empty() && date < f.as_str() {
                    return false;
                }
            }
            if let Some(t) = to {
                if !t.is_empty() && date > t.as_str() {
                    return false;
                }
            }
            true
        }
        _ => true,
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
}
