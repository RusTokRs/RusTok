#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("Contract tests cover the current public use-cases"),
        "implementation plan must include contract test checklist item"
    );
}

#[test]
fn module_manifest_declares_optional_forum_widget_catalog_contract() {
    let manifest = include_str!("../rustok-module.toml");
    let value: toml::Value = toml::from_str(manifest).expect("rustok-module.toml must stay valid");

    let dependencies = value
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .expect("forum manifest dependencies table is required");
    assert!(dependencies.contains_key("content"));
    assert!(dependencies.contains_key("taxonomy"));
    assert!(
        !dependencies.contains_key("page_builder"),
        "Page Builder is an optional FBA capability and must not disable Forum startup"
    );

    let builder_consumer = value
        .get("fba")
        .and_then(|fba| fba.get("builder_consumer"))
        .expect("fba.builder_consumer metadata is required");
    assert_eq!(
        builder_consumer
            .get("provider_module")
            .and_then(toml::Value::as_str)
            .expect("fba.builder_consumer.provider_module is required"),
        "page-builder"
    );
    assert_eq!(
        builder_consumer
            .get("contract")
            .and_then(toml::Value::as_str)
            .expect("fba.builder_consumer.contract is required"),
        "grapesjs_v1"
    );
    assert_eq!(
        builder_consumer
            .get("catalog_version")
            .and_then(toml::Value::as_str)
            .expect("fba.builder_consumer.catalog_version is required"),
        "v1"
    );

    let widgets = builder_consumer
        .get("widgets")
        .expect("fba.builder_consumer.widgets metadata is required");
    for widget_type in ["topic_list", "topic_detail", "reply_stream"] {
        let widget = widgets
            .get(widget_type)
            .expect("missing widget catalog entry in manifest");
        assert_eq!(
            widget
                .get("data_contract_version")
                .and_then(toml::Value::as_str)
                .expect("widget data_contract_version is required"),
            "1.0"
        );
        assert!(
            widget
                .get("props_schema")
                .and_then(toml::Value::as_str)
                .is_some(),
            "widget props_schema marker is required"
        );
    }

    let degraded_modes = builder_consumer
        .get("degraded_modes")
        .expect("fba.builder_consumer.degraded_modes is required");
    assert!(
        degraded_modes
            .get("builder_disabled")
            .and_then(toml::Value::as_str)
            .is_some(),
        "the optional builder capability must declare disabled behavior"
    );

    let error_mapping = builder_consumer
        .get("error_mapping")
        .expect("fba.builder_consumer.error_mapping is required");
    for key in ["validation", "sanitize", "rbac", "runtime"] {
        assert!(
            error_mapping
                .get(key)
                .and_then(toml::Value::as_str)
                .is_some(),
            "missing error mapping: {key}"
        );
    }
}
