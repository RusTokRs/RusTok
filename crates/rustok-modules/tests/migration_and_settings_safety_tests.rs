use rustok_modules::{
    MigrationPreflightInput, ModuleSettingSpec, SettingsCompatibilityGuard, UpdateMode,
    evaluate_migration_preflight,
};
use std::collections::HashMap;
use uuid::Uuid;

#[test]
fn test_end_to_end_additive_migration_preflight_and_settings_guard() {
    let operation_id = Uuid::new_v4();

    // 1. Evaluate preflight for an additive change
    let preflight_input = MigrationPreflightInput {
        operation_id,
        module_slug: "customer".to_string(),
        source_schema_digest:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        target_schema_digest:
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        migration_plan_digest: "sha256:plan_additive".to_string(),
        is_additive_safe: true,
        migration_reasons: vec![],
        settings_guard_installed: true,
        has_irreversible_external_effects: false,
        requires_cross_revision_data_copy: false,
    };

    let receipt = evaluate_migration_preflight(preflight_input);
    assert_eq!(receipt.mode, UpdateMode::Automatic);
    assert!(receipt.denial_reasons.is_empty());

    // 2. Install Settings Guard during active rollout window
    let mut schema_n = HashMap::new();
    schema_n.insert(
        "max_sessions".to_string(),
        ModuleSettingSpec {
            value_type: "number".to_string(),
            required: true,
            default: Some(serde_json::json!(5)),
            min: Some(1.0),
            max: Some(50.0),
            ..Default::default()
        },
    );

    let mut schema_n1 = schema_n.clone();
    schema_n1.insert(
        "enable_2fa".to_string(),
        ModuleSettingSpec {
            value_type: "boolean".to_string(),
            required: false,
            default: Some(serde_json::json!(false)),
            ..Default::default()
        },
    );

    let mut guard = SettingsCompatibilityGuard::new(
        Some(Uuid::new_v4()),
        "customer".to_string(),
        "sha256:schema_n".to_string(),
        "sha256:schema_n1".to_string(),
        Uuid::new_v4(),
    );

    // Value accepted by N and N+1
    let safe_write = serde_json::json!({ "max_sessions": 10 });
    let normalized = guard.validate_and_normalize_write(&schema_n, &schema_n1, safe_write);
    assert!(normalized.is_ok());
    let value = normalized.unwrap();
    assert_eq!(value["max_sessions"], 10);
    assert_eq!(value["enable_2fa"], false);

    // Value with unknown key to N (breaking backward-compatibility if reverted)
    let breaking_write = serde_json::json!({
        "max_sessions": 10,
        "enable_2fa": true,
        "unknown_extra": "rejected"
    });
    let rejected = guard.validate_and_normalize_write(&schema_n, &schema_n1, breaking_write);
    assert!(rejected.is_err());

    // 3. Close the guard upon rollback window expiration
    guard.close();
    // After closing, candidate is sole authority
    let candidate_write = serde_json::json!({
        "max_sessions": 10,
        "enable_2fa": true
    });
    let after_close = guard.validate_and_normalize_write(&schema_n, &schema_n1, candidate_write);
    assert!(after_close.is_ok());
    assert_eq!(after_close.unwrap()["enable_2fa"], true);
}

#[test]
fn test_destructive_migration_strictly_denies_automatic_mode() {
    let operation_id = Uuid::new_v4();

    let preflight_input = MigrationPreflightInput {
        operation_id,
        module_slug: "billing".to_string(),
        source_schema_digest: "sha256:source".to_string(),
        target_schema_digest: "sha256:target".to_string(),
        migration_plan_digest: "sha256:plan_destructive".to_string(),
        is_additive_safe: false,
        migration_reasons: vec![
            "Destructive operation: Drop table 'legacy_invoices'".to_string(),
            "Non-concurrent index creation on 'accounts'".to_string(),
        ],
        settings_guard_installed: false,
        has_irreversible_external_effects: false,
        requires_cross_revision_data_copy: false,
    };

    let receipt = evaluate_migration_preflight(preflight_input);
    assert_eq!(receipt.mode, UpdateMode::Maintenance);
    assert_eq!(receipt.denial_reasons.len(), 2);
    assert!(receipt.denial_reasons[0].contains("Drop table"));
}

#[test]
fn test_settings_guard_from_observing_checkpoint() {
    use chrono::{Duration, Utc};
    use rustok_modules::{
        ConflictFenceSet, GlobalSecurityEpoch, ModuleTransitionCheckpoint, ModuleTransitionState,
    };

    let operation_id = Uuid::new_v4();
    let tenant_id = Some(Uuid::new_v4());

    let observing_checkpoint = ModuleTransitionCheckpoint {
        operation_id,
        revision: 1,
        module_slug: "customer".to_string(),
        tenant_id,
        predecessor_digest: Some("sha256:source_pred".to_string()),
        candidate_digest: "sha256:cand_001".to_string(),
        state: ModuleTransitionState::Observing {
            timeout_at: Utc::now() + Duration::seconds(300),
        },
        security_epoch: GlobalSecurityEpoch::INITIAL,
        fences: ConflictFenceSet::new(Vec::new()),
        recovery_attempt_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let guard = SettingsCompatibilityGuard::from_observing_checkpoint(
        &observing_checkpoint,
        "sha256:schema_pred".to_string(),
        "sha256:schema_cand".to_string(),
    );
    assert!(guard.is_some());
    let guard = guard.unwrap();
    assert_eq!(guard.rollback_window_id, operation_id);
    assert_eq!(guard.module_slug, "customer");
    assert_eq!(guard.state, rustok_modules::SettingsGuardState::Active);

    // Converged checkpoint does not generate an active observing guard
    let converged_checkpoint = ModuleTransitionCheckpoint {
        state: ModuleTransitionState::Converged {
            finalized_at: Utc::now(),
        },
        ..observing_checkpoint
    };
    let no_guard = SettingsCompatibilityGuard::from_observing_checkpoint(
        &converged_checkpoint,
        "sha256:schema_pred".to_string(),
        "sha256:schema_cand".to_string(),
    );
    assert!(no_guard.is_none());
}

