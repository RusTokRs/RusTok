//! Typed distributed-role deployment contracts owned by the installer.
//!
//! This module deliberately has no build-system, container, HTTP, or cloud
//! dependency. A host adapter translates a request into `rustok-build` work
//! and waits until the resulting release is active.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    InstallComposition, InstallEnvironment, InstallExecutionError, InstallPersistencePort,
    InstallPlan, InstallReceipt, InstallRole, InstallSessionRecord, InstallState, InstallStep,
    InstallSurface, InstallTopologyMode,
};

/// One immutable role hand-off produced from a distributed install topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRoleDeploymentRequest {
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub environment: InstallEnvironment,
    pub composition: InstallComposition,
    pub role: InstallRole,
    pub surfaces: Vec<InstallSurface>,
}

/// Durable identity returned when one distributed role is active.
///
/// Adapters must return the existing matching release on retry. They must not
/// rebuild or redeploy a role whose composition, role, and surface set already
/// have an active receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRoleDeployment {
    pub role: InstallRole,
    pub composition: InstallComposition,
    pub build_id: String,
    pub release_id: String,
    pub deployment_reference: String,
}

/// Installer receipt linking one active role release to a durable session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRoleDeploymentReceipt {
    pub deployment: InstallRoleDeployment,
    pub receipt_id: Uuid,
    pub receipt_checksum: String,
}

/// Result of recording all distributed role deployments for an install session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedDeploymentOutput {
    pub session: InstallSessionRecord,
    pub receipts: Vec<InstallRoleDeploymentReceipt>,
}

/// Host boundary for one role-specific build and deployment.
///
/// The adapter owns the `rustok-build` request, release publication, polling,
/// and provider-specific idempotency. It must return only after the matching
/// role is active and must reject a release for another composition.
#[async_trait::async_trait]
pub trait InstallDeploymentPort<R>: Send + Sync {
    fn supports_distributed_deployment(&self) -> bool;

    async fn deploy_role(
        &self,
        runtime: &R,
        request: InstallRoleDeploymentRequest,
    ) -> Result<InstallRoleDeployment, InstallExecutionError>;
}

/// Creates a deterministic set of independent distributed-role hand-offs.
pub fn distributed_deployment_requests(
    plan: &InstallPlan,
    session_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<InstallRoleDeploymentRequest>, InstallExecutionError> {
    if plan.topology.mode != InstallTopologyMode::Distributed {
        return Err(InstallExecutionError::new(
            "role deployment requests require a distributed install topology",
        ));
    }
    plan.topology
        .validate()
        .map_err(InstallExecutionError::new)?;
    let composition = plan.topology.composition.clone().ok_or_else(|| {
        InstallExecutionError::new("distributed install topology requires a composition")
    })?;
    let mut assignments = plan.topology.roles.clone();
    assignments.sort_by_key(|assignment| role_sort_key(assignment.role));

    Ok(assignments
        .into_iter()
        .map(|mut assignment| {
            assignment
                .surfaces
                .sort_by_key(|surface| surface_sort_key(*surface));
            InstallRoleDeploymentRequest {
                session_id,
                tenant_id,
                environment: plan.environment,
                composition: composition.clone(),
                role: assignment.role,
                surfaces: assignment.surfaces,
            }
        })
        .collect())
}

/// Deploys every distributed role exactly once through a host adapter and
/// records a receipt for every active release.
///
/// Callers invoke this after the shared database, schema, tenant, and admin
/// stages. The adapter is responsible for retrying a matching active role
/// release instead of issuing another build. The receipt checksum is derived
/// from the immutable role request, so each role remains independently auditable.
pub async fn execute_distributed_role_deployments<P, R>(
    ports: &P,
    runtime: &R,
    plan: &InstallPlan,
    session: InstallSessionRecord,
    tenant_id: Uuid,
) -> Result<DistributedDeploymentOutput, InstallExecutionError>
where
    P: InstallPersistencePort<R> + InstallDeploymentPort<R>,
    R: Send + Sync,
{
    let requests = distributed_deployment_requests(plan, session.id, tenant_id)?;
    let session = ports
        .set_state(runtime, session.id, InstallState::Deploying)
        .await?;
    let mut receipts = Vec::with_capacity(requests.len());

    for request in requests {
        let deployment = ports.deploy_role(runtime, request.clone()).await?;
        validate_deployment(&request, &deployment)?;
        let receipt = InstallReceipt::success(
            session.id.to_string(),
            InstallStep::Deploy,
            &request,
            serde_json::json!({
                "role": deployment.role,
                "surfaces": &request.surfaces,
                "composition": &deployment.composition,
                "build_id": &deployment.build_id,
                "release_id": &deployment.release_id,
                "deployment_reference": &deployment.deployment_reference,
            }),
        )
        .map_err(|error| InstallExecutionError::new(error.to_string()))?;
        let recorded = ports.record_receipt(runtime, &receipt).await?;
        receipts.push(InstallRoleDeploymentReceipt {
            deployment,
            receipt_id: recorded.id,
            receipt_checksum: recorded.input_checksum,
        });
    }

    Ok(DistributedDeploymentOutput { session, receipts })
}

fn validate_deployment(
    request: &InstallRoleDeploymentRequest,
    deployment: &InstallRoleDeployment,
) -> Result<(), InstallExecutionError> {
    if deployment.role != request.role {
        return Err(InstallExecutionError::new(
            "deployment adapter returned a release for a different role",
        ));
    }
    if deployment.composition != request.composition {
        return Err(InstallExecutionError::new(
            "deployment adapter returned a release for a different composition",
        ));
    }
    for (label, value) in [
        ("build_id", &deployment.build_id),
        ("release_id", &deployment.release_id),
        ("deployment_reference", &deployment.deployment_reference),
    ] {
        if value.trim().is_empty() {
            return Err(InstallExecutionError::new(format!(
                "deployment adapter returned an empty {label}"
            )));
        }
    }
    Ok(())
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
        InstallComposition, InstallExecutionError, InstallPersistencePort, InstallPlan,
        InstallReceipt, InstallReceiptRecord, InstallRole, InstallRoleAssignment,
        InstallSessionRecord, InstallState, InstallSurface, InstallTopology, InstallTopologyMode,
    };

    use super::{
        InstallDeploymentPort, InstallRoleDeployment, InstallRoleDeploymentRequest,
        distributed_deployment_requests, execute_distributed_role_deployments, validate_deployment,
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
        fn supports_distributed_deployment(&self) -> bool {
            true
        }

        async fn deploy_role(
            &self,
            _runtime: &(),
            request: InstallRoleDeploymentRequest,
        ) -> Result<InstallRoleDeployment, InstallExecutionError> {
            Ok(InstallRoleDeployment {
                role: request.role,
                composition: request.composition,
                build_id: format!("build_{:?}", request.role),
                release_id: format!("release_{:?}", request.role),
                deployment_reference: format!("deployment_{:?}", request.role),
            })
        }
    }

    #[test]
    fn distributed_requests_are_role_and_surface_deterministic() {
        let mut topology = InstallTopology::for_mode(InstallTopologyMode::Distributed)
            .bind_composition("distribution@1".to_string(), "a".repeat(64));
        topology.roles.reverse();
        topology.roles[0] = InstallRoleAssignment {
            role: InstallRole::Worker,
            surfaces: vec![InstallSurface::Worker],
        };
        let mut plan = sample_plan();
        plan.topology = topology;

        let requests = distributed_deployment_requests(&plan, Uuid::nil(), Uuid::nil()).unwrap();

        assert_eq!(requests.len(), 4);
        assert_eq!(requests[0].role, InstallRole::Api);
        assert_eq!(requests[3].role, InstallRole::Worker);
        assert!(requests.iter().all(|request| request.composition
            == InstallComposition {
                revision: "distribution@1".to_string(),
                hash: "a".repeat(64),
            }));
    }

    #[test]
    fn deployment_result_must_match_the_requested_role_and_composition() {
        let mut plan = sample_plan();
        plan.topology = InstallTopology::for_mode(InstallTopologyMode::Distributed)
            .bind_composition("distribution@1".to_string(), "a".repeat(64));
        let request = distributed_deployment_requests(&plan, Uuid::nil(), Uuid::nil())
            .unwrap()
            .remove(0);
        let invalid = InstallRoleDeployment {
            role: InstallRole::Worker,
            composition: request.composition.clone(),
            build_id: "build_1".to_string(),
            release_id: "release_1".to_string(),
            deployment_reference: "deployment_1".to_string(),
        };

        assert!(validate_deployment(&request, &invalid).is_err());
    }

    #[tokio::test]
    async fn distributed_deployments_record_one_receipt_per_role() {
        let mut plan = sample_plan();
        plan.topology = InstallTopology::for_mode(InstallTopologyMode::Distributed)
            .bind_composition("distribution@1".to_string(), "a".repeat(64));
        let ports = FakePorts::default();

        let output =
            execute_distributed_role_deployments(&ports, &(), &plan, session(), Uuid::nil())
                .await
                .unwrap();

        assert_eq!(output.receipts.len(), 4);
        assert!(
            output
                .receipts
                .iter()
                .all(|receipt| receipt.deployment.composition.hash == "a".repeat(64))
        );
        assert_eq!(ports.receipts.lock().unwrap().len(), 4);
        assert_eq!(
            ports.states.lock().unwrap().as_slice(),
            &[InstallState::Deploying]
        );
    }

    fn session() -> InstallSessionRecord {
        InstallSessionRecord {
            id: Uuid::nil(),
            tenant_id: None,
            lock_owner: None,
            lock_expires_at: None,
        }
    }

    fn sample_plan() -> InstallPlan {
        InstallPlan::production_minimal(
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
        )
    }
}
