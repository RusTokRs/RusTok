use std::sync::Arc;

use async_trait::async_trait;
use rustok_notifications_api::{PortContext, PortError};
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
    pub fn new(policy: Arc<dyn NotificationRecipientPolicy>, relation_ports_ready: bool) -> Self {
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

    pub fn policy_arc(&self) -> Arc<dyn NotificationRecipientPolicy> {
        self.policy.clone()
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

impl NotificationRelationPolicyRequest {
    pub fn validate(&self) -> Result<(), crate::error::NotificationError> {
        if self.tenant_id.is_nil() || self.recipient_id.is_nil() || self.actor_id.is_nil() {
            return Err(crate::error::NotificationError::Validation(
                "relation policy identities must not be nil".to_string(),
            ));
        }
        if self.source_slug.trim().is_empty() || self.notification_type.trim().is_empty() {
            return Err(crate::error::NotificationError::Validation(
                "relation policy source slug and notification type must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_policy_request_validation_rejects_nil_and_empty() {
        let valid = NotificationRelationPolicyRequest {
            tenant_id: Uuid::new_v4(),
            recipient_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            source_slug: "blog".to_string(),
            notification_type: "post.published".to_string(),
        };
        assert!(valid.validate().is_ok());

        let mut nil_tenant = valid.clone();
        nil_tenant.tenant_id = Uuid::nil();
        assert!(nil_tenant.validate().is_err());

        let mut nil_recipient = valid.clone();
        nil_recipient.recipient_id = Uuid::nil();
        assert!(nil_recipient.validate().is_err());

        let mut nil_actor = valid.clone();
        nil_actor.actor_id = Uuid::nil();
        assert!(nil_actor.validate().is_err());

        let mut empty_slug = valid.clone();
        empty_slug.source_slug = "   ".to_string();
        assert!(empty_slug.validate().is_err());

        let mut empty_type = valid.clone();
        empty_type.notification_type = "".to_string();
        assert!(empty_type.validate().is_err());
    }

    struct MockPolicy;

    #[async_trait]
    impl NotificationRecipientPolicy for MockPolicy {
        async fn evaluate(
            &self,
            _request: crate::candidate::NotificationRecipientPolicyRequest,
        ) -> Result<
            crate::candidate::NotificationRecipientPolicyDecision,
            crate::candidate::NotificationRecipientPolicyError,
        > {
            Ok(crate::candidate::NotificationRecipientPolicyDecision::Allow)
        }
    }

    #[test]
    fn recipient_policy_runtime_ready_flag_requires_both_ports_and_enabled() {
        let runtime = NotificationRecipientPolicyRuntime::new(Arc::new(MockPolicy), false);
        assert!(!runtime.candidate_worker_ready());

        let runtime = NotificationRecipientPolicyRuntime::new(Arc::new(MockPolicy), true);
        assert!(!runtime.candidate_worker_ready());

        let runtime = runtime.with_candidate_worker_enabled(true);
        assert!(runtime.candidate_worker_ready());
    }
}
