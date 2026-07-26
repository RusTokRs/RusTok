use std::time::Instant;

use rustok_api::PortError;
use uuid::Uuid;

use crate::SocialRelationKind;

pub const SOCIAL_GRAPH_OPERATION_TARGET: &str = "rustok_social_graph::operations";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocialGraphCommandOperation {
    Block,
    Unblock,
    Mute,
    Unmute,
    Follow,
    Unfollow,
}

impl SocialGraphCommandOperation {
    pub const fn from_relation_state(relation_kind: SocialRelationKind, active: bool) -> Self {
        match (relation_kind, active) {
            (SocialRelationKind::Block, true) => Self::Block,
            (SocialRelationKind::Block, false) => Self::Unblock,
            (SocialRelationKind::Mute, true) => Self::Mute,
            (SocialRelationKind::Mute, false) => Self::Unmute,
            (SocialRelationKind::Follow, true) => Self::Follow,
            (SocialRelationKind::Follow, false) => Self::Unfollow,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "social_graph.block",
            Self::Unblock => "social_graph.unblock",
            Self::Mute => "social_graph.mute",
            Self::Unmute => "social_graph.unmute",
            Self::Follow => "social_graph.follow",
            Self::Unfollow => "social_graph.unfollow",
        }
    }
}

#[derive(Debug)]
pub struct SocialGraphCommandTimer {
    operation: SocialGraphCommandOperation,
    tenant_id: Uuid,
    source_user_id: Uuid,
    target_user_id: Uuid,
    started_at: Instant,
}

impl SocialGraphCommandTimer {
    pub fn start(
        operation: SocialGraphCommandOperation,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
    ) -> Self {
        Self {
            operation,
            tenant_id,
            source_user_id,
            target_user_id,
            started_at: Instant::now(),
        }
    }

    pub fn finish_port_result<T>(self, result: &Result<T, PortError>) {
        match result {
            Ok(_) => self.finish_success(),
            Err(error) => self.finish_failure(&error.code, error.retryable),
        }
    }

    pub fn finish_success(self) {
        tracing::info!(
            target: SOCIAL_GRAPH_OPERATION_TARGET,
            operation = self.operation.as_str(),
            tenant_id = %self.tenant_id,
            source_user_id = %self.source_user_id,
            target_user_id = %self.target_user_id,
            outcome = "success",
            duration_ms = self.started_at.elapsed().as_millis() as u64,
            "Social Graph owner command completed"
        );
    }

    pub fn finish_failure(self, error_code: &str, retryable: bool) {
        tracing::warn!(
            target: SOCIAL_GRAPH_OPERATION_TARGET,
            operation = self.operation.as_str(),
            tenant_id = %self.tenant_id,
            source_user_id = %self.source_user_id,
            target_user_id = %self.target_user_id,
            outcome = "failure",
            error_code,
            retryable,
            duration_ms = self.started_at.elapsed().as_millis() as u64,
            "Social Graph owner command failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::SocialGraphCommandOperation;
    use crate::SocialRelationKind;

    #[test]
    fn operation_names_are_stable_and_relation_scoped() {
        assert_eq!(
            SocialGraphCommandOperation::from_relation_state(SocialRelationKind::Follow, true)
                .as_str(),
            "social_graph.follow"
        );
        assert_eq!(
            SocialGraphCommandOperation::from_relation_state(SocialRelationKind::Follow, false)
                .as_str(),
            "social_graph.unfollow"
        );
        assert_eq!(
            SocialGraphCommandOperation::from_relation_state(SocialRelationKind::Block, false)
                .as_str(),
            "social_graph.unblock"
        );
    }
}
