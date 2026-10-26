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
fn core_view_models_pure_unit_test() {
    use crate::core::index_table_grid_columns;

    let cols_en = index_table_grid_columns(Some("en"));
    assert_eq!(cols_en.len(), 2);
    assert_eq!(cols_en[0].title, "Table Name");
    assert_eq!(cols_en[1].title, "Table Role");
}
