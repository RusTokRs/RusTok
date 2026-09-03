use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, TransactionTrait};
use serde_json::{json, Value};

use rustok_events::DomainEvent;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    SandboxError, SandboxResult,
};

use crate::artifact_capability_router::{
    resolve_granted_artifact_capability, ArtifactCapabilityBrokerResolver,
    ArtifactCapabilityExecution,
};
use crate::ControlPlaneInfrastructure;

/// Production broker for the `platform.events` sandbox capability.
///
/// Publishes domain events to the platform's transactional outbox within
/// the bounds enforced by `EventCapabilityConstraints`.
#[derive(Clone)]
pub struct ArtifactEventCapabilityBroker {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
    module_slug: String,
}

impl ArtifactEventCapabilityBroker {
    pub fn new(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
        module_slug: String,
    ) -> Self {
        Self {
            db,
            infrastructure,
            module_slug,
        }
    }
}

#[async_trait]
impl CapabilityBroker for ArtifactEventCapabilityBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        let input = call
            .input
            .as_object()
            .ok_or_else(|| SandboxError::InvalidRequest("Event input must be an object".into()))?;

        let topic = input
            .get("topic")
            .and_then(Value::as_str)
            .ok_or_else(|| SandboxError::InvalidRequest("Event topic is required".into()))?;

        let payload = input.get("payload").cloned().unwrap_or(Value::Null);

        let tenant_id = call.context.tenant_id;

        let domain_event = DomainEvent::ModuleGuestEventEmitted {
            module_slug: self.module_slug.clone(),
            topic: topic.to_string(),
            payload,
        };

        let envelope = self.infrastructure.event_envelope(tenant_id, None, domain_event);
        let event_id = envelope.id;

        let transaction = self.db.begin().await.map_err(|err| {
            SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("failed to begin event transaction: {err}"),
            }
        })?;

        self.infrastructure
            .write_event(&transaction, envelope)
            .await
            .map_err(|err| SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("failed to write event to outbox: {err}"),
            })?;

        transaction.commit().await.map_err(|err| {
            SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("failed to commit event transaction: {err}"),
            }
        })?;

        Ok(CapabilityResponse {
            output: json!({
                "published": true,
                "topic": topic,
                "event_id": event_id,
            }),
        })
    }
}

/// Deployment-owned resolver for the `platform.events` capability broker.
#[derive(Clone)]
pub struct SeaOrmArtifactEventCapabilityBrokerResolver {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
}

impl SeaOrmArtifactEventCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_infrastructure(db, ControlPlaneInfrastructure::default())
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure }
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for SeaOrmArtifactEventCapabilityBrokerResolver {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.events" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        Ok(Arc::new(ArtifactEventCapabilityBroker::new(
            self.db.clone(),
            self.infrastructure.clone(),
            execution.slug.clone(),
        )))
    }
}
