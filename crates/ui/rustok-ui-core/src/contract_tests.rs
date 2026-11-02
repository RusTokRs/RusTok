use super::*;

#[test]
fn test_status_badge_tone_and_classes() {
    assert_eq!(status_badge_tone("active"), UiBadgeTone::Success);
    assert_eq!(status_badge_tone("published"), UiBadgeTone::Success);
    assert_eq!(status_badge_tone("pending"), UiBadgeTone::Warning);
    assert_eq!(status_badge_tone("failed"), UiBadgeTone::Danger);
    assert_eq!(status_badge_tone("archived"), UiBadgeTone::Neutral);
    assert_eq!(status_badge_tone("unknown_state"), UiBadgeTone::Muted);

    assert!(!status_badge_class("active").is_empty());
    assert!(!status_badge_class("pending").is_empty());
    assert!(!status_badge_class("suspended").is_empty());
    assert!(!status_badge_class("draft").is_empty());

    assert!(!ui_badge_container_class(UiBadgeTone::Success).is_empty());
    assert!(!ui_badge_container_class(UiBadgeTone::Warning).is_empty());
    assert!(!ui_badge_container_class(UiBadgeTone::Danger).is_empty());
    assert!(!ui_badge_container_class(UiBadgeTone::Info).is_empty());
    assert!(!ui_badge_container_class(UiBadgeTone::Neutral).is_empty());
}

#[test]
fn test_css_hex_normalization_and_accents() {
    assert_eq!(
        normalize_css_hex_color("#fff"),
        Some("#fff".to_string())
    );
    assert_eq!(
        normalize_css_hex_color("#123456"),
        Some("#123456".to_string())
    );
    assert_eq!(normalize_css_hex_color("invalid"), None);

    assert!(!css_hex_accent_class(Some("#ffffff")).is_empty());
    assert!(!css_background_accent_class("primary").is_empty());
}

#[test]
fn test_money_formatting() {
    let formatted = format_ui_price(1999, "USD");
    assert!(formatted.contains("19.99") || formatted.contains("19,99") || formatted.contains("19"));
    assert!(formatted.contains("USD") || formatted.contains('$'));

    let badge = format_ui_discount_badge(2000, 1500);
    assert_eq!(badge, Some("-25%".to_string()));

    let no_discount = format_ui_discount_badge(2000, 2000);
    assert_eq!(no_discount, None);
}

#[test]
fn test_pagination_and_sorting_contracts() {
    let pagination = UiPaginationState::new(2, 10, Some(25));
    assert_eq!(pagination.page, 2);
    assert_eq!(pagination.per_page, 10);
    assert_eq!(pagination.total_items, Some(25));
    assert_eq!(pagination.total_pages(), Some(3));
    assert_eq!(pagination.offset(), 10);

    let mut sort = UiSortState::new("created_at", UiSortDirection::Asc);
    assert_eq!(sort.field.as_deref(), Some("created_at"));
    assert_eq!(sort.direction, UiSortDirection::Asc);

    sort.sort_by("created_at");
    assert_eq!(sort.direction, UiSortDirection::Desc);

    sort.sort_by("title");
    assert_eq!(sort.field.as_deref(), Some("title"));
    assert_eq!(sort.direction, UiSortDirection::Asc);
}

#[test]
fn test_navigation_and_breadcrumbs() {
    let groups = vec![UiNavGroup {
        key: "System",
        items: vec![UiNavItem {
            label: "Settings".to_string(),
            order: 1,
            children: vec![UiNavChild {
                href: "/dashboard/settings".to_string(),
                label: "General".to_string(),
            }],
        }],
    }];

    let breadcrumbs = build_ui_breadcrumbs("/dashboard/settings", &groups, "Dashboard");
    assert_eq!(breadcrumbs.len(), 3);
    assert_eq!(breadcrumbs[0].label, "Dashboard");

    assert!(ui_href_is_active("/dashboard", None, "/dashboard"));
    assert!(ui_href_is_active("/", None, "/dashboard"));
    assert!(!ui_href_is_active("/modules/blog", None, "/dashboard"));
}

#[test]
fn test_route_query_lifecycle() {
    let base_url = "/admin/products?category=books&page=1";
    let updated = apply_ui_query_pairs(
        base_url,
        &[("page", Some("2".to_string())), ("category", None)],
    );
    assert!(updated.contains("page=2"));
    assert!(!updated.contains("category=books"));
}

#[test]
fn test_selection_state_contracts() {
    let mut selection = UiSelectionState::<String>::new();
    assert!(selection.is_empty());

    selection.toggle("item-1".to_string());
    assert!(selection.is_selected(&"item-1".to_string()));
    assert_eq!(selection.selected.len(), 1);

    selection.toggle("item-1".to_string());
    assert!(!selection.is_selected(&"item-1".to_string()));
    assert!(selection.is_empty());
}

#[test]
fn test_text_helpers_and_busy_keys() {
    assert_eq!(
        normalize_ui_text("  hello world  "),
        Some("hello world".to_string())
    );
    assert_eq!(normalize_ui_text("   "), None);
    assert_eq!(parse_ui_csv("a, b, , c"), vec!["a", "b", "c"]);

    let key = ui_busy_key_with_id("save", "item-1");
    assert_eq!(key, "save:item-1");
    assert!(ui_busy_key_matches_action(Some(&key), "save"));
}
