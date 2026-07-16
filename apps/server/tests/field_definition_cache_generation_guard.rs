#[test]
fn field_definition_cache_uses_one_transactional_generation_and_fail_closed_recovery() {
    let helper = include_str!("../../../crates/flex/src/cache_generation.rs");
    let wrapper = include_str!("../src/services/field_definition_cache.rs");
    let base = include_str!("../src/services/field_definition_cache_base.rs");
    let reconciler = include_str!("../src/services/field_definition_cache_reconciliation.rs");
    let guardrails = include_str!("../src/services/runtime_guardrails.rs");

    assert!(helper.contains(
        "pub const FIELD_DEFINITION_CACHE_GENERATION_TABLE"
    ));
    assert!(helper.contains("flex_field_definition_cache_generation"));
    assert!(helper.contains("generation = generation + 1"));
    assert!(helper.contains("AFTER INSERT OR UPDATE OR DELETE"));
    assert!(helper.contains("for operation in [\"insert\", \"update\", \"delete\"]"));
    assert!(helper.contains("validate_identifier(table_name)?"));
    assert!(helper.contains("validate_identifier(trigger_name)?"));

    assert!(wrapper.contains("#[path = \"field_definition_cache_base.rs\"]"));
    assert!(wrapper.contains(
        "let cache = base::field_definition_cache_from_context(ctx, bus);"
    ));
    assert!(wrapper.contains(
        "start_field_definition_cache_generation_reconciliation(ctx, cache.clone())"
    ));
    assert!(base.contains("pub fn field_definition_cache_from_context"));
    assert!(base.contains("cache.invalidate_all();"));

    let durable_read = reconciler
        .find("let mut applied = read_field_definition_cache_generation(&db)")
        .expect("startup must seed from durable generation");
    let startup_clear = reconciler[durable_read..]
        .find("cache.invalidate_all();")
        .map(|offset| durable_read + offset)
        .expect("startup must clear before trusting local cache");
    let startup_ack = reconciler[startup_clear..]
        .find("state.applied_generation.store(applied, Ordering::Release)")
        .map(|offset| startup_clear + offset)
        .expect("startup must record the generation only after clearing");
    let startup_healthy = reconciler[startup_ack..]
        .find("state.healthy.store(true, Ordering::Release)")
        .map(|offset| startup_ack + offset)
        .expect("startup must become healthy only after clear and acknowledgement");
    assert!(durable_read < startup_clear);
    assert!(startup_clear < startup_ack);
    assert!(startup_ack < startup_healthy);

    let advance = reconciler
        .find("if current < applied")
        .expect("generation regression must be checked");
    let advance_unhealthy = reconciler[advance..]
        .find("state.healthy.store(false, Ordering::Release)")
        .map(|offset| advance + offset)
        .expect("recovery must fail closed while advancing");
    let advance_clear = reconciler[advance_unhealthy..]
        .find("cache.invalidate_all();")
        .map(|offset| advance_unhealthy + offset)
        .expect("generation advance must clear the namespace");
    let advance_ack = reconciler[advance_clear..]
        .find("state.applied_generation.store(applied, Ordering::Release)")
        .map(|offset| advance_clear + offset)
        .expect("generation must be acknowledged only after clearing");
    assert!(advance_unhealthy < advance_clear);
    assert!(advance_clear < advance_ack);

    assert!(reconciler.contains("field-definition cache generation regressed"));
    assert!(reconciler.contains("struct AbortOnDropFieldDefinitionCacheGenerationTask"));
    assert!(reconciler.contains("self.task.abort();"));
    assert!(reconciler.contains(".catch_unwind()"));
    assert!(!reconciler.contains("invalidate_all().await"));

    assert!(guardrails.contains(
        "ctx.shared_get::<FieldDefinitionCacheGenerationReconciliationHandle>()"
    ));
    assert!(guardrails.contains(
        "Flex field-definition durable cache reconciliation"
    ));
}

#[test]
fn field_definition_generation_triggers_follow_table_creation_order() {
    let auth = include_str!(
        "../../../crates/rustok-auth/src/migrations/m20260716_000001_create_flex_field_definition_cache_generation.rs"
    );
    let product = include_str!(
        "../../../crates/rustok-product/src/migrations/m20260716_000002_add_product_field_cache_generation_trigger.rs"
    );
    let order = include_str!(
        "../../../crates/rustok-commerce/src/migrations/m20260716_000003_add_order_field_cache_generation_trigger.rs"
    );
    let topic = include_str!(
        "../../../crates/rustok-forum/src/migrations/m20260716_000004_add_topic_field_cache_generation_trigger.rs"
    );

    assert!(auth.contains("create_field_definition_cache_generation_table"));
    assert!(auth.contains("user_field_definitions"));
    assert!(product.contains("product_field_definitions"));
    assert!(order.contains("order_field_definitions"));
    assert!(topic.contains("topic_field_definitions"));

    let auth_mod = include_str!("../../../crates/rustok-auth/src/migrations/mod.rs");
    let product_mod = include_str!("../../../crates/rustok-product/src/migrations/mod.rs");
    let commerce_mod = include_str!("../../../crates/rustok-commerce/src/migrations/mod.rs");
    let forum_mod = include_str!("../../../crates/rustok-forum/src/migrations/mod.rs");
    assert!(auth_mod.contains("m20260716_000001_create_flex_field_definition_cache_generation"));
    assert!(product_mod.contains("m20260716_000002_add_product_field_cache_generation_trigger"));
    assert!(commerce_mod.contains("m20260716_000003_add_order_field_cache_generation_trigger"));
    assert!(forum_mod.contains("m20260716_000004_add_topic_field_cache_generation_trigger"));
}
