use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ports::{map_owner_error, parse_tenant_id, validate_source_actor};
use crate::{SocialGraphPairRequest, SocialGraphService, SocialRelationKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphFollowState {
    pub target_user_id: Uuid,
    pub following: bool,
    pub revision: Option<i64>,
}

#[async_trait]
pub trait SocialGraphFollowReadPort: Send + Sync {
    async fn source_follow_state(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<SocialGraphFollowState, PortError>;
}

#[async_trait]
impl SocialGraphFollowReadPort for SocialGraphService {
    async fn source_follow_state(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<SocialGraphFollowState, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        request.validate()?;
        validate_source_actor(&context, request.source_user_id)?;

        let relation = self
            .relation_state(
                parse_tenant_id(&context)?,
                request.source_user_id,
                request.target_user_id,
                SocialRelationKind::Follow,
            )
            .await
            .map_err(map_owner_error)?;

        Ok(SocialGraphFollowState {
            target_user_id: request.target_user_id,
            following: relation.as_ref().is_some_and(|relation| relation.active),
            revision: relation.map(|relation| relation.revision),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_state_construction() {
        let target = Uuid::new_v4();
        let state = SocialGraphFollowState {
            target_user_id: target,
            following: true,
            revision: Some(42),
        };
        assert_eq!(state.target_user_id, target);
        assert!(state.following);
        assert_eq!(state.revision, Some(42));
    }
}
