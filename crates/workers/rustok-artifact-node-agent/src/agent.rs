use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_artifact_node_transport::GrpcArtifactNodeAgent;
use rustok_modules::{
    ModuleArtifactNodeAssignmentHeartbeatReceipt, ModuleArtifactNodeAssignmentReport,
    ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeAssignmentWorkItem,
    ModuleReconciliationEvidence, ModuleReconciliationFailure, ModuleReconciliationPhase,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::watch;
use uuid::Uuid;

const HEALTH_EVIDENCE_CONTRACT: &str = "rustok.artifact-node-agent.health";
const MAX_FAILURE_CODE_BYTES: usize = 128;
const MAX_FAILURE_DETAIL_BYTES: usize = 2_000;

/// Narrow client contract used by the independently deployed node agent. The
/// concrete mTLS transport derives node and agent identity from the client
/// certificate; no method accepts topology or artifact selection input.
#[async_trait]
pub trait ArtifactNodeAssignmentController: Send + Sync {
    async fn claim_assignment(
        &self,
    ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, String>;

    async fn heartbeat_assignment(
        &self,
        claim_id: Uuid,
    ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, String>;

    async fn report_assignment(
        &self,
        report: &ModuleArtifactNodeAssignmentReport,
    ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, String>;
}

#[async_trait]
impl ArtifactNodeAssignmentController for GrpcArtifactNodeAgent {
    async fn claim_assignment(
        &self,
    ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, String> {
        GrpcArtifactNodeAgent::claim_assignment(self).await
    }

    async fn heartbeat_assignment(
        &self,
        claim_id: Uuid,
    ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, String> {
        GrpcArtifactNodeAgent::heartbeat_assignment(self, claim_id).await
    }

    async fn report_assignment(
        &self,
        report: &ModuleArtifactNodeAssignmentReport,
    ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, String> {
        GrpcArtifactNodeAgent::report_assignment(self, report).await
    }
}

/// Non-secret local preparation identity. The materializer keeps its cache
/// paths private; reports carry only this immutable runtime fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeArtifactPreparation {
    pub runtime_fingerprint: String,
}

/// Distinguishes a retryable infrastructure condition from a terminal exact
/// artifact or local-boundary failure. Retryable conditions are never reported
/// as `healthy` or terminal `failed`; the owner lease is allowed to expire or
/// be reclaimed for another attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactNodeMaterializationError {
    Retryable { detail: String },
    Terminal { code: String, detail: String },
}

impl ArtifactNodeMaterializationError {
    pub fn retryable(detail: impl Into<String>) -> Self {
        Self::Retryable {
            detail: bounded_detail(detail.into()),
        }
    }

    pub fn terminal(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Terminal {
            code: bounded_failure_code(code.into()),
            detail: bounded_detail(detail.into()),
        }
    }
}

#[async_trait]
pub trait ArtifactNodeMaterializer: Send + Sync {
    /// Downloads, rehashes, and atomically materializes the exact assigned
    /// payload, then performs non-executing local payload preparation.
    async fn prepare(
        &self,
        work: &ModuleArtifactNodeAssignmentWorkItem,
    ) -> Result<NodeArtifactPreparation, ArtifactNodeMaterializationError>;

    /// Revalidates the local payload and performs the exact runtime readiness
    /// check needed before the owner may count an assignment healthy.
    async fn verify_ready(
        &self,
        work: &ModuleArtifactNodeAssignmentWorkItem,
    ) -> Result<NodeArtifactPreparation, ArtifactNodeMaterializationError>;
}

/// One independently supervised dynamic artifact node agent. It does not own
/// desired topology, policy, database state, capability grants, or activation.
pub struct ArtifactNodeAgent<C, M> {
    controller: Arc<C>,
    materializer: Arc<M>,
    identity: rustok_artifact_node_transport::ModuleArtifactNodeAgentIdentity,
    heartbeat_interval: Duration,
}

impl<C, M> ArtifactNodeAgent<C, M>
where
    C: ArtifactNodeAssignmentController + 'static,
    M: ArtifactNodeMaterializer + 'static,
{
    pub fn new(
        controller: Arc<C>,
        materializer: Arc<M>,
        identity: rustok_artifact_node_transport::ModuleArtifactNodeAgentIdentity,
        heartbeat_interval: Duration,
    ) -> Result<Self, ArtifactNodeAgentError> {
        if heartbeat_interval.is_zero() {
            return Err(ArtifactNodeAgentError::InvalidConfiguration(
                "artifact node heartbeat interval must be positive".to_string(),
            ));
        }
        Ok(Self {
            controller,
            materializer,
            identity,
            heartbeat_interval,
        })
    }

    /// Claims and handles at most one exact owner assignment. `Ok(false)`
    /// means the authenticated controller currently has no assignment for this
    /// node; it does not imply the node is globally healthy.
    pub async fn run_once(&self) -> Result<bool, ArtifactNodeAgentError> {
        let Some(work) = self
            .controller
            .claim_assignment()
            .await
            .map_err(ArtifactNodeAgentError::Controller)?
        else {
            return Ok(false);
        };
        if work.assignment.node_id != self.identity.node_id {
            return Err(ArtifactNodeAgentError::AssignmentIdentityMismatch);
        }
        self.run_claimed_assignment(work).await?;
        Ok(true)
    }

    pub async fn run_until_shutdown(&self, poll_interval: Duration) {
        let shutdown = rustok_worker_transport::shutdown_signal();
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _ = &mut shutdown => return,
                result = self.run_once() => match result {
                    Ok(true) => {}
                    Ok(false) => {
                        if wait_for_poll_or_shutdown(&mut shutdown, poll_interval).await {
                            return;
                        }
                    }
                    Err(error) => {
                        eprintln!("artifact node agent iteration failed: {error}");
                        if wait_for_poll_or_shutdown(&mut shutdown, poll_interval).await {
                            return;
                        }
                    }
                }
            }
        }
    }

    async fn run_claimed_assignment(
        &self,
        work: ModuleArtifactNodeAssignmentWorkItem,
    ) -> Result<(), ArtifactNodeAgentError> {
        let (stop_heartbeats, heartbeat_stop) = watch::channel(false);
        let heartbeat = tokio::spawn(maintain_assignment_lease(
            Arc::clone(&self.controller),
            work.claim_id,
            self.heartbeat_interval,
            heartbeat_stop,
        ));
        let result = self.process_assignment(&work).await;
        let _ = stop_heartbeats.send(true);
        let heartbeat_result = heartbeat
            .await
            .map_err(|error| ArtifactNodeAgentError::Controller(error.to_string()))?;
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                if let Err(heartbeat_error) = heartbeat_result {
                    return Err(ArtifactNodeAgentError::Controller(heartbeat_error));
                }
                Err(error)
            }
        }
    }

    async fn process_assignment(
        &self,
        work: &ModuleArtifactNodeAssignmentWorkItem,
    ) -> Result<(), ArtifactNodeAgentError> {
        match work.assignment.phase {
            ModuleReconciliationPhase::Pending => match self.materializer.prepare(work).await {
                Ok(_) => {
                    self.report(work, ModuleReconciliationPhase::Prepared, None, None)
                        .await
                }
                Err(ArtifactNodeMaterializationError::Retryable { detail }) => {
                    Err(ArtifactNodeAgentError::Retryable(detail))
                }
                Err(ArtifactNodeMaterializationError::Terminal { code, detail }) => {
                    self.report(
                        work,
                        ModuleReconciliationPhase::Failed,
                        None,
                        Some(ModuleReconciliationFailure { code, detail }),
                    )
                    .await
                }
            },
            ModuleReconciliationPhase::Prepared => {
                match self.materializer.verify_ready(work).await {
                    Ok(preparation) => {
                        self.report(
                            work,
                            ModuleReconciliationPhase::Healthy,
                            Some(health_evidence(work, &preparation)?),
                            None,
                        )
                        .await
                    }
                    Err(ArtifactNodeMaterializationError::Retryable { detail }) => {
                        Err(ArtifactNodeAgentError::Retryable(detail))
                    }
                    Err(ArtifactNodeMaterializationError::Terminal { code, detail }) => {
                        self.report(
                            work,
                            ModuleReconciliationPhase::Failed,
                            None,
                            Some(ModuleReconciliationFailure { code, detail }),
                        )
                        .await
                    }
                }
            }
            _ => Err(ArtifactNodeAgentError::UnexpectedAssignmentPhase),
        }
    }

    async fn report(
        &self,
        work: &ModuleArtifactNodeAssignmentWorkItem,
        phase: ModuleReconciliationPhase,
        health_evidence: Option<ModuleReconciliationEvidence>,
        failure: Option<ModuleReconciliationFailure>,
    ) -> Result<(), ArtifactNodeAgentError> {
        let report = ModuleArtifactNodeAssignmentReport {
            claim_id: work.claim_id,
            reconciliation_id: work.reconciliation.reconciliation_id,
            node_id: work.assignment.node_id,
            installation_id: work.assignment.installation_id,
            expected_observation_revision: work.expected_observation_revision,
            phase,
            installation_scope: work.assignment.installation_scope,
            release_digest: work.assignment.release_digest.clone(),
            payload_digest: work.assignment.payload_digest.clone(),
            payload_kind: work.assignment.payload_kind,
            payload_media_type: work.assignment.payload_media_type.clone(),
            admission_revision: work.assignment.admission_revision,
            dependency_graph_revision: work.assignment.dependency_graph_revision,
            dependency_graph_digest: work.assignment.dependency_graph_digest.clone(),
            capability_grant_revision: work.assignment.capability_grant_revision,
            executor_abi: work.assignment.executor_abi.clone(),
            policy_revision: work.assignment.policy_revision.clone(),
            health_evidence,
            failure,
            agent_id: self.identity.agent_id.clone(),
            idempotency_key: Uuid::new_v4(),
        };
        self.controller
            .report_assignment(&report)
            .await
            .map_err(ArtifactNodeAgentError::Controller)?;
        Ok(())
    }
}

async fn wait_for_poll_or_shutdown<F>(
    shutdown: &mut std::pin::Pin<&mut F>,
    poll_interval: Duration,
) -> bool
where
    F: Future<Output = ()>,
{
    tokio::select! {
        _ = shutdown.as_mut() => true,
        _ = tokio::time::sleep(poll_interval) => false,
    }
}

async fn maintain_assignment_lease<C>(
    controller: Arc<C>,
    claim_id: Uuid,
    heartbeat_interval: Duration,
    mut stop: watch::Receiver<bool>,
) -> Result<(), String>
where
    C: ArtifactNodeAssignmentController + 'static,
{
    let mut interval = tokio::time::interval(heartbeat_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;
    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return Ok(());
                }
            }
            _ = interval.tick() => {
                let _ = controller.heartbeat_assignment(claim_id).await?;
            }
        }
    }
}

#[derive(Serialize)]
struct HealthEvidenceInput<'a> {
    contract: &'static str,
    reconciliation_id: Uuid,
    reconciliation_revision: u64,
    claim_id: Uuid,
    node_id: Uuid,
    installation_id: Uuid,
    installation_scope: rustok_modules::ModuleArtifactNodeInstallationScope,
    release_digest: &'a str,
    payload_digest: &'a str,
    payload_kind: rustok_modules::ArtifactPayloadKind,
    payload_media_type: &'a str,
    admission_revision: u64,
    dependency_graph_revision: u64,
    dependency_graph_digest: &'a str,
    capability_grant_revision: u64,
    executor_abi: &'a str,
    policy_revision: &'a str,
    runtime_fingerprint: &'a str,
}

fn health_evidence(
    work: &ModuleArtifactNodeAssignmentWorkItem,
    preparation: &NodeArtifactPreparation,
) -> Result<ModuleReconciliationEvidence, ArtifactNodeAgentError> {
    let evidence = HealthEvidenceInput {
        contract: HEALTH_EVIDENCE_CONTRACT,
        reconciliation_id: work.reconciliation.reconciliation_id,
        reconciliation_revision: work.reconciliation.reconciliation_revision,
        claim_id: work.claim_id,
        node_id: work.assignment.node_id,
        installation_id: work.assignment.installation_id,
        installation_scope: work.assignment.installation_scope,
        release_digest: &work.assignment.release_digest,
        payload_digest: &work.assignment.payload_digest,
        payload_kind: work.assignment.payload_kind,
        payload_media_type: &work.assignment.payload_media_type,
        admission_revision: work.assignment.admission_revision,
        dependency_graph_revision: work.assignment.dependency_graph_revision,
        dependency_graph_digest: &work.assignment.dependency_graph_digest,
        capability_grant_revision: work.assignment.capability_grant_revision,
        executor_abi: &work.assignment.executor_abi,
        policy_revision: &work.assignment.policy_revision,
        runtime_fingerprint: &preparation.runtime_fingerprint,
    };
    let encoded =
        serde_json::to_vec(&evidence).map_err(|_| ArtifactNodeAgentError::EvidenceEncoding)?;
    Ok(ModuleReconciliationEvidence {
        reference: format!(
            "artifact-node-agent:{}:{}",
            work.assignment.node_id, work.claim_id
        ),
        digest: format!("sha256:{}", hex::encode(Sha256::digest(encoded))),
    })
}

fn bounded_failure_code(value: String) -> String {
    if !value.is_empty()
        && value.len() <= MAX_FAILURE_CODE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        value
    } else {
        "artifact_node_agent_failure".to_string()
    }
}

fn bounded_detail(value: String) -> String {
    let mut detail: String = value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_FAILURE_DETAIL_BYTES)
        .collect();
    if detail.trim().is_empty() {
        detail = "artifact node agent encountered an unspecified failure".to_string();
    }
    detail
}

#[derive(Debug, Error)]
pub enum ArtifactNodeAgentError {
    #[error("artifact node agent configuration is invalid: {0}")]
    InvalidConfiguration(String),
    #[error("artifact node controller request failed: {0}")]
    Controller(String),
    #[error("owner assignment node does not match the authenticated agent identity")]
    AssignmentIdentityMismatch,
    #[error("owner assignment is not in an agent-actionable phase")]
    UnexpectedAssignmentPhase,
    #[error("artifact node preparation must be retried: {0}")]
    Retryable(String),
    #[error("artifact node health evidence could not be encoded")]
    EvidenceEncoding,
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use async_trait::async_trait;
    use chrono::Utc;
    use rustok_artifact_node_transport::ModuleArtifactNodeAgentIdentity;
    use rustok_modules::{
        ArtifactPayloadKind, ModuleArtifactNodeAssignment,
        ModuleArtifactNodeAssignmentHeartbeatReceipt, ModuleArtifactNodeAssignmentReport,
        ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeAssignmentWorkItem,
        ModuleArtifactNodeInstallationScope, ModuleArtifactNodeReconciliationStatus,
        ModuleArtifactNodeReconciliationWorkIdentity, ModuleReconciliationPhase,
    };

    use super::{
        ArtifactNodeAgent, ArtifactNodeAssignmentController, ArtifactNodeMaterializationError,
        ArtifactNodeMaterializer, NodeArtifactPreparation,
    };

    struct RecordingController {
        claims: Mutex<VecDeque<ModuleArtifactNodeAssignmentWorkItem>>,
        reports: Mutex<Vec<ModuleArtifactNodeAssignmentReport>>,
        heartbeats: Mutex<Vec<uuid::Uuid>>,
    }

    #[async_trait]
    impl ArtifactNodeAssignmentController for RecordingController {
        async fn claim_assignment(
            &self,
        ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, String> {
            Ok(self.claims.lock().expect("claims lock").pop_front())
        }

        async fn heartbeat_assignment(
            &self,
            claim_id: uuid::Uuid,
        ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, String> {
            self.heartbeats
                .lock()
                .expect("heartbeats lock")
                .push(claim_id);
            Ok(ModuleArtifactNodeAssignmentHeartbeatReceipt {
                claim_id,
                lease_expires_at: Utc::now(),
            })
        }

        async fn report_assignment(
            &self,
            report: &ModuleArtifactNodeAssignmentReport,
        ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, String> {
            self.reports
                .lock()
                .expect("reports lock")
                .push(report.clone());
            Ok(ModuleArtifactNodeAssignmentReportReceipt {
                reconciliation_id: report.reconciliation_id,
                reconciliation_revision: 1,
                reconciliation_state_revision: 1,
                reconciliation_status: ModuleArtifactNodeReconciliationStatus::Preparing,
                node_id: report.node_id,
                installation_id: report.installation_id,
                observation_revision: report.expected_observation_revision + 1,
                phase: report.phase,
                created: true,
            })
        }
    }

    struct RecordingMaterializer {
        prepare: Result<NodeArtifactPreparation, ArtifactNodeMaterializationError>,
        ready: Result<NodeArtifactPreparation, ArtifactNodeMaterializationError>,
        delay: Option<std::time::Duration>,
    }

    #[async_trait]
    impl ArtifactNodeMaterializer for RecordingMaterializer {
        async fn prepare(
            &self,
            _work: &ModuleArtifactNodeAssignmentWorkItem,
        ) -> Result<NodeArtifactPreparation, ArtifactNodeMaterializationError> {
            if let Some(delay) = self.delay {
                tokio::time::sleep(delay).await;
            }
            self.prepare.clone()
        }

        async fn verify_ready(
            &self,
            _work: &ModuleArtifactNodeAssignmentWorkItem,
        ) -> Result<NodeArtifactPreparation, ArtifactNodeMaterializationError> {
            self.ready.clone()
        }
    }

    fn identity() -> ModuleArtifactNodeAgentIdentity {
        ModuleArtifactNodeAgentIdentity::new(uuid::Uuid::new_v4(), "node-agent-a")
            .expect("identity")
    }

    fn work(
        node_id: uuid::Uuid,
        phase: ModuleReconciliationPhase,
        observation_revision: u64,
    ) -> ModuleArtifactNodeAssignmentWorkItem {
        ModuleArtifactNodeAssignmentWorkItem {
            claim_id: uuid::Uuid::new_v4(),
            lease_expires_at: Utc::now(),
            expected_observation_revision: observation_revision,
            reconciliation: ModuleArtifactNodeReconciliationWorkIdentity {
                reconciliation_id: uuid::Uuid::new_v4(),
                reconciliation_revision: 1,
                topology_reference: "deployment:node-a".to_string(),
                topology_digest: format!("sha256:{}", "a".repeat(64)),
                policy_revision: format!("sha256:{}", "b".repeat(64)),
            },
            assignment: ModuleArtifactNodeAssignment {
                node_id,
                installation_id: uuid::Uuid::new_v4(),
                installation_scope: ModuleArtifactNodeInstallationScope::Platform,
                release_digest: format!("sha256:{}", "c".repeat(64)),
                payload_digest: format!("sha256:{}", "d".repeat(64)),
                payload_kind: ArtifactPayloadKind::Rhai,
                payload_media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
                admission_revision: 1,
                dependency_graph_revision: 1,
                dependency_graph_digest: format!("sha256:{}", "e".repeat(64)),
                capability_grant_revision: 1,
                executor_abi: "rustok:module/runtime@1".to_string(),
                policy_revision: format!("sha256:{}", "b".repeat(64)),
                ordinal: 0,
                observation_revision,
                phase,
                health_evidence: None,
                failure: None,
                reported_by: None,
                last_report_digest: None,
                active_claim_id: None,
                claimed_by_agent: None,
                claim_expires_at: None,
            },
        }
    }

    fn successful_materializer() -> RecordingMaterializer {
        let preparation = NodeArtifactPreparation {
            runtime_fingerprint: format!("sha256:{}", "f".repeat(64)),
        };
        RecordingMaterializer {
            prepare: Ok(preparation.clone()),
            ready: Ok(preparation),
            delay: None,
        }
    }

    #[tokio::test]
    async fn agent_reports_prepared_then_exact_healthy_evidence() {
        let identity = identity();
        let controller = std::sync::Arc::new(RecordingController {
            claims: Mutex::new(VecDeque::from([
                work(identity.node_id, ModuleReconciliationPhase::Pending, 0),
                work(identity.node_id, ModuleReconciliationPhase::Prepared, 1),
            ])),
            reports: Mutex::new(Vec::new()),
            heartbeats: Mutex::new(Vec::new()),
        });
        let agent = ArtifactNodeAgent::new(
            controller.clone(),
            std::sync::Arc::new(successful_materializer()),
            identity,
            std::time::Duration::from_secs(1),
        )
        .expect("agent");

        assert!(agent.run_once().await.expect("prepared iteration"));
        assert!(agent.run_once().await.expect("healthy iteration"));
        let reports = controller.reports.lock().expect("reports lock");
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].phase, ModuleReconciliationPhase::Prepared);
        assert!(reports[0].health_evidence.is_none());
        assert_eq!(reports[1].phase, ModuleReconciliationPhase::Healthy);
        assert!(reports[1].health_evidence.is_some());
    }

    #[tokio::test]
    async fn terminal_preparation_failure_reports_failed_without_health_evidence() {
        let identity = identity();
        let controller = std::sync::Arc::new(RecordingController {
            claims: Mutex::new(VecDeque::from([work(
                identity.node_id,
                ModuleReconciliationPhase::Pending,
                0,
            )])),
            reports: Mutex::new(Vec::new()),
            heartbeats: Mutex::new(Vec::new()),
        });
        let materializer = RecordingMaterializer {
            prepare: Err(ArtifactNodeMaterializationError::terminal(
                "payload_invalid",
                "the immutable payload is invalid",
            )),
            ready: Ok(NodeArtifactPreparation {
                runtime_fingerprint: format!("sha256:{}", "f".repeat(64)),
            }),
            delay: None,
        };
        let agent = ArtifactNodeAgent::new(
            controller.clone(),
            std::sync::Arc::new(materializer),
            identity,
            std::time::Duration::from_secs(1),
        )
        .expect("agent");

        assert!(agent.run_once().await.expect("failed report"));
        let reports = controller.reports.lock().expect("reports lock");
        assert_eq!(reports[0].phase, ModuleReconciliationPhase::Failed);
        assert!(reports[0].health_evidence.is_none());
        assert_eq!(
            reports[0].failure.as_ref().expect("failure").code,
            "payload_invalid"
        );
    }

    #[tokio::test]
    async fn retryable_preparation_failure_never_publishes_a_terminal_or_healthy_report() {
        let identity = identity();
        let controller = std::sync::Arc::new(RecordingController {
            claims: Mutex::new(VecDeque::from([work(
                identity.node_id,
                ModuleReconciliationPhase::Pending,
                0,
            )])),
            reports: Mutex::new(Vec::new()),
            heartbeats: Mutex::new(Vec::new()),
        });
        let materializer = RecordingMaterializer {
            prepare: Err(ArtifactNodeMaterializationError::retryable(
                "durable CAS is unavailable",
            )),
            ready: Ok(NodeArtifactPreparation {
                runtime_fingerprint: format!("sha256:{}", "f".repeat(64)),
            }),
            delay: None,
        };
        let agent = ArtifactNodeAgent::new(
            controller.clone(),
            std::sync::Arc::new(materializer),
            identity,
            std::time::Duration::from_secs(1),
        )
        .expect("agent");

        assert!(matches!(
            agent.run_once().await,
            Err(super::ArtifactNodeAgentError::Retryable(_))
        ));
        assert!(controller.reports.lock().expect("reports lock").is_empty());
    }

    #[tokio::test]
    async fn active_lease_is_heartbeated_while_preparation_runs() {
        let identity = identity();
        let controller = std::sync::Arc::new(RecordingController {
            claims: Mutex::new(VecDeque::from([work(
                identity.node_id,
                ModuleReconciliationPhase::Pending,
                0,
            )])),
            reports: Mutex::new(Vec::new()),
            heartbeats: Mutex::new(Vec::new()),
        });
        let mut materializer = successful_materializer();
        materializer.delay = Some(std::time::Duration::from_millis(25));
        let agent = ArtifactNodeAgent::new(
            controller.clone(),
            std::sync::Arc::new(materializer),
            identity,
            std::time::Duration::from_millis(5),
        )
        .expect("agent");

        agent.run_once().await.expect("prepared report");
        assert!(
            !controller
                .heartbeats
                .lock()
                .expect("heartbeats lock")
                .is_empty()
        );
    }
}
