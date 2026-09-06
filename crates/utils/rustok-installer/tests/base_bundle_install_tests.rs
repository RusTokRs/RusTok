use std::sync::Mutex;

use async_trait::async_trait;
use rustok_installer::{
    AdminBootstrap, InstallAdminOutcome, InstallAdminPort, InstallApplyOptions,
    InstallBootstrapPort, InstallComposition, InstallDatabasePort, InstallDatabaseReady,
    InstallDeploymentPort, InstallDistributionBinding, InstallDistributionDeployment,
    InstallDistributionDeploymentRequest, InstallExecutionError, InstallPersistencePort,
    InstallPlan, InstallReceipt, InstallReceiptRecord, InstallRoleDeploymentObservation,
    InstallSchemaPort, InstallSeedOutcome, InstallSeedPort, InstallSessionRecord, InstallState,
    InstallTopology, InstallTopologyMode, InstallVerificationOutcome, InstallVerificationPort,
    InstancePlacement, SecretRef, SecretValue, TenantBootstrap, execute_install_apply,
};
use uuid::Uuid;

#[derive(Default)]
struct TestPorts {
    pub states: Mutex<Vec<InstallState>>,
    pub receipts: Mutex<Vec<InstallReceipt>>,
    pub fail_at_bootstrap: Mutex<bool>,
    pub fail_at_seed: Mutex<bool>,
    pub fail_at_deployment: Mutex<bool>,
    pub tenant_id: Mutex<Option<Uuid>>,
    pub lock_owner: Mutex<Option<String>>,
}

#[async_trait]
impl InstallDatabasePort for TestPorts {
    type Runtime = ();

    async fn prepare_database(
        &self,
        _plan: &InstallPlan,
        _database_url: &str,
        _options: &InstallApplyOptions,
    ) -> Result<InstallDatabaseReady<Self::Runtime>, InstallExecutionError> {
        Ok(InstallDatabaseReady {
            runtime: (),
            database_name: Some("rustok_test".to_string()),
            created_database: true,
        })
    }
}

#[async_trait]
impl InstallSchemaPort<()> for TestPorts {
    async fn apply_owner_schema(&self, _runtime: &()) -> Result<(), InstallExecutionError> {
        Ok(())
    }

    async fn apply_remaining_schema(&self, _runtime: &()) -> Result<(), InstallExecutionError> {
        Ok(())
    }
}

#[async_trait]
impl InstallBootstrapPort<()> for TestPorts {
    async fn import_base_distribution(
        &self,
        _runtime: &(),
        _plan: &InstallPlan,
        _public_key_base64: Option<&str>,
    ) -> Result<
        Option<rustok_modules::ModuleStaticDistributionBootstrapImportReceipt>,
        InstallExecutionError,
    > {
        if *self.fail_at_bootstrap.lock().unwrap() {
            return Err(InstallExecutionError::new("simulated bootstrap import failure"));
        }
        Ok(None)
    }
}

#[async_trait]
impl InstallPersistencePort<()> for TestPorts {
    async fn create_session(
        &self,
        _runtime: &(),
        _plan: &InstallPlan,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        Ok(InstallSessionRecord {
            id: Uuid::from_u128(100),
            tenant_id: *self.tenant_id.lock().unwrap(),
            lock_owner: self.lock_owner.lock().unwrap().clone(),
            lock_expires_at: None,
        })
    }

    async fn acquire_lock(
        &self,
        _runtime: &(),
        session: InstallSessionRecord,
        owner: &str,
        _ttl_secs: i64,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        *self.lock_owner.lock().unwrap() = Some(owner.to_string());
        Ok(InstallSessionRecord {
            lock_owner: Some(owner.to_string()),
            ..session
        })
    }

    async fn record_receipt(
        &self,
        _runtime: &(),
        receipt: &InstallReceipt,
    ) -> Result<InstallReceiptRecord, InstallExecutionError> {
        self.receipts.lock().unwrap().push(receipt.clone());
        Ok(InstallReceiptRecord {
            id: Uuid::new_v4(),
            input_checksum: receipt.input_checksum.clone(),
        })
    }

    async fn set_state(
        &self,
        _runtime: &(),
        session_id: Uuid,
        state: InstallState,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        self.states.lock().unwrap().push(state);
        Ok(InstallSessionRecord {
            id: session_id,
            tenant_id: *self.tenant_id.lock().unwrap(),
            lock_owner: self.lock_owner.lock().unwrap().clone(),
            lock_expires_at: None,
        })
    }

    async fn set_tenant_id(
        &self,
        _runtime: &(),
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        *self.tenant_id.lock().unwrap() = Some(tenant_id);
        Ok(InstallSessionRecord {
            id: session_id,
            tenant_id: Some(tenant_id),
            lock_owner: self.lock_owner.lock().unwrap().clone(),
            lock_expires_at: None,
        })
    }
}

#[async_trait]
impl InstallSeedPort<()> for TestPorts {
    async fn apply_seed(
        &self,
        _runtime: &(),
        _plan: &InstallPlan,
    ) -> Result<InstallSeedOutcome, InstallExecutionError> {
        if *self.fail_at_seed.lock().unwrap() {
            return Err(InstallExecutionError::new("simulated seed error"));
        }
        Ok(InstallSeedOutcome {
            tenant_id: Uuid::from_u128(200),
            tenant_slug: "main".to_string(),
            tenant_created: true,
            enabled_modules: vec!["content".to_string(), "commerce".to_string()],
            disabled_modules: Vec::new(),
            demo_customer_created: false,
        })
    }
}

#[async_trait]
impl InstallAdminPort<()> for TestPorts {
    async fn provision_admin(
        &self,
        _runtime: &(),
        _plan: &InstallPlan,
        _tenant_id: Uuid,
        _password: &str,
    ) -> Result<InstallAdminOutcome, InstallExecutionError> {
        Ok(InstallAdminOutcome {
            user_id: Uuid::from_u128(300),
            email: "admin@example.com".to_string(),
            created: true,
        })
    }
}

#[async_trait]
impl InstallVerificationPort<()> for TestPorts {
    async fn verify_installation(
        &self,
        _runtime: &(),
        _plan: &InstallPlan,
        tenant_id: Uuid,
    ) -> Result<InstallVerificationOutcome, InstallExecutionError> {
        Ok(InstallVerificationOutcome {
            tenant_id,
            tenant_slug: "main".to_string(),
            admin_user_id: Uuid::from_u128(300),
            enabled_modules: vec!["content".to_string(), "commerce".to_string()],
        })
    }
}

#[async_trait]
impl InstallDeploymentPort<()> for TestPorts {
    fn supports_distribution_deployment(&self) -> bool {
        true
    }

    async fn deploy_distribution(
        &self,
        _runtime: &(),
        request: InstallDistributionDeploymentRequest,
    ) -> Result<InstallDistributionDeployment, InstallExecutionError> {
        if *self.fail_at_deployment.lock().unwrap() {
            return Err(InstallExecutionError::new("simulated deployment rollout failure"));
        }

        let observations = request
            .roles
            .iter()
            .map(|assignment| InstallRoleDeploymentObservation {
                role: assignment.role,
                surfaces: assignment.surfaces.clone(),
                artifact_digest: request
                    .distribution
                    .roles
                    .iter()
                    .find(|artifact| artifact.role == assignment.role.static_distribution_role())
                    .expect("role must exist in bundle")
                    .artifact_digest
                    .clone(),
                health_evidence_reference: format!("probe://healthy/{}", assignment.role.as_str()),
            })
            .collect();

        Ok(InstallDistributionDeployment {
            composition: request.composition.clone(),
            distribution: request.distribution.clone(),
            rollout_id: Uuid::new_v4(),
            deployment_reference: "owner-rollout:candidate-001".to_string(),
            observations,
        })
    }
}

fn ensure_test_env_secrets() {
    // SAFETY: In tests, setting static environment variables for secret resolution is safe.
    unsafe {
        std::env::set_var("DATABASE_URL", "sqlite::memory:");
        std::env::set_var("ADMIN_PASSWORD", "SuperSecret123!");
    }
}

fn sample_plan(root: &std::path::Path) -> InstallPlan {
    ensure_test_env_secrets();

    let roles = [
        (
            rustok_modules::ModuleStaticDistributionRole::Api,
            format!("sha256:{}", "1".repeat(64)),
        ),
        (
            rustok_modules::ModuleStaticDistributionRole::AdminSsr,
            format!("sha256:{}", "2".repeat(64)),
        ),
        (
            rustok_modules::ModuleStaticDistributionRole::StorefrontSsr,
            format!("sha256:{}", "3".repeat(64)),
        ),
        (
            rustok_modules::ModuleStaticDistributionRole::Worker,
            format!("sha256:{}", "4".repeat(64)),
        ),
    ]
    .into_iter()
    .map(
        |(role, artifact_digest)| rustok_modules::ModuleStaticDistributionRoleArtifact {
            role,
            artifact_digest,
        },
    )
    .collect::<Vec<_>>();

    let distribution = InstallDistributionBinding {
        preparation_id: Uuid::from_u128(1),
        distribution_release_id: Uuid::from_u128(2),
        bundle_reference: format!("registry.example/rustok/base@sha256:{}", "a".repeat(64)),
        bundle_root_digest: format!("sha256:{}", "a".repeat(64)),
        role_set_digest:
            rustok_modules::ModuleStaticDistributionBuildEvidence::role_set_digest(&roles).unwrap(),
        roles,
        bootstrap_receipt: None,
    };

    let mut plan = InstallPlan::production_minimal(
        InstancePlacement::new(root.display().to_string()),
        SecretValue::Reference {
            reference: SecretRef {
                backend: "env".to_string(),
                key: "DATABASE_URL".to_string(),
            },
        },
        TenantBootstrap {
            slug: "main".to_string(),
            name: "Main".to_string(),
        },
        AdminBootstrap {
            email: "admin@example.com".to_string(),
            password: SecretValue::Reference {
                reference: SecretRef {
                    backend: "env".to_string(),
                    key: "ADMIN_PASSWORD".to_string(),
                },
            },
        },
        InstallComposition {
            revision: "distribution@1".to_string(),
            hash: "a".repeat(64),
        },
        distribution.clone(),
    );
    plan.topology = InstallTopology::for_mode(InstallTopologyMode::Distributed)
        .bind_composition("distribution@1".to_string(), "a".repeat(64))
        .bind_distribution(distribution);
    plan
}

#[tokio::test]
async fn test_base_bundle_install_happy_path_and_candidate_role_prestaging() {
    let temp = std::env::temp_dir().join(format!("rustok-install-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();

    let ports = TestPorts::default();
    let plan = sample_plan(&temp);
    let options = InstallApplyOptions::default();

    let output = execute_install_apply(&ports, plan, options)
        .await
        .expect("First-install apply must succeed");

    assert_eq!(output.status, "completed");
    assert_eq!(output.tenant_id, Some(Uuid::from_u128(200)));

    // Verify candidate role deployment and health observations
    assert_eq!(output.deployment_receipt.deployment.observations.len(), 4);
    for obs in &output.deployment_receipt.deployment.observations {
        assert!(obs.health_evidence_reference.starts_with("probe://healthy/"));
    }

    // Verify step progression
    let states = ports.states.lock().unwrap().clone();
    assert_eq!(
        states,
        vec![
            InstallState::PreflightPassed,
            InstallState::ConfigPrepared,
            InstallState::DatabaseReady,
            InstallState::SchemaApplied,
            InstallState::SeedApplied,
            InstallState::AdminProvisioned,
            InstallState::Deploying,
            InstallState::Verified,
            InstallState::Completed,
        ]
    );

    // Verify receipts were durably recorded
    let receipts = ports.receipts.lock().unwrap().clone();
    assert_eq!(receipts.len(), 9); // Preflight, Config, Database, Migrate, Seed, Admin, Deploy, Verify, Finalize

    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test]
async fn test_fresh_install_cleanup_on_pre_schema_failure() {
    let temp = std::env::temp_dir().join(format!("rustok-install-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();

    let ports = TestPorts::default();
    *ports.fail_at_bootstrap.lock().unwrap() = true;

    let plan = sample_plan(&temp);
    let options = InstallApplyOptions::default();

    let err = execute_install_apply(&ports, plan, options)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("simulated bootstrap import failure"));

    // Verify state transitioned to FreshInstallCleaned
    let states = ports.states.lock().unwrap().clone();
    assert_eq!(states, vec![InstallState::FreshInstallCleaned]);

    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test]
async fn test_recovery_required_on_post_schema_seed_failure() {
    let temp = std::env::temp_dir().join(format!("rustok-install-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();

    let ports = TestPorts::default();
    *ports.fail_at_seed.lock().unwrap() = true;

    let plan = sample_plan(&temp);
    let options = InstallApplyOptions::default();

    let err = execute_install_apply(&ports, plan, options)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("simulated seed error"));

    // Verify that after SchemaApplied, failure marks RecoveryRequired
    let states = ports.states.lock().unwrap().clone();
    assert_eq!(
        states,
        vec![
            InstallState::PreflightPassed,
            InstallState::ConfigPrepared,
            InstallState::DatabaseReady,
            InstallState::SchemaApplied,
            InstallState::RecoveryRequired,
        ]
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test]
async fn test_recovery_required_on_deployment_rollout_failure() {
    let temp = std::env::temp_dir().join(format!("rustok-install-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp).unwrap();

    let ports = TestPorts::default();
    *ports.fail_at_deployment.lock().unwrap() = true;

    let plan = sample_plan(&temp);
    let options = InstallApplyOptions::default();

    let err = execute_install_apply(&ports, plan, options)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("simulated deployment rollout failure"));

    let states = ports.states.lock().unwrap().clone();
    assert_eq!(
        states,
        vec![
            InstallState::PreflightPassed,
            InstallState::ConfigPrepared,
            InstallState::DatabaseReady,
            InstallState::SchemaApplied,
            InstallState::SeedApplied,
            InstallState::AdminProvisioned,
            InstallState::Deploying,
            InstallState::RecoveryRequired,
        ]
    );

    let _ = std::fs::remove_dir_all(&temp);
}
