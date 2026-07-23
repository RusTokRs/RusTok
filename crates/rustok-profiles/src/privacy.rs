use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{ProfileStatus, ProfileVisibility};
use crate::entities::profile;
use crate::ProfileService;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfilePrivacyReadRequest {
    pub recipient_id: Uuid,
    pub actor_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilePrivacyDecision {
    Allow,
    RecipientUnavailable,
    Restricted,
}

#[async_trait]
pub trait ProfilePrivacyReadPort: Send + Sync {
    async fn evaluate_profile_privacy(
        &self,
        context: PortContext,
        request: ProfilePrivacyReadRequest,
    ) -> Result<ProfilePrivacyDecision, PortError>;
}

#[derive(Clone)]
pub struct ProfilePrivacyRuntime {
    port: Arc<dyn ProfilePrivacyReadPort>,
}

impl ProfilePrivacyRuntime {
    pub fn new(port: Arc<dyn ProfilePrivacyReadPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn ProfilePrivacyReadPort {
        self.port.as_ref()
    }
}

#[async_trait]
impl ProfilePrivacyReadPort for ProfileService {
    async fn evaluate_profile_privacy(
        &self,
        context: PortContext,
        request: ProfilePrivacyReadRequest,
    ) -> Result<ProfilePrivacyDecision, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            PortError::validation(
                "profiles.tenant_id_invalid",
                "profile privacy reads require a valid tenant identifier",
            )
        })?;

        let model = profile::Entity::find_by_id(request.recipient_id)
            .filter(profile::Column::TenantId.eq(tenant_id))
            .one(self.db())
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "profiles.privacy_read_unavailable",
                    "profile privacy state is temporarily unavailable",
                )
            })?;

        let Some(model) = model else {
            return Ok(ProfilePrivacyDecision::RecipientUnavailable);
        };

        if model.status != ProfileStatus::Active {
            return Ok(ProfilePrivacyDecision::RecipientUnavailable);
        }

        if request.actor_id == Some(request.recipient_id) {
            return Ok(ProfilePrivacyDecision::Allow);
        }

        match model.visibility {
            ProfileVisibility::Public | ProfileVisibility::Authenticated => {
                Ok(ProfilePrivacyDecision::Allow)
            }
            ProfileVisibility::FollowersOnly | ProfileVisibility::Private => {
                Ok(ProfilePrivacyDecision::Restricted)
            }
        }
    }
}
