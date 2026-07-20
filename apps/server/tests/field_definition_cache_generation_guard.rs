#[test]
fn field_definition_cache_uses_one_transactional_generation_and_fail_closed_recovery() {
    let helper = include_str!("../../../crates/flex/src/cache_generation.rs");
    let wrapper = include_str!("../src/services/field_definition_cache.rs");
    let base = include_str!("../src/services/field_definition_cache_base.rs");
    let reconciler = include_str!("../src/services/field_definition_cache_reconciliation.rs");
    let evidence = include_str!("../src/services/field_definition_cache_reconciliation_tests.rs");
    let guardrails = include_str!("../src/services/runtime_guardrails.rs");

    for required in [
        "pub const FIELD_DEFINITION_CACHE_GENERATION_TABLE",
        "flex_field_definition_cache_generation",
        "generation = generation + 1",
        "AFTER INSERT OR UPDATE OR DELETE",
        "FOR EACH STATEMENT",
        "for operation in [\"insert\", \"update\", \"delete\"]",
        "validate_identifier(table_name)?",
        "validate_identifier(trigger_name)?",
        "sqlite_all_owner_mutations_are_transactional_and_replay_safe",
        "UPDATE {table} SET position = position + 1",
        "UPDATE {table} SET is_active = 0",
        "rolled-back owner mutation should execute",
        ".rollback()",
        "assert_eq!(read_generation(&db).await, 16);",
    ] {
        assert!(
            helper.contains(required),
            "Flex generation helper must retain {required}"
        );
    }

    assert!(wrapper.contains("#[path = \"field_definition_cache_base.rs\"]"));
    assert!(wrapper.contains("let cache = base::field_definition_cache_from_context(ctx, bus);"));
    assert!(
        wrapper
            .contains("start_field_definition_cache_generation_reconciliation(ctx, cache.clone())")
    );
    assert!(base.contains("pub fn field_definition_cache_from_context"));
    assert!(base.contains("cache.invalidate_all();"));

    let durable_read = reconciler
        .find("let mut applied = match read_field_definition_cache_generation(&db).await")
        .expect("startup must seed from durable generation");
    let startup_clear = reconciler[durable_read..]
        .find("// Seed from durable state before trusting any process-local cache contents.\n    cache.invalidate_all();")
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

    let regression = reconciler
        .find("if current < applied")
        .expect("generation regression must be checked");
    let regression_unhealthy = reconciler[regression..]
        .find("state.healthy.store(false, Ordering::Release)")
        .map(|offset| regression + offset)
        .expect("generation regression must fail closed");
    let regression_clear = reconciler[regression_unhealthy..]
        .find("cache.invalidate_all();")
        .map(|offset| regression_unhealthy + offset)
        .expect("generation regression must clear cached schemas");
    let regression_error = reconciler[regression_clear..]
        .find("field-definition cache generation regressed")
        .map(|offset| regression_clear + offset)
        .expect("generation regression must terminate the worker iteration");
    assert!(regression_unhealthy < regression_clear);
    assert!(regression_clear < regression_error);

    let advance = reconciler
        .find("if current == applied")
        .expect("generation equality must be handled");
    let advance_unhealthy = reconciler[advance..]
        .find("state.healthy.store(false, Ordering::Release)")
        .map(|offset| advance + offset)
        .expect("generation advance must fail closed while clearing");
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

    for required in [
        "pub fn is_running(&self) -> bool",
        "pub fn is_ready(&self) -> bool",
        "self.is_running() && self.state.healthy.load(Ordering::Acquire)",
        "start_field_definition_cache_generation_reconciliation_with_timing",
        "struct AbortOnDropFieldDefinitionCacheGenerationTask",
        "self.task.abort();",
        ".catch_unwind()",
        "#[path = \"field_definition_cache_reconciliation_tests.rs\"]",
    ] {
        assert!(
            reconciler.contains(required),
            "Flex reconciler must retain {required}"
        );
    }
    assert!(!reconciler.contains("invalidate_all().await"));

    for required in [
        "field_definition_cache_generation_recovers_two_replicas_across_faults",
        "before-startup-a",
        "set_generation(&db, 6).await;",
        "drop_generation_table(&db).await;",
        "restore_generation_table(&db, 7).await;",
        "set_generation(&db, 3).await;",
        "set_generation(&db, 8).await;",
        "wait_for_state(&handle_a, false, Some(7)).await;",
        "wait_for_state(&handle_b, true, Some(8)).await;",
    ] {
        assert!(
            evidence.contains(required),
            "Flex two-replica evidence must retain {required}"
        );
    }

    assert!(
        guardrails
            .contains("ctx.shared_get::<FieldDefinitionCacheGenerationReconciliationHandle>()")
    );
    assert!(guardrails.contains("Flex field-definition durable cache reconciliation"));
    assert!(guardrails.contains(".map(|handle| handle.is_ready())"));
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

#[test]
fn permanent_gate_retains_postgres_and_sqlite_flex_generation_evidence() {
    let postgres = include_str!("../../../crates/flex/tests/postgres_cache_generation.rs");
    let workflow = include_str!("../../../.github/workflows/cache-hardening.yml");

    for required in [
        "postgres_flex_generation_is_transactional_concurrent_and_replay_safe",
        "ConnectOptions::new(url.to_string())",
        "let replica = connect_postgres(url.as_str()).await;",
        "let mutation_a = tokio::spawn",
        "let mutation_b = tokio::spawn",
        "assert_eq!(read_generation(&replica).await, 14);",
        "install_generation_contract(&writer).await;",
        "assert_eq!(read_generation(&replica).await, 0);",
    ] {
        assert!(
            postgres.contains(required),
            "PostgreSQL Flex evidence must retain {required}"
        );
    }

    for required in [
        "crates/flex/tests/**",
        "apps/server/src/services/field_definition_cache_reconciliation*.rs",
        "cargo test -p flex cache_generation --lib",
        "RUSTOK_FLEX_TEST_POSTGRES_URL",
        "cargo test -p flex --test postgres_cache_generation -- --ignored --nocapture --test-threads=1",
        "cargo test -p rustok-server field_definition_cache_generation --lib",
    ] {
        assert!(
            workflow.contains(required),
            "cache workflow must retain Flex evidence command: {required}"
        );
    }
}
