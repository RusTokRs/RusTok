use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use rustok_events::DomainEvent;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    SandboxError, SandboxResult,
};

use crate::ControlPlaneInfrastructure;
use crate::artifact_capability_router::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution,
    resolve_granted_artifact_capability,
};

/// Production broker for the `platform.events` sandbox capability.
///
/// Publishes domain events to the platform's transactional outbox within
/// the bounds enforced by `EventCapabilityConstraints`.
#[derive(Clone)]
pub struct ArtifactEventCapabilityBroker {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
    module_slug: String,
    scope: Option<crate::ArtifactCapabilityScope>,
    effect_authorizer: Option<Arc<dyn crate::ArtifactSqlEffectAuthorizer>>,
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
            scope: None,
            effect_authorizer: None,
        }
    }
}

#[async_trait]
impl CapabilityBroker for ArtifactEventCapabilityBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if call.capability.as_str() != "platform.events" || grant.name != call.capability
            || call.context.tenant_id.is_none_or(|tenant| tenant.is_nil())
            || !matches!(&call.subject, rustok_sandbox::SandboxSubject::ModuleArtifact { slug, .. } if slug == &self.module_slug)
            || self.scope.as_ref().is_some_and(|scope| {
                !scope.matches_subject(&call.subject) || call.context.tenant_id != Some(scope.tenant_id)
            })
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
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

        let envelope = self
            .infrastructure
            .event_envelope(tenant_id, None, domain_event);
        let event_id = envelope.id;

        let fence = match self.effect_authorizer.as_ref() {
            Some(authorizer) => Some(authorizer.prepare(call).await?),
            None => None,
        };
        let transaction = crate::backend::begin_fenced_transaction(&self.db)
            .await
            .map_err(|err| SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("failed to begin event transaction: {err}"),
            })?;

        match tenant_id {
            Some(tenant_id) => crate::backend::set_tenant_scope(&transaction, tenant_id).await,
            None => crate::backend::set_platform_scope(&transaction).await,
        }
        .map_err(|err| SandboxError::HostCapability {
            capability: call.capability.clone(),
            message: format!("failed to scope event transaction: {err}"),
        })?;

        let guard = match fence.as_ref() {
            Some(fence) => Some(fence.lock_on(&transaction).await?),
            None => None,
        };
        self.infrastructure
            .write_event(&transaction, envelope)
            .await
            .map_err(|err| SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("failed to write event to outbox: {err}"),
            })?;

        if let Some(guard) = guard.as_ref() {
            guard.check_before_commit(&transaction).await?;
        }
        transaction
            .commit()
            .await
            .map_err(|err| SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("failed to commit event transaction: {err}"),
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
    effect_authorizer: Option<Arc<dyn crate::ArtifactSqlEffectAuthorizer>>,
}

impl SeaOrmArtifactEventCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_infrastructure(db, ControlPlaneInfrastructure::default())
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure, effect_authorizer: None }
    }

    pub fn with_sql_effect_authorizer(mut self, authorizer: Arc<dyn crate::ArtifactSqlEffectAuthorizer>) -> Self {
        self.effect_authorizer = Some(authorizer);
        self
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
        let installation = resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        let scope = crate::artifact_capability_router::artifact_capability_scope_for_execution(
            &installation, execution, capability,
        )?;
        let mut broker = ArtifactEventCapabilityBroker::new(
            self.db.clone(), self.infrastructure.clone(), execution.slug.clone(),
        );
        broker.scope = Some(scope);
        broker.effect_authorizer = self.effect_authorizer.clone();
        Ok(Arc::new(broker))
    }
}
