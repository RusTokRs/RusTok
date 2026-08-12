//! Typed immutable-distribution deployment contracts owned by the installer.
//!
//! This module deliberately has no build-system, container, HTTP, or cloud
//! dependency. Installer apply hands one already admitted role bundle to a
//! host adapter and waits for complete per-role observations. It never creates
//! independent build or release heads for individual roles.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    InstallComposition, InstallDistributionBinding, InstallEnvironment, InstallExecutionError,
    InstallPersistencePort, InstallPlan, InstallReceipt, InstallRole, InstallRoleAssignment,
    InstallSessionRecord, InstallState, InstallStep, InstallSurface,
};

/// One immutable hand-off for the complete admitted distribution role bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallDistributionDeploymentRequest {
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub environment: InstallEnvironment,
    pub composition: InstallComposition,
    pub distribution: InstallDistributionBinding,
    pub roles: Vec<InstallRoleAssignment>,
}

/// Healthy observation for one role from the owner-controlled rollout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallRoleDeploymentObservation {
    pub role: InstallRole,
    pub surfaces: Vec<InstallSurface>,
    pub artifact_digest: String,
    pub health_evidence_reference: String,
}

/// Durable result returned only after the complete bundle converges healthy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallDistributionDeployment {
    pub composition: InstallComposition,
    pub distribution: InstallDistributionBinding,
    pub rollout_id: Uuid,
    pub deployment_reference: String,
    pub observations: Vec<InstallRoleDeploymentObservation>,
}

/// Installer receipt linking one converged distribution rollout to a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallDistributionDeploymentReceipt {
    pub deployment: InstallDistributionDeployment,
    pub receipt_id: Uuid,
    pub receipt_checksum: String,
}

/// Result of recording the complete distribution deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionDeploymentOutput {
    pub session: InstallSessionRecord,
    pub receipt: InstallDistributionDeploymentReceipt,
}

/// Host boundary for one owner-controlled distribution rollout.
///
/// The adapter consumes an already admitted immutable bundle. It may reconcile
/// and observe deployment, but it may not compile, publish, activate a second
/// release head, or substitute another bundle identity.
#[async_trait::async_trait]
pub trait InstallDeploymentPort<R>: Send + Sync {
    fn supports_distribution_deployment(&self) -> bool;

    async fn deploy_distribution(
        &self,
        runtime: &R,
        request: InstallDistributionDeploymentRequest,
    ) -> Result<InstallDistributionDeployment, InstallExecutionError>;
}

/// Creates the single deterministic deployment request for any install plan.
///
/// Monolith and distributed installations differ only in the role assignments
/// inside the immutable bundle. Both use this owner-controlled deployment path.
pub fn distribution_deployment_request(
    plan: &InstallPlan,
    session_id: Uuid,
    tenant_id: Uuid,
) -> Result<InstallDistributionDeploymentRequest, InstallExecutionError> {
    plan.topology
        .validate()
        .map_err(InstallExecutionError::new)?;
    let composition = plan
        .topology
        .composition
        .clone()
        .ok_or_else(|| InstallExecutionError::new("install topology requires a composition"))?;
    let distribution = plan.topology.distribution.clone().ok_or_else(|| {
        InstallExecutionError::new("install topology requires an admitted distribution bundle")
    })?;
    let mut roles = plan.topology.roles.clone();
    roles.sort_by_key(|assignment| role_sort_key(assignment.role));
    for assignment in &mut roles {
        assignment
            .surfaces
            .sort_by_key(|surface| surface_sort_key(*surface));
    }

    Ok(InstallDistributionDeploymentRequest {
        session_id,
        tenant_id,
        environment: plan.environment,
        composition,
        distribution,
        roles,
    })
}

/// Reconciles one complete role bundle and records one bound deployment receipt.
pub async fn execute_distribution_deployment<P, R>(
    ports: &P,
    runtime: &R,
    plan: &InstallPlan,
    session: InstallSessionRecord,
    tenant_id: Uuid,
) -> Result<DistributionDeploymentOutput, InstallExecutionError>
where
    P: InstallPersistencePort<R> + InstallDeploymentPort<R>,
    R: Send + Sync,
{
    let request = distribution_deployment_request(plan, session.id, tenant_id)?;
    let session = ports
        .set_state(runtime, session.id, InstallState::Deploying)
        .await?;
    let deployment = ports.deploy_distribution(runtime, request.clone()).await?;
    validate_deployment(&request, &deployment)?;
    let receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Deploy,
        &request,
        serde_json::json!({
            "composition": &deployment.composition,
            "distribution": &deployment.distribution,
            "rollout_id": deployment.rollout_id,
            "deployment_reference": &deployment.deployment_reference,
            "observations": &deployment.observations,
        }),
    )
    .map_err(|error| InstallExecutionError::new(error.to_string()))?;
    let recorded = ports.record_receipt(runtime, &receipt).await?;

    Ok(DistributionDeploymentOutput {
        session,
        receipt: InstallDistributionDeploymentReceipt {
            deployment,
            receipt_id: recorded.id,
            receipt_checksum: recorded.input_checksum,
        },
    })
}

fn validate_deployment(
    request: &InstallDistributionDeploymentRequest,
    deployment: &InstallDistributionDeployment,
) -> Result<(), InstallExecutionError> {
    if deployment.composition != request.composition {
        return Err(InstallExecutionError::new(
            "deployment adapter observed a different composition",
        ));
    }
    if deployment.distribution != request.distribution {
        return Err(InstallExecutionError::new(
            "deployment adapter observed a different distribution bundle",
        ));
    }
    if deployment.rollout_id.is_nil() {
        return Err(InstallExecutionError::new(
            "deployment adapter returned a nil rollout ID",
        ));
    }
    if deployment.deployment_reference.trim().is_empty() {
        return Err(InstallExecutionError::new(
            "deployment adapter returned an empty deployment reference",
        ));
    }
    if deployment.observations.len() != request.roles.len() {
        return Err(InstallExecutionError::new(
            "deployment adapter did not return exactly one observation per role",
        ));
    }

    let mut expected = request.roles.clone();
    expected.sort_by_key(|assignment| role_sort_key(assignment.role));
    let mut observed = deployment.observations.clone();
    observed.sort_by_key(|observation| role_sort_key(observation.role));
    for (assignment, observation) in expected.iter_mut().zip(observed.iter_mut()) {
        assignment
            .surfaces
            .sort_by_key(|surface| surface_sort_key(*surface));
        observation
            .surfaces
            .sort_by_key(|surface| surface_sort_key(*surface));
        if assignment.role != observation.role || assignment.surfaces != observation.surfaces {
            return Err(InstallExecutionError::new(
                "deployment adapter returned an observation for a different role assignment",
            ));
        }
        let expected_role = assignment.role.static_distribution_role();
        let Some(expected_artifact) = request
            .distribution
            .roles
            .iter()
            .find(|artifact| artifact.role == expected_role)
        else {
            return Err(InstallExecutionError::new(
                "deployment request role is absent from the admitted bundle",
            ));
        };
        if !valid_sha256_digest(&observation.artifact_digest) {
            return Err(InstallExecutionError::new(
                "deployment role observation requires a canonical artifact digest",
            ));
        }
        if observation.artifact_digest != expected_artifact.artifact_digest {
            return Err(InstallExecutionError::new(
                "deployment role observation has a different artifact digest",
            ));
        }
        if observation.health_evidence_reference.trim().is_empty() {
            return Err(InstallExecutionError::new(
                "deployment role observation requires health evidence",
            ));
        }
    }
    Ok(())
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn role_sort_key(role: InstallRole) -> u8 {
    match role {
        InstallRole::Api => 1,
        InstallRole::AdminSsr => 2,
        InstallRole::StorefrontSsr => 3,
        InstallRole::Worker => 4,
        InstallRole::Registry => 5,
        InstallRole::Monolith => 6,
    }
}

fn surface_sort_key(surface: InstallSurface) -> u8 {
    match surface {
        InstallSurface::Api => 1,
        InstallSurface::Admin => 2,
        InstallSurface::Storefront => 3,
        InstallSurface::Worker => 4,
        InstallSurface::Registry => 5,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use uuid::Uuid;

    use crate::{
        InstallComposition, InstallDeploymentPort, InstallDistributionBinding,
        InstallDistributionDeployment, InstallDistributionDeploymentRequest, InstallExecutionError,
        InstallPersistencePort, InstallPlan, InstallReceipt, InstallReceiptRecord, InstallRole,
        InstallRoleDeploymentObservation, InstallSessionRecord, InstallState, InstallTopology,
        InstallTopologyMode,
    };

    use super::{
        distribution_deployment_request, execute_distribution_deployment, validate_deployment,
    };

    #[derive(Default)]
    struct FakePorts {
        receipts: Mutex<Vec<InstallReceipt>>,
        states: Mutex<Vec<InstallState>>,
    }

    #[async_trait]
    impl InstallPersistencePort<()> for FakePorts {
        async fn create_session(
            &self,
            _runtime: &(),
            _plan: &InstallPlan,
        ) -> Result<InstallSessionRecord, InstallExecutionError> {
            Ok(session())
        }

        async fn acquire_lock(
            &self,
            _runtime: &(),
            session: InstallSessionRecord,
            _owner: &str,
            _ttl_secs: i64,
        ) -> Result<InstallSessionRecord, InstallExecutionError> {
            Ok(session)
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
                ..session()
            })
        }

        async fn set_tenant_id(
            &self,
            _runtime: &(),
            session_id: Uuid,
            tenant_id: Uuid,
        ) -> Result<InstallSessionRecord, InstallExecutionError> {
            Ok(InstallSessionRecord {
                id: session_id,
                tenant_id: Some(tenant_id),
                ..session()
            })
        }
    }

    #[async_trait]
    impl InstallDeploymentPort<()> for FakePorts {
        fn supports_distribution_deployment(&self) -> bool {
            true
        }

        async fn deploy_distribution(
            &self,
            _runtime: &(),
            request: InstallDistributionDeploymentRequest,
        ) -> Result<InstallDistributionDeployment, InstallExecutionError> {
            Ok(successful_deployment(&request))
        }
    }

    #[test]
    fn distribution_request_contains_one_sorted_complete_bundle() {
        let mut plan = sample_plan();
        plan.topology.roles.reverse();

        let request = distribution_deployment_request(&plan, Uuid::nil(), Uuid::nil()).unwrap();

        assert_eq!(request.roles.len(), 4);
        assert_eq!(request.roles[0].role, InstallRole::Api);
        assert_eq!(request.roles[3].role, InstallRole::Worker);
        assert_eq!(request.distribution, distribution());
    }

    #[test]
    fn monolith_request_uses_the_same_distribution_deployment_contract() {
        let mut plan = sample_plan();
        plan.topology = InstallTopology::for_mode(InstallTopologyMode::Monolith)
            .bind_composition("distribution@1".to_string(), "a".repeat(64))
            .bind_distribution(monolith_distribution());

        let request = distribution_deployment_request(&plan, Uuid::nil(), Uuid::nil()).unwrap();

        assert_eq!(request.roles.len(), 1);
        assert_eq!(request.roles[0].role, InstallRole::Monolith);
        assert_eq!(request.distribution, monolith_distribution());
    }

    #[test]
    fn deployment_must_match_bundle_and_complete_role_set() {
        let request =
            distribution_deployment_request(&sample_plan(), Uuid::nil(), Uuid::nil()).unwrap();
        let mut invalid = successful_deployment(&request);
        invalid.distribution.bundle_root_digest = format!("sha256:{}", "f".repeat(64));
        assert!(validate_deployment(&request, &invalid).is_err());

        let mut incomplete = successful_deployment(&request);
        incomplete.observations.pop();
        assert!(validate_deployment(&request, &incomplete).is_err());

        let mut wrong_role_bytes = successful_deployment(&request);
        wrong_role_bytes.observations[0].artifact_digest = format!("sha256:{}", "e".repeat(64));
        assert!(validate_deployment(&request, &wrong_role_bytes).is_err());
    }

    #[tokio::test]
    async fn distribution_deployment_records_one_receipt() {
        let ports = FakePorts::default();

        let output =
            execute_distribution_deployment(&ports, &(), &sample_plan(), session(), Uuid::nil())
                .await
                .unwrap();

        assert_eq!(output.receipt.deployment.distribution, distribution());
        assert_eq!(ports.receipts.lock().unwrap().len(), 1);
        assert_eq!(
            ports.states.lock().unwrap().as_slice(),
            &[InstallState::Deploying]
        );
    }

    fn successful_deployment(
        request: &InstallDistributionDeploymentRequest,
    ) -> InstallDistributionDeployment {
        InstallDistributionDeployment {
            composition: request.composition.clone(),
            distribution: request.distribution.clone(),
            rollout_id: Uuid::new_v4(),
            deployment_reference: "owner-rollout:1".to_string(),
            observations: request
                .roles
                .iter()
                .map(|assignment| InstallRoleDeploymentObservation {
                    role: assignment.role,
                    surfaces: assignment.surfaces.clone(),
                    artifact_digest: request
                        .distribution
                        .roles
                        .iter()
                        .find(|artifact| {
                            artifact.role == assignment.role.static_distribution_role()
                        })
                        .expect("sample bundle contains every assigned role")
                        .artifact_digest
                        .clone(),
                    health_evidence_reference: format!("health://{}", assignment.role.as_str()),
                })
                .collect(),
        }
    }

    fn distribution() -> InstallDistributionBinding {
        let roles = [
            rustok_modules::ModuleStaticDistributionRole::Api,
            rustok_modules::ModuleStaticDistributionRole::AdminSsr,
            rustok_modules::ModuleStaticDistributionRole::StorefrontSsr,
            rustok_modules::ModuleStaticDistributionRole::Worker,
        ]
        .into_iter()
        .enumerate()
        .map(
            |(index, role)| rustok_modules::ModuleStaticDistributionRoleArtifact {
                role,
                artifact_digest: format!("sha256:{:064x}", index + 1),
            },
        )
        .collect::<Vec<_>>();
        InstallDistributionBinding {
            preparation_id: Uuid::from_u128(1),
            distribution_release_id: Uuid::from_u128(2),
            bundle_reference: format!("registry.example/rustok/base@sha256:{}", "a".repeat(64)),
            bundle_root_digest: format!("sha256:{}", "a".repeat(64)),
            role_set_digest:
                rustok_modules::ModuleStaticDistributionBuildEvidence::role_set_digest(&roles)
                    .unwrap(),
            roles,
            bootstrap_receipt: None,
        }
    }

    fn monolith_distribution() -> InstallDistributionBinding {
        let roles = vec![rustok_modules::ModuleStaticDistributionRoleArtifact {
            role: rustok_modules::ModuleStaticDistributionRole::Monolith,
            artifact_digest: format!("sha256:{}", "f".repeat(64)),
        }];
        InstallDistributionBinding {
            preparation_id: Uuid::from_u128(1),
            distribution_release_id: Uuid::from_u128(2),
            bundle_reference: format!("registry.example/rustok/base@sha256:{}", "a".repeat(64)),
            bundle_root_digest: format!("sha256:{}", "a".repeat(64)),
            role_set_digest:
                rustok_modules::ModuleStaticDistributionBuildEvidence::role_set_digest(&roles)
                    .unwrap(),
            roles,
            bootstrap_receipt: None,
        }
    }

    fn session() -> InstallSessionRecord {
        InstallSessionRecord {
            id: Uuid::from_u128(3),
            tenant_id: None,
            lock_owner: None,
            lock_expires_at: None,
        }
    }

    fn sample_plan() -> InstallPlan {
        let mut plan = InstallPlan::production_minimal(
            crate::InstancePlacement::new("."),
            crate::SecretValue::Reference {
                reference: crate::SecretRef {
                    backend: "environment".to_string(),
                    key: "DATABASE_URL".to_string(),
                },
            },
            crate::TenantBootstrap {
                slug: "main".to_string(),
                name: "Main".to_string(),
            },
            crate::AdminBootstrap {
                email: "admin@example.com".to_string(),
                password: crate::SecretValue::Reference {
                    reference: crate::SecretRef {
                        backend: "environment".to_string(),
                        key: "ADMIN_PASSWORD".to_string(),
                    },
                },
            },
            InstallComposition {
                revision: "distribution@1".to_string(),
                hash: "a".repeat(64),
            },
            distribution(),
        );
        plan.topology = InstallTopology::for_mode(InstallTopologyMode::Distributed)
            .bind_composition("distribution@1".to_string(), "a".repeat(64))
            .bind_distribution(distribution());
        plan
    }
}
