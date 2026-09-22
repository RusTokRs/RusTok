use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use rustok_api::{PortActor, PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_social_graph::{
    MAX_SOCIAL_GRAPH_FOLLOW_TARGETS, SocialGraphFollowBatchRequest, SocialGraphPrivacyReadPort,
    SocialGraphService,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ProfileStatus, ProfileVisibility, entities};

const PROFILE_FOLLOW_READ_DEADLINE: Duration = Duration::from_secs(2);

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

/// The caller class used to evaluate profile visibility without coupling the
/// policy to GraphQL, notifications, or any other transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileAccessAudience {
    Anonymous,
    Authenticated { actor_id: Uuid },
    TrustedService { actor_id: Option<Uuid> },
}

impl ProfileAccessAudience {
    fn actor_id(self) -> Option<Uuid> {
        match self {
            Self::Anonymous => None,
            Self::Authenticated { actor_id } => Some(actor_id),
            Self::TrustedService { actor_id } => actor_id,
        }
    }

    fn is_authenticated(self) -> bool {
        !matches!(self, Self::Anonymous)
    }
}

/// Evaluate the canonical active-profile visibility matrix.
///
/// `followers_only` remains `Restricted` in this pure base decision and is
/// upgraded only after the owner service resolves an active follow relation.
pub fn evaluate_profile_access(
    recipient_id: Uuid,
    status: ProfileStatus,
    visibility: ProfileVisibility,
    audience: ProfileAccessAudience,
) -> ProfilePrivacyDecision {
    if status != ProfileStatus::Active {
        return ProfilePrivacyDecision::RecipientUnavailable;
    }

    if audience.actor_id() == Some(recipient_id) {
        return ProfilePrivacyDecision::Allow;
    }

    match visibility {
        ProfileVisibility::Public => ProfilePrivacyDecision::Allow,
        ProfileVisibility::Authenticated if audience.is_authenticated() => {
            ProfilePrivacyDecision::Allow
        }
        ProfileVisibility::Authenticated
        | ProfileVisibility::FollowersOnly
        | ProfileVisibility::Private => ProfilePrivacyDecision::Restricted,
    }
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
/// Base profile state comes only from the tenant-scoped `profiles` row. Active
/// follower relations are resolved through the Social Graph owner port and do
/// not depend on localized presentation copy, taxonomy labels, or media joins.
#[derive(Clone, Debug)]
pub struct ProfilePrivacyService {
    db: DatabaseConnection,
}

impl ProfilePrivacyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn evaluate_access(
        &self,
        tenant_id: Uuid,
        recipient_id: Uuid,
        audience: ProfileAccessAudience,
    ) -> Result<ProfilePrivacyDecision, PortError> {
        let decisions = self
            .evaluate_access_batch(tenant_id, &[recipient_id], audience)
            .await?;
        Ok(decisions
            .get(&recipient_id)
            .copied()
            .unwrap_or(ProfilePrivacyDecision::RecipientUnavailable))
    }

    /// Evaluate a bounded presentation batch with one tenant-scoped base-row query.
    ///
    /// Every distinct requested id is represented in the returned map. Missing,
    /// cross-tenant, hidden, and blocked profiles remain `RecipientUnavailable`.
    /// Active `followers_only` candidates are resolved through bounded Social
    /// Graph owner batches before presentation summaries are loaded.
    pub async fn evaluate_access_batch(
        &self,
        tenant_id: Uuid,
        recipient_ids: &[Uuid],
        audience: ProfileAccessAudience,
    ) -> Result<HashMap<Uuid, ProfilePrivacyDecision>, PortError> {
        let mut decisions = recipient_ids
            .iter()
            .copied()
            .map(|recipient_id| (recipient_id, ProfilePrivacyDecision::RecipientUnavailable))
            .collect::<HashMap<_, _>>();
        if decisions.is_empty() {
            return Ok(decisions);
        }

        let recipient_ids = decisions.keys().copied().collect::<Vec<_>>();
        let recipient_count = recipient_ids.len();
        let profiles = entities::profile::Entity::find()
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .filter(entities::profile::Column::UserId.is_in(recipient_ids))
            .all(&self.db)
            .await
            .map_err(|error| {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    recipient_count,
                    error = %error,
                    "Profile privacy batch read failed"
                );
                PortError::unavailable(
                    "profiles.privacy_read_unavailable",
                    "profile privacy state is temporarily unavailable",
                )
            })?;

        let actor_id = audience.actor_id();
        let mut follower_candidates = Vec::new();
        for profile in profiles {
            let decision = evaluate_profile_access(
                profile.user_id,
                profile.status,
                profile.visibility,
                audience,
            );
            if decision == ProfilePrivacyDecision::Restricted
                && profile.status == ProfileStatus::Active
                && profile.visibility == ProfileVisibility::FollowersOnly
                && actor_id.is_some_and(|actor_id| actor_id != profile.user_id)
            {
                follower_candidates.push(profile.user_id);
            }
            decisions.insert(profile.user_id, decision);
        }

        if let Some(actor_id) = actor_id {
            let followed_profile_ids = self
                .followed_profile_ids(tenant_id, actor_id, &follower_candidates)
                .await?;
            apply_followed_profile_access(&mut decisions, &followed_profile_ids);
        }

        Ok(decisions)
    }

    async fn followed_profile_ids(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        profile_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, PortError> {
        if profile_ids.is_empty() {
            return Ok(Vec::new());
        }

        let social_graph = SocialGraphService::new(self.db.clone());
        let mut followed = HashSet::new();
        for chunk in profile_ids.chunks(MAX_SOCIAL_GRAPH_FOLLOW_TARGETS) {
            let context = PortContext::new(
                tenant_id.to_string(),
                PortActor::service("profiles-privacy"),
                "und",
                Uuid::new_v4().to_string(),
            )
            .with_deadline(PROFILE_FOLLOW_READ_DEADLINE);
            let result = SocialGraphPrivacyReadPort::source_follows_targets(
                &social_graph,
                context,
                SocialGraphFollowBatchRequest {
                    source_user_id: actor_id,
                    target_user_ids: chunk.to_vec(),
                },
            )
            .await?;
            followed.extend(result.followed_target_user_ids);
        }

        let mut followed = followed.into_iter().collect::<Vec<_>>();
        followed.sort_unstable();
        Ok(followed)
    }
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
        let audience = audience_from_port_context(&context, request.actor_id)?;

        self.evaluate_access(tenant_id, request.recipient_id, audience)
            .await
    }
}

fn apply_followed_profile_access(
    decisions: &mut HashMap<Uuid, ProfilePrivacyDecision>,
    followed_profile_ids: &[Uuid],
) {
    for profile_id in followed_profile_ids {
        if decisions.get(profile_id) == Some(&ProfilePrivacyDecision::Restricted) {
            decisions.insert(*profile_id, ProfilePrivacyDecision::Allow);
        }
    }
}

fn audience_from_port_context(
    context: &PortContext,
    actor_id: Option<Uuid>,
) -> Result<ProfileAccessAudience, PortError> {
    match &context.actor.kind {
        PortActorKind::User => {
            let context_actor_id = Uuid::parse_str(&context.actor.id).map_err(|_| {
                PortError::validation(
                    "profiles.actor_id_invalid",
                    "profile privacy user actors require a valid actor identifier",
                )
            })?;
            if actor_id != Some(context_actor_id) {
                return Err(PortError::validation(
                    "profiles.actor_id_mismatch",
                    "profile privacy request actor does not match the port context actor",
                ));
            }
            Ok(ProfileAccessAudience::Authenticated {
                actor_id: context_actor_id,
            })
        }
        PortActorKind::Service | PortActorKind::System => {
            Ok(ProfileAccessAudience::TrustedService { actor_id })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_public_profiles_allow_every_audience() {
        let recipient_id = Uuid::new_v4();
        for audience in [
            ProfileAccessAudience::Anonymous,
            ProfileAccessAudience::Authenticated {
                actor_id: Uuid::new_v4(),
            },
            ProfileAccessAudience::TrustedService { actor_id: None },
        ] {
            assert_eq!(
                evaluate_profile_access(
                    recipient_id,
                    ProfileStatus::Active,
                    ProfileVisibility::Public,
                    audience,
                ),
                ProfilePrivacyDecision::Allow
            );
        }
    }

    #[test]
    fn authenticated_profiles_reject_anonymous_audience() {
        let recipient_id = Uuid::new_v4();
        assert_eq!(
            evaluate_profile_access(
                recipient_id,
                ProfileStatus::Active,
                ProfileVisibility::Authenticated,
                ProfileAccessAudience::Anonymous,
            ),
            ProfilePrivacyDecision::Restricted
        );
        assert_eq!(
            evaluate_profile_access(
                recipient_id,
                ProfileStatus::Active,
                ProfileVisibility::Authenticated,
                ProfileAccessAudience::Authenticated {
                    actor_id: Uuid::new_v4(),
                },
            ),
            ProfilePrivacyDecision::Allow
        );
    }

    #[test]
    fn owner_can_read_active_private_or_followers_only_profile() {
        let recipient_id = Uuid::new_v4();
        for visibility in [ProfileVisibility::FollowersOnly, ProfileVisibility::Private] {
            assert_eq!(
                evaluate_profile_access(
                    recipient_id,
                    ProfileStatus::Active,
                    visibility,
                    ProfileAccessAudience::Authenticated {
                        actor_id: recipient_id,
                    },
                ),
                ProfilePrivacyDecision::Allow
            );
        }
    }

    #[test]
    fn followers_only_requires_relationship_resolution_for_non_owner() {
        let recipient_id = Uuid::new_v4();
        assert_eq!(
            evaluate_profile_access(
                recipient_id,
                ProfileStatus::Active,
                ProfileVisibility::FollowersOnly,
                ProfileAccessAudience::Authenticated {
                    actor_id: Uuid::new_v4(),
                },
            ),
            ProfilePrivacyDecision::Restricted
        );
    }

    #[test]
    fn followed_profile_overlay_upgrades_only_restricted_decisions() {
        let followed_id = Uuid::new_v4();
        let unavailable_id = Uuid::new_v4();
        let mut decisions = HashMap::from([
            (followed_id, ProfilePrivacyDecision::Restricted),
            (unavailable_id, ProfilePrivacyDecision::RecipientUnavailable),
        ]);

        apply_followed_profile_access(&mut decisions, &[followed_id, unavailable_id]);

        assert_eq!(decisions[&followed_id], ProfilePrivacyDecision::Allow);
        assert_eq!(
            decisions[&unavailable_id],
            ProfilePrivacyDecision::RecipientUnavailable
        );
    }

    #[test]
    fn hidden_and_blocked_profiles_fail_closed_before_owner_access() {
        let recipient_id = Uuid::new_v4();
        for status in [ProfileStatus::Hidden, ProfileStatus::Blocked] {
            assert_eq!(
                evaluate_profile_access(
                    recipient_id,
                    status,
                    ProfileVisibility::Public,
                    ProfileAccessAudience::Authenticated {
                        actor_id: recipient_id,
                    },
                ),
                ProfilePrivacyDecision::RecipientUnavailable
            );
        }
    }
}
