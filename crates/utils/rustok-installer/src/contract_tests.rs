#[cfg(test)]
mod tests {
    use crate::plan::{
        AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallDistributionBinding,
        InstallEnvironment, InstallPlan, InstallProfile, InstallRole, InstallSurface,
        InstallTopology, InstallTopologyMode, ModuleSelection, SeedProfile, TenantBootstrap,
    };
    use crate::preflight::{
        PreflightSeverity, evaluate_preflight, evaluate_preflight_with_deployment,
    };
    use crate::receipt::{InstallReceipt, ReceiptOutcome, checksum_json};
    use crate::secrets::{
        SecretMode, SecretRef, SecretValue, redact_install_plan, redact_secret,
        resolve_local_secret_value,
    };
    use crate::state::{InstallState, InstallStep, StateTransitionError};
    use crate::{InstancePlacement, SeedContentOutcome};

    #[test]
    fn install_state_happy_path_lifecycle() {
        let mut state = InstallState::Draft;
        let chain = [
            InstallState::PreflightPassed,
            InstallState::ConfigPrepared,
            InstallState::DatabaseReady,
            InstallState::SchemaApplied,
            InstallState::SeedApplied,
            InstallState::AdminProvisioned,
            InstallState::Deploying,
            InstallState::Verified,
            InstallState::Completed,
        ];
        for next in chain {
            state = state.transition_to(next).expect("valid transition");
        }
        assert_eq!(state, InstallState::Completed);
    }

    #[test]
    fn install_state_cleanup_and_recovery_invariants() {
        // Pre-schema states can transition to FreshInstallCleaned
        for pre_schema in [
            InstallState::Draft,
            InstallState::PreflightPassed,
            InstallState::ConfigPrepared,
            InstallState::DatabaseReady,
        ] {
            assert!(
                pre_schema.can_transition_to(InstallState::FreshInstallCleaned),
                "{pre_schema:?} should allow transition to FreshInstallCleaned"
            );
        }

        // Post-schema states MUST NOT transition to FreshInstallCleaned; they require RecoveryRequired
        for post_schema in [
            InstallState::SchemaApplied,
            InstallState::SeedApplied,
            InstallState::AdminProvisioned,
            InstallState::Deploying,
            InstallState::Verified,
        ] {
            assert!(
                !post_schema.can_transition_to(InstallState::FreshInstallCleaned),
                "{post_schema:?} must not allow transition to FreshInstallCleaned"
            );
            assert!(
                post_schema.can_transition_to(InstallState::RecoveryRequired),
                "{post_schema:?} must allow transition to RecoveryRequired"
            );
        }

        // Any state can transition to Failed
        assert!(InstallState::Draft.can_transition_to(InstallState::Failed));
        assert!(InstallState::Deploying.can_transition_to(InstallState::Failed));

        // Invalid transitions return StateTransitionError
        let err = InstallState::Draft
            .transition_to(InstallState::Completed)
            .unwrap_err();
        assert_eq!(
            err,
            StateTransitionError {
                from: InstallState::Draft,
                to: InstallState::Completed,
            }
        );
    }

    #[test]
    fn install_step_serialization_and_variants() {
        let steps = [
            (InstallStep::Preflight, "\"preflight\""),
            (InstallStep::Config, "\"config\""),
            (InstallStep::Database, "\"database\""),
            (InstallStep::Migrate, "\"migrate\""),
            (InstallStep::Seed, "\"seed\""),
            (InstallStep::Admin, "\"admin\""),
            (InstallStep::Deploy, "\"deploy\""),
            (InstallStep::Verify, "\"verify\""),
            (InstallStep::Finalize, "\"finalize\""),
            (InstallStep::Rollback, "\"rollback\""),
        ];
        for (step, expected) in steps {
            let serialized = serde_json::to_string(&step).unwrap();
            assert_eq!(serialized, expected);
            let deserialized: InstallStep = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, step);
        }
    }

    #[test]
    fn secret_mode_parsing_and_aliases() {
        assert_eq!(SecretMode::parse_cli_value("env").unwrap(), SecretMode::Env);
        assert_eq!(
            SecretMode::parse_cli_value("dotenv-file").unwrap(),
            SecretMode::DotenvFile
        );
        assert_eq!(
            SecretMode::parse_cli_value("dotenv_file").unwrap(),
            SecretMode::DotenvFile
        );
        assert_eq!(
            SecretMode::parse_cli_value("mounted-file").unwrap(),
            SecretMode::MountedFile
        );
        assert_eq!(
            SecretMode::parse_cli_value("mounted_file").unwrap(),
            SecretMode::MountedFile
        );
        assert_eq!(
            SecretMode::parse_cli_value("external-secret").unwrap(),
            SecretMode::ExternalSecret
        );
        assert_eq!(
            SecretMode::parse_cli_value("external_secret").unwrap(),
            SecretMode::ExternalSecret
        );

        assert!(SecretMode::parse_cli_value("invalid_mode").is_err());
    }

    #[test]
    fn secret_ref_parsing_validation() {
        let s_ref = SecretRef::parse_cli_value("vault:secret/rustok/db").unwrap();
        assert_eq!(s_ref.backend, "vault");
        assert_eq!(s_ref.key, "secret/rustok/db");

        let s_ref_trimmed = SecretRef::parse_cli_value("  k8s : token  ").unwrap();
        assert_eq!(s_ref_trimmed.backend, "k8s");
        assert_eq!(s_ref_trimmed.key, "token");

        assert!(SecretRef::parse_cli_value("vault").is_err());
        assert!(SecretRef::parse_cli_value(":key").is_err());
        assert!(SecretRef::parse_cli_value("vault:").is_err());
    }

    #[test]
    fn secret_value_and_redaction_guarantees() {
        let plain = SecretValue::Plaintext {
            value: "super_secret_value_123".to_string(),
        };
        assert_eq!(plain.as_plaintext(), Some("super_secret_value_123"));

        let redacted_plain = redact_secret(&plain);
        assert_eq!(redacted_plain["kind"], "redacted");
        assert_eq!(redacted_plain["value"], "***");
        assert!(
            !redacted_plain
                .to_string()
                .contains("super_secret_value_123")
        );

        let reference = SecretValue::Reference {
            reference: SecretRef {
                backend: "vault".to_string(),
                key: "secret/db".to_string(),
            },
        };
        assert_eq!(reference.as_plaintext(), None);

        let redacted_ref = redact_secret(&reference);
        assert_eq!(redacted_ref["kind"], "reference");
        assert_eq!(redacted_ref["backend"], "vault");
        assert_eq!(redacted_ref["key"], "secret/db");

        // Plaintext resolves immediately
        assert_eq!(
            resolve_local_secret_value(&plain, "test").unwrap(),
            "super_secret_value_123"
        );

        // External reference returns typed error without crashing
        let err = resolve_local_secret_value(&reference, "test").unwrap_err();
        assert!(err.to_string().contains("external secret resolver"));
    }

    #[test]
    fn database_engine_parsing_and_variants() {
        assert_eq!(
            DatabaseEngine::parse_cli_value("postgres").unwrap(),
            DatabaseEngine::Postgres
        );
        assert_eq!(
            DatabaseEngine::parse_cli_value("postgresql").unwrap(),
            DatabaseEngine::Postgres
        );
        assert_eq!(
            DatabaseEngine::parse_cli_value("sqlite").unwrap(),
            DatabaseEngine::Sqlite
        );
        assert!(DatabaseEngine::parse_cli_value("mysql").is_err());
    }

    #[test]
    fn install_environment_parsing_and_contract() {
        assert_eq!(
            InstallEnvironment::parse_cli_value("local").unwrap(),
            InstallEnvironment::Local
        );
        assert_eq!(
            InstallEnvironment::parse_cli_value("demo").unwrap(),
            InstallEnvironment::Demo
        );
        assert_eq!(
            InstallEnvironment::parse_cli_value("test").unwrap(),
            InstallEnvironment::Test
        );
        assert_eq!(
            InstallEnvironment::parse_cli_value("production").unwrap(),
            InstallEnvironment::Production
        );
        assert_eq!(
            InstallEnvironment::parse_cli_value("prod").unwrap(),
            InstallEnvironment::Production
        );
        assert!(InstallEnvironment::parse_cli_value("stage").is_err());

        assert!(InstallEnvironment::Production.is_production());
        assert!(!InstallEnvironment::Local.is_production());
        assert!(!InstallEnvironment::Demo.is_production());
        assert!(!InstallEnvironment::Test.is_production());

        assert_eq!(InstallEnvironment::Production.as_str(), "production");
        assert_eq!(InstallEnvironment::Local.as_str(), "local");
    }

    #[test]
    fn install_profile_parsing_and_aliases() {
        assert_eq!(
            InstallProfile::parse_cli_value("dev_local").unwrap(),
            InstallProfile::DevLocal
        );
        assert_eq!(
            InstallProfile::parse_cli_value("dev-local").unwrap(),
            InstallProfile::DevLocal
        );
        assert_eq!(
            InstallProfile::parse_cli_value("dev").unwrap(),
            InstallProfile::DevLocal
        );
        assert_eq!(
            InstallProfile::parse_cli_value("monolith").unwrap(),
            InstallProfile::Monolith
        );
        assert_eq!(
            InstallProfile::parse_cli_value("hybrid_admin").unwrap(),
            InstallProfile::HybridAdmin
        );
        assert_eq!(
            InstallProfile::parse_cli_value("hybrid-admin").unwrap(),
            InstallProfile::HybridAdmin
        );
        assert_eq!(
            InstallProfile::parse_cli_value("headless_leptos").unwrap(),
            InstallProfile::HeadlessLeptos
        );
        assert_eq!(
            InstallProfile::parse_cli_value("headless-leptos").unwrap(),
            InstallProfile::HeadlessLeptos
        );
        assert!(InstallProfile::parse_cli_value("unknown").is_err());
    }

    #[test]
    fn seed_profile_module_policy() {
        assert_eq!(
            SeedProfile::parse_cli_value("none").unwrap(),
            SeedProfile::None
        );
        assert_eq!(
            SeedProfile::parse_cli_value("minimal").unwrap(),
            SeedProfile::Minimal
        );
        assert_eq!(
            SeedProfile::parse_cli_value("dev").unwrap(),
            SeedProfile::Dev
        );

        assert_eq!(
            SeedProfile::Dev.default_enabled_modules(),
            vec![
                "content".to_string(),
                "commerce".to_string(),
                "pages".to_string(),
                "blog".to_string(),
                "forum".to_string(),
                "index".to_string(),
            ]
        );
        assert!(SeedProfile::Minimal.default_enabled_modules().is_empty());
        assert!(SeedProfile::None.default_enabled_modules().is_empty());
    }

    #[test]
    fn module_selection_defaults() {
        let selection = ModuleSelection::default();
        assert!(selection.enable.is_empty());
        assert!(selection.disable.is_empty());
    }

    #[test]
    fn role_and_surface_compatibility() {
        let all_surfaces = [
            InstallSurface::Api,
            InstallSurface::Admin,
            InstallSurface::Storefront,
            InstallSurface::Worker,
            InstallSurface::Registry,
        ];

        // Monolith supports every surface
        for surface in all_surfaces {
            assert!(InstallRole::Monolith.supports_surface(surface));
        }

        // Distributed roles support only their dedicated surface
        assert!(InstallRole::Api.supports_surface(InstallSurface::Api));
        assert!(!InstallRole::Api.supports_surface(InstallSurface::Admin));

        assert!(InstallRole::AdminSsr.supports_surface(InstallSurface::Admin));
        assert!(!InstallRole::AdminSsr.supports_surface(InstallSurface::Api));

        assert!(InstallRole::StorefrontSsr.supports_surface(InstallSurface::Storefront));
        assert!(!InstallRole::StorefrontSsr.supports_surface(InstallSurface::Worker));

        assert!(InstallRole::Worker.supports_surface(InstallSurface::Worker));
        assert!(!InstallRole::Worker.supports_surface(InstallSurface::Registry));

        assert!(InstallRole::Registry.supports_surface(InstallSurface::Registry));
        assert!(!InstallRole::Registry.supports_surface(InstallSurface::Api));

        assert_eq!(InstallRole::Monolith.as_str(), "monolith");
        assert_eq!(InstallRole::Api.as_str(), "api");
        assert_eq!(InstallRole::AdminSsr.as_str(), "admin_ssr");
        assert_eq!(InstallRole::StorefrontSsr.as_str(), "storefront_ssr");
        assert_eq!(InstallRole::Worker.as_str(), "worker");
        assert_eq!(InstallRole::Registry.as_str(), "registry");
    }

    #[test]
    fn checksum_json_determinism_and_receipt_validation() {
        #[derive(serde::Serialize)]
        struct Payload {
            key: &'static str,
            count: u32,
        }

        let p1 = Payload {
            key: "abc",
            count: 42,
        };
        let p2 = Payload {
            key: "abc",
            count: 42,
        };
        let p3 = Payload {
            key: "abc",
            count: 43,
        };

        let cs1 = checksum_json(&p1).unwrap();
        let cs2 = checksum_json(&p2).unwrap();
        let cs3 = checksum_json(&p3).unwrap();

        assert_eq!(cs1, cs2);
        assert_ne!(cs1, cs3);

        let receipt = InstallReceipt::success(
            "session-123",
            InstallStep::Database,
            &p1,
            serde_json::json!({ "ready": true }),
        )
        .unwrap();

        assert_eq!(receipt.session_id, "session-123");
        assert_eq!(receipt.step, InstallStep::Database);
        assert_eq!(receipt.outcome, ReceiptOutcome::Success);
        assert_eq!(receipt.input_checksum, cs1);

        assert!(receipt.can_skip(InstallStep::Database, &p1).unwrap());
        assert!(!receipt.can_skip(InstallStep::Database, &p3).unwrap());
        assert!(!receipt.can_skip(InstallStep::Migrate, &p1).unwrap());
    }

    #[test]
    fn seed_content_outcome_statistics() {
        let empty = SeedContentOutcome {
            module_slug: "blog".to_string(),
            imported_count: 0,
            failed_count: 0,
            errors: Vec::new(),
        };
        assert!(empty.is_success());
        assert_eq!(empty.total_count(), 0);
        assert_eq!(empty.success_rate(), 0.0);

        let success = SeedContentOutcome::success("content".to_string(), 25);
        assert!(success.is_success());
        assert_eq!(success.total_count(), 25);
        assert_eq!(success.success_rate(), 100.0);

        let partial = SeedContentOutcome {
            module_slug: "commerce".to_string(),
            imported_count: 30,
            failed_count: 10,
            errors: vec!["item error".to_string()],
        };
        assert!(!partial.is_success());
        assert_eq!(partial.total_count(), 40);
        assert_eq!(partial.success_rate(), 75.0);
    }

    #[test]
    fn instance_placement_validation_bounds() {
        let valid = InstancePlacement::new("./instance_root");
        assert!(valid.validate().is_ok());

        let empty = InstancePlacement::new("   ");
        assert!(empty.validate().is_err());

        let nul_char = InstancePlacement::new("invalid\0root");
        assert!(nul_char.validate().is_err());
    }

    #[test]
    fn preflight_policy_enforcement_matrix() {
        let valid_distribution = InstallDistributionBinding {
            preparation_id: uuid::Uuid::from_u128(1),
            distribution_release_id: uuid::Uuid::from_u128(2),
            bundle_reference: format!("registry.example/rustok/base@sha256:{}", "b".repeat(64)),
            bundle_root_digest: format!("sha256:{}", "b".repeat(64)),
            role_set_digest: format!("sha256:{}", "c".repeat(64)),
            #[cfg(feature = "host-runtime")]
            roles: vec![rustok_modules::ModuleStaticDistributionRoleArtifact {
                role: rustok_modules::ModuleStaticDistributionRole::Monolith,
                artifact_digest: format!("sha256:{}", "d".repeat(64)),
            }],
            #[cfg(not(feature = "host-runtime"))]
            roles: vec![],
            bootstrap_receipt: None,
        };

        let plan = InstallPlan {
            placement: InstancePlacement::new("."),
            environment: InstallEnvironment::Production,
            profile: InstallProfile::Monolith,
            database: DatabaseConfig {
                engine: DatabaseEngine::Sqlite, // VIOLATION 1: SQLite in production
                url: SecretValue::Plaintext {
                    value: "sqlite://test.db".to_string(), // VIOLATION 2: Plaintext secret in production
                },
                create_if_missing: false,
            },
            tenant: TenantBootstrap {
                slug: "demo".to_string(),
                name: "Demo".to_string(),
            },
            admin: AdminBootstrap {
                email: "admin@example.com".to_string(),
                password: SecretValue::Plaintext {
                    value: "admin12345".to_string(), // VIOLATION 3: Sample secret in production
                },
            },
            modules: ModuleSelection {
                enable: vec![],
                disable: vec!["commerce".to_string()], // WARNING: Schema disable not supported
            },
            topology: InstallTopology::for_mode(InstallTopologyMode::Monolith)
                .bind_composition("test".to_string(), "a".repeat(64))
                .bind_distribution(valid_distribution),
            seed_profile: SeedProfile::Dev, // VIOLATION 4: Dev seed in production
            secrets_mode: SecretMode::DotenvFile, // VIOLATION 5: Dotenv in production
        };

        let redacted = redact_install_plan(&plan);
        assert_eq!(redacted["admin"]["password"]["value"], "***");
        assert_eq!(redacted["database"]["url"]["value"], "***");

        let report = evaluate_preflight(&plan);
        assert!(!report.passed());

        let error_codes: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.severity == PreflightSeverity::Error)
            .map(|i| i.code.as_str())
            .collect();

        assert!(error_codes.contains(&"production_database_engine"));
        assert!(error_codes.contains(&"sqlite_production"));
        assert!(error_codes.contains(&"dotenv_production"));
        assert!(error_codes.contains(&"dev_seed_production"));
        assert!(error_codes.contains(&"plaintext_secret_production"));
        assert!(error_codes.contains(&"sample_secret"));

        let has_warning = report.issues.iter().any(|i| {
            i.code == "schema_disable_not_supported" && i.severity == PreflightSeverity::Warning
        });
        assert!(has_warning);

        // evaluate_preflight_with_deployment missing deployment adapter fails
        let deploy_report = evaluate_preflight_with_deployment(&plan, false);
        assert!(!deploy_report.passed());
        assert!(
            deploy_report
                .issues
                .iter()
                .any(|i| i.code == "distribution_deployment_unavailable")
        );
    }
}
