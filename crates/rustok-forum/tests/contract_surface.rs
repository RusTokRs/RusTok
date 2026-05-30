#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("Contract tests cover the current public use-cases"),
        "implementation plan must include contract test checklist item"
    );
}

#[test]
fn module_manifest_declares_forum_widget_catalog_contract() {
    let manifest = include_str!("../rustok-module.toml");
    let value: toml::Value = toml::from_str(manifest).expect("rustok-module.toml must stay valid");

    let page_builder = value
        .get("dependencies")
        .and_then(|deps| deps.get("page_builder"))
        .expect("forum manifest must declare dependencies.page_builder");
    assert_eq!(
        page_builder
            .get("module")
            .and_then(toml::Value::as_str)
            .expect("dependencies.page_builder.module is required"),
        "page-builder"
    );
    assert_eq!(
        page_builder
            .get("contract")
            .and_then(toml::Value::as_str)
            .expect("dependencies.page_builder.contract is required"),
        "grapesjs_v1"
    );

    let builder_consumer = value
        .get("fba")
        .and_then(|fba| fba.get("builder_consumer"))
        .expect("fba.builder_consumer metadata is required");
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

    let error_mapping = builder_consumer
        .get("error_mapping")
        .expect("fba.builder_consumer.error_mapping is required");
    for key in ["validation", "sanitize", "rbac", "runtime"] {
        assert!(
            error_mapping.get(key).and_then(toml::Value::as_str).is_some(),
            "missing error mapping: {key}"
        );
    }
}
