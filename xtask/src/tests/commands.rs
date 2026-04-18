use super::*;

#[test]
fn build_module_test_plan_includes_main_and_ui_crates() {
    let preview = ModulePublishDryRunPreview {
        slug: "blog".to_string(),
        version: "1.2.3".to_string(),
        crate_name: "rustok-blog".to_string(),
        module_default_locale: "en".to_string(),
        module_name: "Blog".to_string(),
        module_description: "A blog module description long enough.".to_string(),
        ownership: "first_party".to_string(),
        trust_level: "verified".to_string(),
        license: "MIT".to_string(),
        manifest_path: "modules.toml".to_string(),
        package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
        module_entry_type: Some("BlogModule".to_string()),
        marketplace: ModuleMarketplacePreview {
            category: Some("content".to_string()),
            tags: vec!["content".to_string()],
        },
        ui_packages: ModuleUiPackagesPreview {
            admin: Some(ModuleUiPackagePreview {
                crate_name: "rustok-blog-admin".to_string(),
                manifest_path: "crates/rustok-blog/admin/Cargo.toml".to_string(),
            }),
            storefront: Some(ModuleUiPackagePreview {
                crate_name: "rustok-blog-storefront".to_string(),
                manifest_path: "crates/rustok-blog/storefront/Cargo.toml".to_string(),
            }),
        },
    };

    let plan = build_module_test_plan(&preview);
    let commands = plan
        .commands
        .iter()
        .map(|command| command.argv.join(" "))
        .collect::<Vec<_>>();

    assert_eq!(commands.len(), 4);
    assert!(commands[0].contains("module validate blog"));
    assert!(commands[1].contains("cargo check -p rustok-blog"));
    assert!(commands[2].contains("cargo check -p rustok-blog-admin"));
    assert!(commands[3].contains("cargo check -p rustok-blog-storefront"));
}

#[test]
fn build_publish_registry_request_serializes_v2_contract() {
    let preview = ModulePublishDryRunPreview {
        slug: "blog".to_string(),
        version: "1.2.3".to_string(),
        crate_name: "rustok-blog".to_string(),
        module_default_locale: "en".to_string(),
        module_name: "Blog".to_string(),
        module_description: "A blog module description long enough.".to_string(),
        ownership: "first_party".to_string(),
        trust_level: "verified".to_string(),
        license: "MIT".to_string(),
        manifest_path: "modules.toml".to_string(),
        package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
        module_entry_type: Some("BlogModule".to_string()),
        marketplace: ModuleMarketplacePreview {
            category: Some("content".to_string()),
            tags: vec!["content".to_string(), "editorial".to_string()],
        },
        ui_packages: ModuleUiPackagesPreview {
            admin: Some(ModuleUiPackagePreview {
                crate_name: "rustok-blog-admin".to_string(),
                manifest_path: "crates/rustok-blog/admin/Cargo.toml".to_string(),
            }),
            storefront: Some(ModuleUiPackagePreview {
                crate_name: "rustok-blog-storefront".to_string(),
                manifest_path: "crates/rustok-blog/storefront/Cargo.toml".to_string(),
            }),
        },
    };

    let request_body = serde_json::to_value(build_publish_registry_request(&preview))
        .expect("request should serialize");
    assert_eq!(
        request_body["schema_version"],
        REGISTRY_MUTATION_SCHEMA_VERSION
    );
    assert_eq!(request_body["dry_run"], true);
    assert_eq!(request_body["module"]["slug"], "blog");
    assert_eq!(request_body["module"]["version"], "1.2.3");
    assert_eq!(request_body["module"]["crate_name"], "rustok-blog");
    assert_eq!(request_body["module"]["default_locale"], "en");
    assert_eq!(request_body["module"]["name"], "Blog");
    assert_eq!(
        request_body["module"]["description"],
        "A blog module description long enough."
    );
    assert_eq!(request_body["module"]["ownership"], "first_party");
    assert_eq!(request_body["module"]["trust_level"], "verified");
    assert_eq!(request_body["module"]["license"], "MIT");
    assert_eq!(request_body["module"]["entry_type"], "BlogModule");
    assert_eq!(request_body["module"]["marketplace"]["category"], "content");
    assert_eq!(
        request_body["module"]["marketplace"]["tags"],
        serde_json::json!(["content", "editorial"])
    );
    assert_eq!(
        request_body["module"]["ui_packages"]["admin"]["crate_name"],
        "rustok-blog-admin"
    );
    assert_eq!(
        request_body["module"]["ui_packages"]["storefront"]["crate_name"],
        "rustok-blog-storefront"
    );
}

#[test]
fn build_live_publish_registry_request_turns_off_dry_run() {
    let preview = ModulePublishDryRunPreview {
        slug: "blog".to_string(),
        version: "1.2.3".to_string(),
        crate_name: "rustok-blog".to_string(),
        module_default_locale: "en".to_string(),
        module_name: "Blog".to_string(),
        module_description: "A blog module description long enough.".to_string(),
        ownership: "first_party".to_string(),
        trust_level: "verified".to_string(),
        license: "MIT".to_string(),
        manifest_path: "modules.toml".to_string(),
        package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
        module_entry_type: Some("BlogModule".to_string()),
        marketplace: ModuleMarketplacePreview {
            category: Some("content".to_string()),
            tags: vec!["content".to_string(), "editorial".to_string()],
        },
        ui_packages: ModuleUiPackagesPreview {
            admin: Some(ModuleUiPackagePreview {
                crate_name: "rustok-blog-admin".to_string(),
                manifest_path: "crates/rustok-blog/admin/Cargo.toml".to_string(),
            }),
            storefront: Some(ModuleUiPackagePreview {
                crate_name: "rustok-blog-storefront".to_string(),
                manifest_path: "crates/rustok-blog/storefront/Cargo.toml".to_string(),
            }),
        },
    };

    let request_body = serde_json::to_value(build_live_publish_registry_request(&preview))
        .expect("live publish request should serialize");

    assert_eq!(request_body["dry_run"], false);
    assert_eq!(request_body["module"]["slug"], "blog");
}

#[test]
fn module_publish_command_requires_slug() {
    let error = module_publish_command(&[])
        .expect_err("publish command without slug should fail immediately");

    assert!(error
        .to_string()
        .contains("module publish requires a module slug"));
}

#[test]
fn module_publish_command_live_requires_registry_url() {
    let _cwd_guard = WorkspaceRootGuard::enter();
    let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
    let args = vec!["blog".to_string()];

    let error = module_publish_command(&args)
        .expect_err("live publish should require registry url before any network call");

    assert!(error
        .to_string()
        .contains("Live module publish requires --registry-url"));
}

#[test]
fn module_publish_command_live_requires_auth_token() {
    let _cwd_guard = WorkspaceRootGuard::enter();
    let args = vec![
        "blog".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:18080".to_string(),
    ];

    let error = module_publish_command(&args)
        .expect_err("live publish should require auth token before any network call");

    assert!(error
        .to_string()
        .contains("Live module publish requires --auth-token <token>"));
}

#[test]
fn module_publish_command_rejects_unknown_slug() {
    let _cwd_guard = WorkspaceRootGuard::enter();
    let args = vec!["missing-module".to_string(), "--dry-run".to_string()];

    let error = module_publish_command(&args)
        .expect_err("unknown slug should fail before any network call");

    assert!(error
        .to_string()
        .contains("Unknown module slug 'missing-module'"));
}

#[test]
fn module_test_command_requires_slug() {
    let error =
        module_test_command(&[]).expect_err("module test without slug should fail immediately");

    assert!(error
        .to_string()
        .contains("module test requires a module slug"));
}

#[test]
fn build_validation_stage_registry_request_serializes_stage_contract() {
    let preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: "rpr_test".to_string(),
        stage: "compile_smoke".to_string(),
        status: "passed".to_string(),
        detail: Some("External CI recorded the result.".to_string()),
        reason_code: None,
        requeue: false,
    };

    let request_body = serde_json::to_value(build_validation_stage_registry_request(&preview))
        .expect("stage request should serialize");

    assert_eq!(
        request_body
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(REGISTRY_MUTATION_SCHEMA_VERSION as u64)
    );
    assert_eq!(
        request_body
            .get("dry_run")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        request_body
            .get("stage")
            .and_then(serde_json::Value::as_str),
        Some("compile_smoke")
    );
    assert_eq!(
        request_body
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("passed")
    );
    assert_eq!(
        request_body
            .get("requeue")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

#[test]
fn build_validation_stage_registry_request_serializes_requeue_contract() {
    let preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: "rpr_retry".to_string(),
        stage: "targeted_tests".to_string(),
        status: "queued".to_string(),
        detail: Some("Waiting for external rerun.".to_string()),
        reason_code: None,
        requeue: true,
    };

    let request_body = serde_json::to_value(build_validation_stage_registry_request(&preview))
        .expect("stage request should serialize");

    assert_eq!(
        request_body
            .get("stage")
            .and_then(serde_json::Value::as_str),
        Some("targeted_tests")
    );
    assert_eq!(
        request_body
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("queued")
    );
    assert_eq!(
        request_body
            .get("requeue")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        request_body
            .get("detail")
            .and_then(serde_json::Value::as_str),
        Some("Waiting for external rerun.")
    );
    assert_eq!(
        request_body
            .get("dry_run")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn build_live_validation_stage_registry_request_turns_off_dry_run() {
    let preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: "rpr_live".to_string(),
        stage: "security_policy_review".to_string(),
        status: "blocked".to_string(),
        detail: Some("Waiting for manual policy sign-off.".to_string()),
        reason_code: None,
        requeue: false,
    };

    let request_body = serde_json::to_value(build_live_validation_stage_registry_request(&preview))
        .expect("live stage request should serialize");

    assert_eq!(
        request_body
            .get("dry_run")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        request_body
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
}

#[test]
fn build_validation_stage_registry_request_preserves_null_detail() {
    let preview = ModuleValidationStageDryRunPreview {
        action: "validation_stage".to_string(),
        request_id: "rpr_null".to_string(),
        stage: "compile_smoke".to_string(),
        status: "running".to_string(),
        detail: None,
        reason_code: None,
        requeue: false,
    };

    let request_body = serde_json::to_value(build_validation_stage_registry_request(&preview))
        .expect("stage request should serialize");

    assert!(
        request_body.get("detail").is_some(),
        "stage request keeps explicit detail field for API contract stability"
    );
    assert!(
        request_body
            .get("detail")
            .is_some_and(serde_json::Value::is_null),
        "detail should serialize as null when operator omitted it"
    );
}

#[test]
fn module_stage_command_rejects_unknown_status() {
    let args = vec![
        "rpr_test".to_string(),
        "compile_smoke".to_string(),
        "skipped".to_string(),
    ];

    let error = module_stage_command(&args).expect_err("unsupported stage status must fail");

    assert!(error
        .to_string()
        .contains("module stage status 'skipped' is invalid"));
}

#[test]
fn module_stage_command_requires_request_stage_and_status() {
    let args = vec!["rpr_test".to_string(), "compile_smoke".to_string()];

    let error = module_stage_command(&args)
        .expect_err("stage command without full argument set should fail immediately");

    assert!(error
        .to_string()
        .contains("module stage requires a request id, stage key, and status"));
}

#[test]
fn module_stage_command_rejects_requeue_without_queued_status() {
    let args = vec![
        "rpr_test".to_string(),
        "compile_smoke".to_string(),
        "passed".to_string(),
        "--requeue".to_string(),
    ];

    let error =
        module_stage_command(&args).expect_err("requeue should only be accepted for queued status");

    assert!(error
        .to_string()
        .contains("module stage --requeue requires status 'queued'"));
}

#[test]
fn module_stage_command_rejects_empty_request_id() {
    let args = vec![
        "   ".to_string(),
        "compile_smoke".to_string(),
        "queued".to_string(),
    ];

    let error = module_stage_command(&args)
        .expect_err("empty request id should fail before any registry lookup");

    assert!(error
        .to_string()
        .contains("module stage requires a non-empty request id"));
}

#[test]
fn module_stage_command_rejects_empty_stage_key() {
    let args = vec![
        "rpr_test".to_string(),
        "   ".to_string(),
        "queued".to_string(),
    ];

    let error = module_stage_command(&args)
        .expect_err("empty stage key should fail before any registry lookup");

    assert!(error
        .to_string()
        .contains("module stage requires a non-empty stage key"));
}

#[test]
fn module_stage_command_live_requires_registry_url() {
    let args = vec![
        "rpr_test".to_string(),
        "compile_smoke".to_string(),
        "running".to_string(),
    ];

    let error = module_stage_command(&args)
        .expect_err("live stage update should require registry url before any network call");

    assert!(error
        .to_string()
        .contains("Live module stage requires --registry-url"));
}

#[test]
fn module_stage_command_live_requires_auth_token() {
    let args = vec![
        "rpr_test".to_string(),
        "compile_smoke".to_string(),
        "running".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:18080".to_string(),
    ];

    let error = module_stage_command(&args)
        .expect_err("live stage update should require auth token before any network call");

    assert!(error
        .to_string()
        .contains("Live module stage requires --auth-token <token>"));
}

#[test]
fn module_stage_run_command_live_requires_auth_token() {
    let _cwd_guard = WorkspaceRootGuard::enter();
    let args = vec![
        "blog".to_string(),
        "rpr_test".to_string(),
        "compile_smoke".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:18080".to_string(),
    ];

    let error = module_stage_run_command(&args)
        .expect_err("live stage-run should require auth token before any network call");

    assert!(error
        .to_string()
        .contains("Live module stage-run requires --auth-token <token>"));
}

#[test]
fn registry_url_argument_prefers_cli_value_over_env() {
    let _guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", Some("http://env.example"));

    assert_eq!(
        registry_url_argument(&[
            "--registry-url".to_string(),
            "  http://cli.example  ".to_string(),
        ]),
        Some("http://cli.example".to_string())
    );
}

#[test]
fn registry_url_argument_uses_trimmed_env_and_ignores_blank_values() {
    let _guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", Some("  http://env.example  "));
    assert_eq!(
        registry_url_argument(&[]),
        Some("http://env.example".to_string())
    );

    drop(_guard);

    let _guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", Some("   "));
    assert_eq!(registry_url_argument(&[]), None);
}

#[test]
fn runner_token_argument_prefers_cli_value_over_env() {
    let _guard = EnvVarGuard::set(REMOTE_RUNNER_TOKEN_ENV, Some("env-token"));

    assert_eq!(
        runner_token_argument(&["--runner-token".to_string(), "  cli-token  ".to_string(),]),
        Some("cli-token".to_string())
    );
}

#[test]
fn runner_token_argument_uses_trimmed_env_and_ignores_blank_values() {
    let _guard = EnvVarGuard::set(REMOTE_RUNNER_TOKEN_ENV, Some("  env-token  "));
    assert_eq!(runner_token_argument(&[]), Some("env-token".to_string()));

    drop(_guard);

    let _guard = EnvVarGuard::set(REMOTE_RUNNER_TOKEN_ENV, Some("   "));
    assert_eq!(runner_token_argument(&[]), None);
}

#[test]
fn auth_token_argument_trims_and_ignores_blank_values() {
    assert_eq!(
        auth_token_argument(&[
            "--auth-token".to_string(),
            "  bearer-token  ".to_string()
        ]),
        Some("bearer-token".to_string())
    );
    assert_eq!(
        auth_token_argument(&["--auth-token".to_string(), "   ".to_string()]),
        None
    );
    let _guard = EnvVarGuard::set(MODULE_AUTH_TOKEN_ENV, Some("  env-token  "));
    assert_eq!(auth_token_argument(&[]), Some("env-token".to_string()));
}

#[test]
fn supported_remote_runner_stages_include_manual_review_only_when_enabled() {
    assert_eq!(
        supported_remote_runner_stages(false),
        vec!["compile_smoke", "targeted_tests"]
    );
    assert_eq!(
        supported_remote_runner_stages(true),
        vec!["compile_smoke", "targeted_tests", "security_policy_review"]
    );
}

#[test]
fn publish_status_action_available_matches_governance_actions_case_insensitively() {
    let status = RegistryPublishStatusHttpResponse {
        request_id: "rpr_case".to_string(),
        slug: "catalog".to_string(),
        version: "1.0.0".to_string(),
        status: "approved".to_string(),
        accepted: true,
        warnings: Vec::new(),
        errors: Vec::new(),
        follow_up_gates: Vec::new(),
        validation_stages: Vec::new(),
        approval_override_required: false,
        approval_override_reason_codes: Vec::new(),
        governance_actions: vec![RegistryGovernanceActionHttpResponse {
            key: "request_changes".to_string(),
            reason_required: true,
            reason_code_required: true,
            reason_codes: vec!["docs_gap".to_string()],
            destructive: false,
        }],
        next_step: None,
    };

    assert!(publish_status_action_available(&status, "REQUEST_CHANGES"));
    assert!(!publish_status_action_available(&status, "approve"));
}

#[test]
fn module_request_changes_command_live_requires_auth_token() {
    let args = vec![
        "rpr_123".to_string(),
        "--registry-url".to_string(),
        "http://localhost:5150".to_string(),
        "--reason".to_string(),
        "Needs a fresh artifact".to_string(),
        "--reason-code".to_string(),
        "artifact_mismatch".to_string(),
    ];

    let error = module_request_changes_command(&args)
        .expect_err("live request-changes should require auth token before any network call");

    assert!(error
        .to_string()
        .contains("Live module request-changes requires --auth-token <token>"));
}

#[test]
fn module_hold_command_live_requires_reason_code() {
    let args = vec![
        "rpr_123".to_string(),
        "--registry-url".to_string(),
        "http://localhost:5150".to_string(),
        "--auth-token".to_string(),
        "token".to_string(),
        "--reason".to_string(),
        "Incident review".to_string(),
    ];

    let error = module_hold_command(&args)
        .expect_err("live hold should require reason code before any network call");

    assert!(error
        .to_string()
        .contains("Live module hold requires --reason-code"));
}

#[test]
fn module_resume_command_live_requires_reason() {
    let args = vec![
        "rpr_123".to_string(),
        "--registry-url".to_string(),
        "http://localhost:5150".to_string(),
        "--auth-token".to_string(),
        "token".to_string(),
        "--reason-code".to_string(),
        "review_complete".to_string(),
    ];

    let error = module_resume_command(&args)
        .expect_err("live resume should require reason before any network call");

    assert!(error
        .to_string()
        .contains("Live module resume requires --reason <text>"));
}

#[test]
fn positive_u64_argument_rejects_zero_values() {
    let error = positive_u64_argument(
        &["--poll-interval-ms".to_string(), "0".to_string()],
        "--poll-interval-ms",
        "module runner",
    )
    .expect_err("zero interval must fail");

    assert!(error
        .to_string()
        .contains("module runner flag --poll-interval-ms expects a positive integer"));
}

#[test]
fn module_runner_command_requires_runner_id() {
    let error =
        module_runner_command(&[]).expect_err("runner command without id should fail early");

    assert!(error
        .to_string()
        .contains("module runner requires a non-empty runner id"));
}

#[test]
fn reason_argument_trims_and_ignores_blank_values() {
    assert_eq!(
        reason_argument(&["--reason".to_string(), "  ownership move  ".to_string()]),
        Some("ownership move".to_string())
    );
    assert_eq!(
        reason_argument(&["--reason".to_string(), "   ".to_string()]),
        None
    );
}

#[test]
fn detail_argument_trims_and_ignores_blank_values() {
    assert_eq!(
        detail_argument(&["--detail".to_string(), "  external runner  ".to_string()]),
        Some("external runner".to_string())
    );
    assert_eq!(
        detail_argument(&["--detail".to_string(), "   ".to_string()]),
        None
    );
}

#[test]
fn reason_code_argument_trims_and_normalizes_values() {
    assert_eq!(
        reason_code_argument(&[
            "--reason-code".to_string(),
            "  Critical_Regression  ".to_string(),
        ])
        .expect("supported reason code should parse"),
        Some("critical_regression".to_string())
    );
}

#[test]
fn reason_code_argument_rejects_unknown_values() {
    let error = reason_code_argument(&["--reason-code".to_string(), "surprise".to_string()])
        .expect_err("unknown yank reason code must fail");

    assert!(error
        .to_string()
        .contains("--reason-code 'surprise' is invalid"));
}

#[test]
fn module_owner_transfer_command_rejects_empty_new_owner_user_id() {
    let args = vec!["blog".to_string(), "   ".to_string()];

    let error = module_owner_transfer_command(&args)
        .expect_err("empty new owner user id must fail before manifest lookup");

    assert!(error
        .to_string()
        .contains("module owner-transfer requires a non-empty new owner user id"));
}

#[test]
fn module_owner_transfer_command_requires_slug_and_new_owner_user_id() {
    let args = vec!["blog".to_string()];

    let error = module_owner_transfer_command(&args)
        .expect_err("owner-transfer command without new owner user id should fail immediately");

    assert!(error
        .to_string()
        .contains("module owner-transfer requires a module slug and new owner user id"));
}

#[test]
fn module_owner_transfer_command_live_requires_reason() {
    let _guard = WorkspaceRootGuard::enter();
    let args = vec![
        "blog".to_string(),
        "11111111-1111-1111-1111-111111111111".to_string(),
        "--auth-token".to_string(),
        "token".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:5150".to_string(),
    ];

    let error = module_owner_transfer_command(&args)
        .expect_err("live owner-transfer should require reason before any network call");

    assert!(error
        .to_string()
        .contains("Live module owner-transfer requires --reason <text>"));
}

#[test]
fn module_owner_transfer_command_live_requires_registry_url() {
    let _cwd_guard = WorkspaceRootGuard::enter();
    let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
    let args = vec![
        "blog".to_string(),
        "11111111-1111-1111-1111-111111111111".to_string(),
        "--auth-token".to_string(),
        "token".to_string(),
        "--reason".to_string(),
        "ownership move".to_string(),
    ];

    let error = module_owner_transfer_command(&args)
        .expect_err("live owner-transfer should require registry url before any network call");

    assert!(error
        .to_string()
        .contains("Live module owner-transfer requires --registry-url"));
}

#[test]
fn module_owner_transfer_command_live_requires_auth_token() {
    let _guard = WorkspaceRootGuard::enter();
    let args = vec![
        "blog".to_string(),
        "11111111-1111-1111-1111-111111111111".to_string(),
        "--reason".to_string(),
        "ownership move".to_string(),
        "--reason-code".to_string(),
        "maintenance_handoff".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:5150".to_string(),
    ];

    let error = module_owner_transfer_command(&args)
        .expect_err("live owner-transfer should require auth token before any network call");

    assert!(error
        .to_string()
        .contains("Live module owner-transfer requires --auth-token <token>"));
}

#[test]
fn module_yank_command_live_requires_reason() {
    let _guard = WorkspaceRootGuard::enter();
    let args = vec![
        "blog".to_string(),
        "1.2.3".to_string(),
        "--auth-token".to_string(),
        "token".to_string(),
        "--reason-code".to_string(),
        "rollback".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:5150".to_string(),
    ];

    let error = module_yank_command(&args)
        .expect_err("live yank should require reason before any network call");

    assert!(error
        .to_string()
        .contains("Live module yank requires --reason <text>"));
}

#[test]
fn module_yank_command_requires_slug_and_version() {
    let args = vec!["blog".to_string()];

    let error = module_yank_command(&args)
        .expect_err("yank command without version should fail immediately");

    assert!(error
        .to_string()
        .contains("module yank requires a module slug and version"));
}

#[test]
fn module_yank_command_live_requires_registry_url() {
    let _cwd_guard = WorkspaceRootGuard::enter();
    let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
    let args = vec![
        "blog".to_string(),
        "1.2.3".to_string(),
        "--auth-token".to_string(),
        "token".to_string(),
        "--reason".to_string(),
        "policy rollback".to_string(),
        "--reason-code".to_string(),
        "rollback".to_string(),
    ];

    let error = module_yank_command(&args)
        .expect_err("live yank should require registry url before any network call");

    assert!(error
        .to_string()
        .contains("Live module yank requires --registry-url"));
}

#[test]
fn module_yank_command_live_requires_auth_token() {
    let _guard = WorkspaceRootGuard::enter();
    let args = vec![
        "blog".to_string(),
        "1.2.3".to_string(),
        "--reason".to_string(),
        "critical regression in production".to_string(),
        "--reason-code".to_string(),
        "rollback".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:5150".to_string(),
    ];

    let error = module_yank_command(&args)
        .expect_err("live yank should require auth token before any network call");

    assert!(error
        .to_string()
        .contains("Live module yank requires --auth-token <token>"));
}

#[test]
fn module_yank_command_live_requires_reason_code() {
    let _cwd_guard = WorkspaceRootGuard::enter();
    let _env_guard = EnvVarGuard::set("RUSTOK_MODULE_REGISTRY_URL", None);
    let args = vec![
        "blog".to_string(),
        "1.2.3".to_string(),
        "--auth-token".to_string(),
        "token".to_string(),
        "--reason".to_string(),
        "critical regression in production".to_string(),
        "--registry-url".to_string(),
        "http://127.0.0.1:5150".to_string(),
    ];

    let error = module_yank_command(&args)
        .expect_err("live yank should require reason code before any network call");

    assert!(error
        .to_string()
        .contains("Live module yank requires --reason-code"));
    assert!(error
        .to_string()
        .contains(&REGISTRY_YANK_REASON_CODES.join("|")));
}

#[test]
fn module_yank_command_rejects_invalid_semver() {
    let args = vec!["blog".to_string(), "not-a-version".to_string()];

    let error = module_yank_command(&args)
        .expect_err("invalid semver should fail before any manifest lookup");

    assert!(error.to_string().contains("is not valid semver"));
}

#[test]
fn build_yank_registry_request_serializes_v2_contract() {
    let preview = ModuleYankDryRunPreview {
        action: "yank".to_string(),
        slug: "blog".to_string(),
        version: "1.2.3".to_string(),
        crate_name: "rustok-blog".to_string(),
        current_local_version: "1.2.3".to_string(),
        matches_local_version: true,
        package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
    };

    let request_body = serde_json::to_value(build_yank_registry_request(
        &preview,
        Some("Accidental publish".to_string()),
        Some("rollback".to_string()),
    ))
    .expect("request should serialize");
    assert_eq!(
        request_body["schema_version"],
        REGISTRY_MUTATION_SCHEMA_VERSION
    );
    assert_eq!(request_body["dry_run"], true);
    assert_eq!(request_body["slug"], "blog");
    assert_eq!(request_body["version"], "1.2.3");
    assert_eq!(request_body["reason"], "Accidental publish");
    assert_eq!(request_body["reason_code"], "rollback");
}

#[test]
fn build_live_yank_registry_request_turns_off_dry_run_and_keeps_reason_code() {
    let preview = ModuleYankDryRunPreview {
        action: "yank".to_string(),
        slug: "blog".to_string(),
        version: "1.2.3".to_string(),
        crate_name: "rustok-blog".to_string(),
        current_local_version: "1.2.3".to_string(),
        matches_local_version: true,
        package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
    };

    let request_body = serde_json::to_value(build_live_yank_registry_request(
        &preview,
        Some("Security takedown".to_string()),
        Some("security".to_string()),
    ))
    .expect("live yank request should serialize");

    assert_eq!(request_body["dry_run"], false);
    assert_eq!(request_body["reason_code"], "security");
}

#[test]
fn build_owner_transfer_registry_request_serializes_v2_contract() {
    let preview = ModuleOwnerTransferDryRunPreview {
        action: "owner_transfer".to_string(),
        slug: "blog".to_string(),
        crate_name: "rustok-blog".to_string(),
        current_local_version: "1.2.3".to_string(),
        package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
        new_owner_user_id: "11111111-1111-1111-1111-111111111111".to_string(),
        reason: Some("Ownership transferred to the forum publisher".to_string()),
        reason_code: Some("maintenance_handoff".to_string()),
    };

    let request_body = serde_json::to_value(build_owner_transfer_registry_request(
        &preview,
        preview.reason.clone(),
        preview.reason_code.clone(),
    ))
    .expect("request should serialize");
    assert_eq!(
        request_body["schema_version"],
        REGISTRY_MUTATION_SCHEMA_VERSION
    );
    assert_eq!(request_body["dry_run"], true);
    assert_eq!(request_body["slug"], "blog");
    assert_eq!(
        request_body["new_owner_user_id"],
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(
        request_body["reason"],
        "Ownership transferred to the forum publisher"
    );
    assert_eq!(request_body["reason_code"], "maintenance_handoff");
}

#[test]
fn build_live_owner_transfer_registry_request_turns_off_dry_run() {
    let preview = ModuleOwnerTransferDryRunPreview {
        action: "owner_transfer".to_string(),
        slug: "blog".to_string(),
        crate_name: "rustok-blog".to_string(),
        current_local_version: "1.2.3".to_string(),
        package_manifest_path: "crates/rustok-blog/rustok-module.toml".to_string(),
        new_owner_user_id: "22222222-2222-2222-2222-222222222222".to_string(),
        reason: Some("Transfer to the comments publisher".to_string()),
        reason_code: Some("publisher_rotation".to_string()),
    };

    let request_body = serde_json::to_value(build_live_owner_transfer_registry_request(
        &preview,
        preview.reason.clone(),
        preview.reason_code.clone(),
    ))
    .expect("live owner transfer request should serialize");

    assert_eq!(request_body["dry_run"], false);
    assert_eq!(
        request_body["new_owner_user_id"],
        "22222222-2222-2222-2222-222222222222"
    );
    assert_eq!(request_body["reason_code"], "publisher_rotation");
}

#[test]
fn registry_endpoint_uses_loopback_for_local_urls() {
    assert!(registry_endpoint_uses_loopback(
        "http://127.0.0.1:5150/v2/catalog/publish"
    ));
    assert!(registry_endpoint_uses_loopback(
        "http://localhost:5150/v2/catalog/yank"
    ));
    assert!(!registry_endpoint_uses_loopback(
        "https://modules.rustok.dev/v2/catalog/publish"
    ));
    assert!(registry_endpoint_uses_loopback(
        "http://[::1]:5150/v2/catalog/publish"
    ));
    assert!(!registry_endpoint_uses_loopback(
        "http://0.0.0.0:5150/v2/catalog/publish"
    ));
}

#[test]
fn auto_approve_argument_detects_flag() {
    assert!(auto_approve_argument(&[
        "--registry-url".to_string(),
        "http://127.0.0.1:5150".to_string(),
        "--auto-approve".to_string(),
    ]));
    assert!(!auto_approve_argument(&[
        "--registry-url".to_string(),
        "http://127.0.0.1:5150".to_string(),
    ]));
}
