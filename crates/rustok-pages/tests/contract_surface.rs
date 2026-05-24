#[test]
fn implementation_plan_tracks_contract_test_coverage() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("контрактные тесты покрывают все публичные use-case"),
        "implementation plan must include contract test checklist item"
    );
}

#[test]
fn module_manifest_declares_fba_builder_consumer_contract() {
    let manifest = include_str!("../rustok-module.toml");
    let value: toml::Value =
        toml::from_str(manifest).expect("rustok-module.toml must stay valid TOML");

    let dependency = value
        .get("dependencies")
        .and_then(|deps| deps.get("page_builder"))
        .expect("pages manifest must declare dependencies.page_builder");

    assert_eq!(
        dependency
            .get("module")
            .and_then(toml::Value::as_str)
            .expect("dependencies.page_builder.module is required"),
        "page-builder",
        "pages must consume external page-builder module"
    );

    assert_eq!(
        dependency
            .get("contract")
            .and_then(toml::Value::as_str)
            .expect("dependencies.page_builder.contract is required"),
        "grapesjs_v1",
        "pages and builder contract drifted"
    );

    assert_eq!(
        dependency
            .get("contract_version")
            .and_then(toml::Value::as_str)
            .expect("dependencies.page_builder.contract_version is required"),
        "1.0",
        "pages contract version drifted"
    );

    let consumer = value
        .get("fba")
        .and_then(|fba| fba.get("builder_consumer"))
        .expect("fba.builder_consumer metadata is required");
    assert_eq!(
        consumer
            .get("builder_contract_version")
            .and_then(toml::Value::as_str)
            .expect("fba.builder_consumer.builder_contract_version is required"),
        "1.0",
        "pages builder contract version drifted"
    );

    let degraded_modes = consumer
        .get("degraded_modes")
        .expect("fba.builder_consumer.degraded_modes must be defined");
    for key in ["builder_disabled", "preview_disabled", "publish_disabled"] {
        assert!(
            degraded_modes
                .get(key)
                .and_then(toml::Value::as_str)
                .is_some(),
            "missing degraded mode mapping: {key}"
        );
    }

    let profiles = consumer
        .get("toggle_profiles")
        .expect("fba.builder_consumer.toggle_profiles must be defined");
    for profile in ["all_on", "publish_off", "preview_off", "builder_off"] {
        assert!(
            profiles
                .get(profile)
                .and_then(toml::Value::as_array)
                .is_some(),
            "missing toggle profile: {profile}"
        );
    }

    let all_on = profiles
        .get("all_on")
        .and_then(toml::Value::as_array)
        .expect("toggle profile all_on must be an array");
    for expected in [
        "builder.enabled=true",
        "builder.preview.enabled=true",
        "builder.properties.enabled=true",
        "builder.publish.enabled=true",
    ] {
        assert!(
            all_on
                .iter()
                .any(|item| item.as_str().is_some_and(|value| value == expected)),
            "toggle profile all_on missing required switch: {expected}"
        );
    }

    let publish_off = profiles
        .get("publish_off")
        .and_then(toml::Value::as_array)
        .expect("toggle profile publish_off must be an array");
    assert!(
        publish_off.iter().any(|item| item
            .as_str()
            .is_some_and(|value| value == "builder.publish.enabled=false")),
        "toggle profile publish_off must explicitly disable builder.publish.enabled"
    );

    let preview_off = profiles
        .get("preview_off")
        .and_then(toml::Value::as_array)
        .expect("toggle profile preview_off must be an array");
    assert!(
        preview_off.iter().any(|item| item
            .as_str()
            .is_some_and(|value| value == "builder.preview.enabled=false")),
        "toggle profile preview_off must explicitly disable builder.preview.enabled"
    );
}

#[test]
fn pages_consumer_version_satisfies_provider_minimum() {
    let provider_manifest = include_str!("../../rustok-page-builder/rustok-module.toml");
    let provider: toml::Value =
        toml::from_str(provider_manifest).expect("provider rustok-module.toml must stay valid");
    let provider_min = provider
        .get("fba")
        .and_then(|fba| fba.get("provider"))
        .and_then(|provider| provider.get("consumer_min_version"))
        .and_then(toml::Value::as_str)
        .expect("fba.provider.consumer_min_version is required");

    let consumer_manifest = include_str!("../rustok-module.toml");
    let consumer: toml::Value =
        toml::from_str(consumer_manifest).expect("consumer rustok-module.toml must stay valid");
    let consumer_version = consumer
        .get("fba")
        .and_then(|fba| fba.get("builder_consumer"))
        .and_then(|builder_consumer| builder_consumer.get("builder_contract_version"))
        .and_then(toml::Value::as_str)
        .expect("fba.builder_consumer.builder_contract_version is required");

    fn semver_like_key(version: &str) -> Result<Vec<u32>, String> {
        version
            .split('.')
            .map(|segment| {
                if segment.is_empty() || !segment.chars().all(|char| char.is_ascii_digit()) {
                    return Err(format!(
                        "invalid numeric version segment in '{version}' (segment='{segment}')"
                    ));
                }
                segment.parse::<u32>().map_err(|error| {
                    format!("failed to parse numeric segment '{segment}' in '{version}': {error}")
                })
            })
            .collect()
    }

    let consumer_key = semver_like_key(consumer_version)
        .expect("consumer builder_contract_version must contain numeric dot segments");
    let provider_min_key = semver_like_key(provider_min)
        .expect("provider consumer_min_version must contain numeric dot segments");

    assert!(
        consumer_key >= provider_min_key,
        "pages builder_contract_version={} must be >= provider consumer_min_version={}",
        consumer_version,
        provider_min
    );
}

#[test]
fn semver_like_guard_rejects_non_numeric_segments() {
    fn semver_like_key(version: &str) -> Result<Vec<u32>, String> {
        version
            .split('.')
            .map(|segment| {
                if segment.is_empty() || !segment.chars().all(|char| char.is_ascii_digit()) {
                    return Err(format!(
                        "invalid numeric version segment in '{version}' (segment='{segment}')"
                    ));
                }
                segment.parse::<u32>().map_err(|error| {
                    format!("failed to parse numeric segment '{segment}' in '{version}': {error}")
                })
            })
            .collect()
    }

    let error = semver_like_key("1.x").expect_err("non-numeric segment must be rejected");
    assert!(
        error.contains("invalid numeric version segment"),
        "error must explain numeric-segment requirement, got: {error}"
    );
}
