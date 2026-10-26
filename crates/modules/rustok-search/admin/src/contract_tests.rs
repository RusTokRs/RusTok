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
    use crate::core::{facet_bucket_label, facet_display_name, score_label};

    assert_eq!(facet_display_name("blog_post"), "blog post");
    assert_eq!(facet_bucket_label("electronics", 42), "electronics (42)");
    assert_eq!(score_label(0.87654), "score 0.877");
}
