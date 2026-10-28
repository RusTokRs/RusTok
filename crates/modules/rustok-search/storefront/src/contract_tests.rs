#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn core_contains_no_ui_framework_dependencies() {
    let core_src = include_str!("core.rs");
    assert!(!core_src.contains("use leptos::"));
    assert!(!core_src.contains("use dioxus::"));
    assert!(!core_src.contains("view! {"));
    assert!(!core_src.contains("rsx! {"));
}

#[test]
fn route_filter_query_parsing_is_pure_and_deterministic() {
    use crate::core::{SearchRouteFilterQuery, parse_search_route_filters};

    let query = SearchRouteFilterQuery {
        channel_id: Some("default-channel".to_string()),
        entity_types: Some("product,article".to_string()),
        source_modules: Some("commerce,blog".to_string()),
        statuses: Some("published".to_string()),
        category_ids: Some("1,2,3".to_string()),
        attribute_code: Some("color".to_string()),
        attribute_values: Some("red,blue".to_string()),
        attribute_min: None,
        attribute_max: None,
        sort_attribute_code: Some("price".to_string()),
        sort_desc: Some("true".to_string()),
    };

    let parsed = parse_search_route_filters(query);
    assert_eq!(parsed.channel_id.as_deref(), Some("default-channel"));
    assert_eq!(parsed.entity_types, vec!["product", "article"]);
    assert_eq!(parsed.source_modules, vec!["commerce", "blog"]);
    assert_eq!(parsed.statuses, vec!["published"]);
    assert_eq!(parsed.category_ids, vec!["1", "2", "3"]);
    assert_eq!(parsed.attribute_code.as_deref(), Some("color"));
    assert_eq!(parsed.attribute_values, vec!["red", "blue"]);
    assert!(parsed.sort_desc);
}
