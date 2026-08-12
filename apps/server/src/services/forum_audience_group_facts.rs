use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_forum::{
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    MAX_FORUM_AUDIENCE_GROUPS, SharedForumAudienceFactsPort,
};
use rustok_groups::{
    GroupMembershipEnforcementService, ReadGroupMembershipEnforcementRequest,
    SharedGroupMembershipEnforcementReadPort,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const INVALID_REQUEST_CODE: &str = "forum.audience_group_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.audience_group_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.audience_group_facts.actor_mismatch";
const OWNER_RESPONSE_CODE: &str = "forum.audience_group_facts.owner_response_invalid";
const PARTIAL_PROVIDER_CODE: &str = "forum.audience_group_facts.partial_provider_unavailable";

/// Server-owned adapter from Forum's bounded audience-facts capability to the
/// Groups-owned effective-membership read port.
///
/// The adapter resolves only requested group identifiers. Trust and channel
/// facts remain unsupported: when no requested group membership already
/// decides the positive-selector union, those dimensions return typed
/// retryable unavailability instead of being misrepresented as negative facts.
#[derive(Clone)]
pub(crate) struct ServerForumAudienceGroupFactsPort {
    groups: SharedGroupMembershipEnforcementReadPort,
}

impl ServerForumAudienceGroupFactsPort {
    pub(crate) fn new(groups: SharedGroupMembershipEnforcementReadPort) -> Self {
        Self { groups }
    }

    pub(crate) fn from_db(db: DatabaseConnection) -> Self {
        Self::new(Arc::new(GroupMembershipEnforcementService::new(db)))
    }

    pub(crate) fn shared(db: DatabaseConnection) -> SharedForumAudienceFactsPort {
        Arc::new(Self::from_db(db))
    }
}

#[async_trait]
impl ForumAudienceFactsPort for ServerForumAudienceGroupFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        validate_request(&context, &request)?;

        let mut group_memberships = Vec::with_capacity(request.group_ids.len());
        for group_id in &request.group_ids {
            let state = self
                .groups
                .read_membership_enforcement(
                    context.clone(),
                    ReadGroupMembershipEnforcementRequest {
                        group_id: *group_id,
                        user_id: request.user_id,
                    },
                )
                .await?;
            validate_owner_state(&request, *group_id, &state)?;
            if state.active_member {
                group_memberships.push(*group_id);
            }
        }

        if group_memberships.is_empty()
            && (request.include_trust_level || !request.channel_slugs.is_empty())
        {
            return Err(partial_provider_unavailable());
        }

        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: None,
            channel_memberships: Vec::new(),
            group_memberships,
        })
    }
}

fn validate_request(
    context: &PortContext,
    request: &ForumAudienceFactsRequest,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    if request.tenant_id.is_nil()
        || request.user_id.is_nil()
        || request.group_ids.iter().any(Uuid::is_nil)
        || request.group_ids.len() > MAX_FORUM_AUDIENCE_GROUPS
    {
        return Err(PortError::validation(
            INVALID_REQUEST_CODE,
            "Forum audience group facts request is invalid",
        ));
    }
    if context.tenant_id != request.tenant_id.to_string() {
        return Err(PortError::validation(
            TENANT_MISMATCH_CODE,
            "Forum audience group facts tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(request.user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum audience group facts require the exact requested user actor",
        ));
    }
    Ok(())
}

fn validate_owner_state(
    request: &ForumAudienceFactsRequest,
    group_id: Uuid,
    state: &rustok_groups::GroupMembershipEffectiveState,
) -> Result<(), PortError> {
    if state.tenant_id != request.tenant_id
        || state.group_id != group_id
        || state.user_id != request.user_id
    {
        return Err(PortError::invariant_violation(
            OWNER_RESPONSE_CODE,
            "Groups owner returned a different audience membership identity",
        ));
    }
    Ok(())
}

fn partial_provider_unavailable() -> PortError {
    PortError::unavailable(
        PARTIAL_PROVIDER_CODE,
        "Forum trust or channel audience facts are not available from the group facts adapter",
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::time::Duration;

    use chrono::Utc;
    use rustok_api::{PortActor, PortErrorKind};
    use rustok_groups::{
        GroupMembershipEffectiveState, GroupMembershipEffectiveStatus, GroupMembershipStatus,
        GroupRole,
    };

    use super::*;

    #[derive(Clone)]
    struct StaticGroupMembershipPort {
        active_groups: BTreeSet<Uuid>,
        calls: Arc<Mutex<Vec<Uuid>>>,
    }

    #[async_trait]
    impl GroupMembershipEnforcementReadPort for StaticGroupMembershipPort {
        async fn read_membership_enforcement(
            &self,
            context: PortContext,
            request: ReadGroupMembershipEnforcementRequest,
        ) -> Result<GroupMembershipEffectiveState, PortError> {
            context.require_policy(PortCallPolicy::read())?;
            self.calls
                .lock()
                .expect("group fact call recorder should stay available")
                .push(request.group_id);
            let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
                PortError::validation("test.invalid_tenant", "Test tenant is invalid")
            })?;
            let active = self.active_groups.contains(&request.group_id);
            Ok(GroupMembershipEffectiveState {
                tenant_id,
                group_id: request.group_id,
                user_id: request.user_id,
                membership_id: active.then(Uuid::new_v4),
                role: active.then_some(GroupRole::Member),
                stored_status: active.then_some(GroupMembershipStatus::Active),
                membership_revision: active.then_some(1),
                effective_status: if active {
                    GroupMembershipEffectiveStatus::Active
                } else {
                    GroupMembershipEffectiveStatus::Missing
                },
                active_member: active,
                denied_reentry: false,
                enforcement: None,
                evaluated_at: Utc::now(),
            })
        }
    }

    fn user_context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "forum-group-facts-test",
        )
        .with_deadline(Duration::from_secs(2))
    }

    fn adapter(
        active_groups: impl IntoIterator<Item = Uuid>,
    ) -> (ServerForumAudienceGroupFactsPort, Arc<Mutex<Vec<Uuid>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let groups: SharedGroupMembershipEnforcementReadPort =
            Arc::new(StaticGroupMembershipPort {
                active_groups: active_groups.into_iter().collect(),
                calls: calls.clone(),
            });
        (ServerForumAudienceGroupFactsPort::new(groups), calls)
    }

    #[tokio::test]
    async fn group_facts_resolve_only_requested_active_memberships() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let first = Uuid::new_v4();
        let active = Uuid::new_v4();
        let third = Uuid::new_v4();
        let requested = vec![first, active, third];
        let (adapter, calls) = adapter([active]);

        let facts = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: false,
                    channel_slugs: Vec::new(),
                    group_ids: requested.clone(),
                },
            )
            .await
            .expect("requested group facts should resolve");

        assert_eq!(facts.tenant_id, tenant_id);
        assert_eq!(facts.user_id, user_id);
        assert_eq!(facts.group_memberships, vec![active]);
        assert!(facts.trust_level.is_none());
        assert!(facts.channel_memberships.is_empty());
        assert_eq!(
            *calls
                .lock()
                .expect("group fact call recorder should stay available"),
            requested
        );
    }

    #[tokio::test]
    async fn active_group_match_short_circuits_unsupported_positive_dimensions() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let active = Uuid::new_v4();
        let (adapter, _) = adapter([active]);

        let facts = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: true,
                    channel_slugs: vec!["mobile".to_string()],
                    group_ids: vec![active],
                },
            )
            .await
            .expect("an active requested group should decide the positive-selector union");

        assert_eq!(facts.group_memberships, vec![active]);
    }

    #[tokio::test]
    async fn unsupported_dimensions_are_retryable_when_groups_do_not_decide() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let (adapter, _) = adapter([]);

        let error = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: true,
                    channel_slugs: vec!["mobile".to_string()],
                    group_ids: vec![Uuid::new_v4()],
                },
            )
            .await
            .expect_err("unsupported owner facts must not be converted into a final deny");

        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert!(error.retryable);
        assert_eq!(error.code, PARTIAL_PROVIDER_CODE);
    }

    #[tokio::test]
    async fn foreign_user_context_is_rejected_before_owner_calls() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let (adapter, calls) = adapter([]);

        let error = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, Uuid::new_v4()),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: false,
                    channel_slugs: Vec::new(),
                    group_ids: vec![Uuid::new_v4()],
                },
            )
            .await
            .expect_err("foreign user facts lookup must fail closed");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert!(
            calls
                .lock()
                .expect("group fact call recorder should stay available")
                .is_empty()
        );
    }
}

#[cfg(test)]
mod owner_backed_tests;
