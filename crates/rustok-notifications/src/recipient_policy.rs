use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::NotificationRecipientPolicy;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationRelationPolicyRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub actor_id: Uuid,
    pub source_slug: String,
    pub notification_type: String,
}

#[async_trait]
pub trait NotificationBlockReadPort: Send + Sync {
    /// Returns true when either owner-defined blocking direction suppresses
    /// delivery between the actor and recipient.
    async fn blocks_notification(
        &self,
        context: PortContext,
        request: NotificationRelationPolicyRequest,
    ) -> Result<bool, PortError>;
}

#[async_trait]
pub trait NotificationMuteReadPort: Send + Sync {
    /// Returns true when the recipient currently mutes this actor or source
    /// according to the relation owner's policy.
    async fn mutes_notification(
        &self,
        context: PortContext,
        request: NotificationRelationPolicyRequest,
    ) -> Result<bool, PortError>;
}

#[derive(Clone)]
pub struct NotificationBlockReadRuntime {
    port: Arc<dyn NotificationBlockReadPort>,
}

impl NotificationBlockReadRuntime {
    pub fn new(port: Arc<dyn NotificationBlockReadPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn NotificationBlockReadPort {
        self.port.as_ref()
    }
}

#[derive(Clone)]
pub struct NotificationMuteReadRuntime {
    port: Arc<dyn NotificationMuteReadPort>,
}

impl NotificationMuteReadRuntime {
    pub fn new(port: Arc<dyn NotificationMuteReadPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn NotificationMuteReadPort {
        self.port.as_ref()
    }
}

#[derive(Clone)]
pub struct NotificationRecipientPolicyRuntime {
    policy: Arc<dyn NotificationRecipientPolicy>,
    relation_ports_ready: bool,
    candidate_worker_enabled: bool,
}

impl NotificationRecipientPolicyRuntime {
    pub fn new(
        policy: Arc<dyn NotificationRecipientPolicy>,
        relation_ports_ready: bool,
    ) -> Self {
        Self {
            policy,
            relation_ports_ready,
            candidate_worker_enabled: false,
        }
    }

    pub fn with_candidate_worker_enabled(mut self, enabled: bool) -> Self {
        self.candidate_worker_enabled = enabled;
        self
    }

    pub fn policy(&self) -> &dyn NotificationRecipientPolicy {
        self.policy.as_ref()
    }

    pub const fn relation_ports_ready(&self) -> bool {
        self.relation_ports_ready
    }

    pub const fn candidate_worker_enabled(&self) -> bool {
        self.candidate_worker_enabled
    }

    pub const fn candidate_worker_ready(&self) -> bool {
        self.relation_ports_ready && self.candidate_worker_enabled
    }
}
