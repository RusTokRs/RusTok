use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ProfileStatus, ProfileVisibility, entities};

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

/// Owner-local read adapter for privacy decisions.
///
/// Privacy evaluation deliberately reads only the base `profiles` row. It must
/// not depend on localized presentation copy, taxonomy labels, or media joins.
#[derive(Clone, Debug)]
pub struct ProfilePrivacyService {
    db: DatabaseConnection,
}

impl ProfilePrivacyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn find_state(
        &self,
        tenant_id: Uuid,
        recipient_id: Uuid,
    ) -> Result<Option<ProfilePrivacyState>, PortError> {
        let profile = entities::profile::Entity::find_by_id(recipient_id)
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    recipient_id = %recipient_id,
                    error = %error,
                    "Profile privacy state read failed"
                );
                PortError::unavailable(
                    "profiles.privacy_read_unavailable",
                    "profile privacy state is temporarily unavailable",
                )
            })?;

        Ok(profile.map(|profile| ProfilePrivacyState {
            status: profile.status,
            visibility: profile.visibility,
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProfilePrivacyState {
    status: ProfileStatus,
    visibility: ProfileVisibility,
}

#[async_trait]
impl ProfilePrivacyReadPort for ProfilePrivacyService {
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

        let Some(state) = self.find_state(tenant_id, request.recipient_id).await? else {
            return Ok(ProfilePrivacyDecision::RecipientUnavailable);
        };

        if state.status != ProfileStatus::Active {
            return Ok(ProfilePrivacyDecision::RecipientUnavailable);
        }

        if request.actor_id == Some(request.recipient_id) {
            return Ok(ProfilePrivacyDecision::Allow);
        }

        match state.visibility {
            ProfileVisibility::Public | ProfileVisibility::Authenticated => {
                Ok(ProfilePrivacyDecision::Allow)
            }
            ProfileVisibility::FollowersOnly | ProfileVisibility::Private => {
                Ok(ProfilePrivacyDecision::Restricted)
            }
        }
    }
}
