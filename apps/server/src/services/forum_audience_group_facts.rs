use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_channel::{
    ChannelListRequest, ChannelReadPort, ChannelReadProjection, ChannelReadRequest,
    ChannelReadSelector, ChannelService,
};
use rustok_forum::{
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    MAX_FORUM_AUDIENCE_GROUPS, SharedForumAudienceFactsPort,
};
#[cfg(feature = "mod-groups")]
use rustok_groups::{
    GroupMembershipEnforcementReadPort, GroupMembershipEnforcementService,
    ReadGroupMembershipEnforcementRequest, SharedGroupMembershipEnforcementReadPort,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const INVALID_REQUEST_CODE: &str = "forum.audience_group_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.audience_group_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.audience_group_facts.actor_mismatch";
const GROUP_OWNER_RESPONSE_CODE: &str = "forum.audience_group_facts.owner_response_invalid";
const CHANNEL_OWNER_RESPONSE_CODE: &str = "forum.audience_channel_facts.owner_response_invalid";
const PARTIAL_PROVIDER_CODE: &str = "forum.audience_group_facts.partial_provider_unavailable";

type SharedChannelReadPort = Arc<dyn ChannelReadPort>;

/// Server-owned adapter from Forum's bounded audience-facts capability to the
/// Channel-owned active-channel read port and, when compiled, the Groups-owned
/// effective-membership read port.
///
/// Channel membership means that the trusted middleware-resolved caller channel
/// is one of the exact requested slugs and still resolves to an active channel
/// in the same tenant. The adapter never discovers other channels. Group reads
/// remain exact and bounded. Trust facts remain unsupported and therefore fail
/// closed with typed retryable unavailability when neither a channel nor group
/// match already decides the positive-selector union.
#[derive(Clone)]
pub(crate) struct ServerForumAudienceGroupFactsPort {
    channels: SharedChannelReadPort,
    #[cfg(feature = "mod-groups")]
    groups: SharedGroupMembershipEnforcementReadPort,
}

impl ServerForumAudienceGroupFactsPort {
    pub(crate) fn from_db(db: DatabaseConnection) -> Self {
        Self {
            channels: Arc::new(ChannelService::new(db.clone())),
            #[cfg(feature = "mod-groups")]
            groups: Arc::new(GroupMembershipEnforcementService::new(db)),
        }
    }

    pub(crate) fn shared(db: DatabaseConnection) -> SharedForumAudienceFactsPort {
        Arc::new(Self::from_db(db))
    }

    async fn resolve_channel_memberships(
        &self,
        context: &PortContext,
        request: &ForumAudienceFactsRequest,
    ) -> Result<Vec<String>, PortError> {
        if request.channel_slugs.is_empty() {
            return Ok(Vec::new());
        }

        let Some(current_channel) = context
            .channel
            .as_deref()
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(str::to_lowercase)
        else {
            return Ok(Vec::new());
        };

        if request
            .channel_slugs
            .binary_search(&current_channel)
            .is_err()
        {
            return Ok(Vec::new());
        }

        let projection = self
            .channels
            .read_channel(
                context.clone(),
                ChannelReadRequest {
                    selector: ChannelReadSelector::Slug(current_channel.clone()),
                    include_inactive: false,
                },
            )
            .await?;
        let Some(projection) = projection else {
            return Ok(Vec::new());
        };
        validate_channel_owner_projection(request, &current_channel, &projection)?;
        Ok(vec![current_channel])
    }

    #[cfg(feature = "mod-groups")]
    async fn resolve_group_memberships(
        &self,
        context: &PortContext,
        request: &ForumAudienceFactsRequest,
    ) -> Result<Vec<Uuid>, PortError> {
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
            validate_group_owner_state(request, *group_id, &state)?;
            if state.active_member {
                group_memberships.push(*group_id);
            }
        }
        Ok(group_memberships)
    }
}

#[async_trait]
impl ForumAudienceFactsPort for ServerForumAudienceGroupFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        let request = normalize_request(request)?;
        validate_context(&context, &request)?;

        let channel_memberships = self
            .resolve_channel_memberships(&context, &request)
            .await?;

        #[cfg(feature = "mod-groups")]
        let (group_memberships, unresolved_group_facts) = (
            self.resolve_group_memberships(&context, &request).await?,
            false,
        );
        #[cfg(not(feature = "mod-groups"))]
        let (group_memberships, unresolved_group_facts) =
            (Vec::new(), !request.group_ids.is_empty());

        let positive_match =
            !channel_memberships.is_empty() || !group_memberships.is_empty();
        if !positive_match && (request.include_trust_level || unresolved_group_facts) {
            return Err(partial_provider_unavailable());
        }

        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: None,
            channel_memberships,
            group_memberships,
        })
    }
}

fn normalize_request(
    request: ForumAudienceFactsRequest,
) -> Result<ForumAudienceFactsRequest, PortError> {
    request.normalize().map_err(|_| {
        PortError::validation(
            INVALID_REQUEST_CODE,
            "Forum audience facts request is invalid",
        )
    })
}

fn validate_context(
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
            "Forum audience facts request is invalid",
        ));
    }
    if context.tenant_id != request.tenant_id.to_string() {
        return Err(PortError::validation(
            TENANT_MISMATCH_CODE,
            "Forum audience facts tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(request.user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum audience facts require the exact requested user actor",
        ));
    }
    Ok(())
}

fn validate_channel_owner_projection(
    request: &ForumAudienceFactsRequest,
    requested_slug: &str,
    projection: &ChannelReadProjection,
) -> Result<(), PortError> {
    let channel = &projection.detail.channel;
    if channel.tenant_id != request.tenant_id
        || channel.slug.trim().to_lowercase() != requested_slug
        || !channel.is_active
    {
        return Err(PortError::invariant_violation(
            CHANNEL_OWNER_RESPONSE_CODE,
            "Channel owner returned a different or inactive audience channel",
        ));
    }
    Ok(())
}

#[cfg(feature = "mod-groups")]
fn validate_group_owner_state(
    request: &ForumAudienceFactsRequest,
    group_id: Uuid,
    state: &rustok_groups::GroupMembershipEffectiveState,
) -> Result<(), PortError> {
    if state.tenant_id != request.tenant_id
        || state.group_id != group_id
        || state.user_id != request.user_id
    {
        return Err(PortError::invariant_violation(
            GROUP_OWNER_RESPONSE_CODE,
            "Groups owner returned a different audience membership identity",
        ));
    }
    Ok(())
}

fn partial_provider_unavailable() -> PortError {
    PortError::unavailable(
        PARTIAL_PROVIDER_CODE,
        "Forum trust or optional group audience facts are not available from the host adapter",
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::time::Duration;

    use chrono::Utc;
    use rustok_api::{PortActor, PortErrorKind};
    use rustok_channel::{ChannelDetailResponse, ChannelResponse};
    #[cfg(feature = "mod-groups")]
    use rustok_groups::{
        GroupMembershipEffectiveState, GroupMembershipEffectiveStatus, GroupMembershipStatus,
        GroupRole,
    };

    use super::*;

    #[derive(Clone)]
    struct StaticChannelReadPort {
        tenant_id: Uuid,
        active_slugs: BTreeSet<String>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ChannelReadPort for StaticChannelReadPort {
        async fn read_channel(
            &self,
            context: PortContext,
            request: ChannelReadRequest,
        ) -> Result<Option<ChannelReadProjection>, PortError> {
            context.require_policy(PortCallPolicy::read())?;
            let ChannelReadSelector::Slug(slug) = request.selector else {
                return Err(PortError::validation(
                    "test.selector",
                    "Test channel port requires a slug selector",
                ));
            };
            self.calls
                .lock()
                .expect("channel fact call recorder should stay available")
                .push(slug.clone());
            if !self.active_slugs.contains(&slug) || request.include_inactive {
                return Ok(None);
            }
            let now = Utc::now();
            Ok(Some(ChannelReadProjection {
                detail: ChannelDetailResponse {
                    channel: ChannelResponse {
                        id: Uuid::new_v4(),
                        tenant_id: self.tenant_id,
                        slug,
                        name: "Test channel".to_string(),
                        is_active: true,
                        is_default: false,
                        status: "active".to_string(),
                        settings: serde_json::json!({}),
                        created_at: now,
                        updated_at: now,
                    },
                    targets: Vec::new(),
                    module_bindings: Vec::new(),
                    oauth_apps: Vec::new(),
                },
            }))
        }

        async fn list_channels_for_tenant(
            &self,
            _context: PortContext,
            _request: ChannelListRequest,
        ) -> Result<Vec<ChannelReadProjection>, PortError> {
            Ok(Vec::new())
        }
    }

    #[cfg(feature = "mod-groups")]
    #[derive(Clone)]
    struct StaticGroupMembershipPort {
        active_groups: BTreeSet<Uuid>,
        calls: Arc<Mutex<Vec<Uuid>>>,
    }

    #[cfg(feature = "mod-groups")]
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

    fn user_context(tenant_id: Uuid, user_id: Uuid, channel: Option<&str>) -> PortContext {
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "forum-audience-facts-test",
        )
        .with_deadline(Duration::from_secs(2));
        match channel {
            Some(channel) => context.with_channel(channel.to_string()),
            None => context,
        }
    }

    fn channel_port(
        tenant_id: Uuid,
        active_slugs: impl IntoIterator<Item = &'static str>,
    ) -> (SharedChannelReadPort, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let channels: SharedChannelReadPort = Arc::new(StaticChannelReadPort {
            tenant_id,
            active_slugs: active_slugs.into_iter().map(str::to_string).collect(),
            calls: calls.clone(),
        });
        (channels, calls)
    }

    #[cfg(feature = "mod-groups")]
    fn adapter(
        tenant_id: Uuid,
        active_channels: impl IntoIterator<Item = &'static str>,
        active_groups: impl IntoIterator<Item = Uuid>,
    ) -> (
        ServerForumAudienceGroupFactsPort,
        Arc<Mutex<Vec<String>>>,
        Arc<Mutex<Vec<Uuid>>>,
    ) {
        let (channels, channel_calls) = channel_port(tenant_id, active_channels);
        let group_calls = Arc::new(Mutex::new(Vec::new()));
        let groups: SharedGroupMembershipEnforcementReadPort =
            Arc::new(StaticGroupMembershipPort {
                active_groups: active_groups.into_iter().collect(),
                calls: group_calls.clone(),
            });
        (
            ServerForumAudienceGroupFactsPort { channels, groups },
            channel_calls,
            group_calls,
        )
    }

    #[cfg(not(feature = "mod-groups"))]
    fn adapter(
        tenant_id: Uuid,
        active_channels: impl IntoIterator<Item = &'static str>,
    ) -> (ServerForumAudienceGroupFactsPort, Arc<Mutex<Vec<String>>>) {
        let (channels, channel_calls) = channel_port(tenant_id, active_channels);
        (
            ServerForumAudienceGroupFactsPort { channels },
            channel_calls,
        )
    }

    #[tokio::test]
    async fn channel_facts_confirm_only_the_requested_active_resolved_channel() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        #[cfg(feature = "mod-groups")]
        let (adapter, calls, _) = adapter(tenant_id, ["members"], []);
        #[cfg(not(feature = "mod-groups"))]
        let (adapter, calls) = adapter(tenant_id, ["members"]);

        let facts = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id, Some("MEMBERS")),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: false,
                    channel_slugs: vec!["members".to_string(), "partners".to_string()],
                    group_ids: Vec::new(),
                },
            )
            .await
            .expect("active resolved channel should be confirmed");

        assert_eq!(facts.channel_memberships, vec!["members".to_string()]);
        assert_eq!(
            *calls
                .lock()
                .expect("channel fact call recorder should stay available"),
            vec!["members".to_string()]
        );
    }

    #[tokio::test]
    async fn unrequested_route_channel_does_not_trigger_owner_discovery() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        #[cfg(feature = "mod-groups")]
        let (adapter, calls, _) = adapter(tenant_id, ["members"], []);
        #[cfg(not(feature = "mod-groups"))]
        let (adapter, calls) = adapter(tenant_id, ["members"]);

        let facts = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id, Some("public")),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: false,
                    channel_slugs: vec!["members".to_string()],
                    group_ids: Vec::new(),
                },
            )
            .await
            .expect("a non-matching resolved channel should be an authoritative miss");

        assert!(facts.channel_memberships.is_empty());
        assert!(
            calls
                .lock()
                .expect("channel fact call recorder should stay available")
                .is_empty()
        );
    }

    #[cfg(feature = "mod-groups")]
    #[tokio::test]
    async fn group_facts_resolve_only_requested_active_memberships() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let first = Uuid::new_v4();
        let active = Uuid::new_v4();
        let third = Uuid::new_v4();
        let requested = vec![first, active, third];
        let (adapter, _, calls) = adapter(tenant_id, [], [active]);

        let facts = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id, None),
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

        assert_eq!(facts.group_memberships, vec![active]);
        assert_eq!(
            *calls
                .lock()
                .expect("group fact call recorder should stay available"),
            requested
        );
    }

    #[tokio::test]
    async fn a_channel_match_short_circuits_unsupported_positive_dimensions() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        #[cfg(feature = "mod-groups")]
        let (adapter, _, _) = adapter(tenant_id, ["members"], []);
        #[cfg(not(feature = "mod-groups"))]
        let (adapter, _) = adapter(tenant_id, ["members"]);

        let facts = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id, Some("members")),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: true,
                    channel_slugs: vec!["members".to_string()],
                    group_ids: vec![Uuid::new_v4()],
                },
            )
            .await
            .expect("a channel match should decide the positive-selector union");

        assert_eq!(facts.channel_memberships, vec!["members".to_string()]);
    }

    #[tokio::test]
    async fn unsupported_dimensions_are_retryable_when_available_facts_do_not_decide() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        #[cfg(feature = "mod-groups")]
        let (adapter, _, _) = adapter(tenant_id, [], []);
        #[cfg(not(feature = "mod-groups"))]
        let (adapter, _) = adapter(tenant_id, []);

        let error = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, user_id, None),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: true,
                    channel_slugs: vec!["members".to_string()],
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
        #[cfg(feature = "mod-groups")]
        let (adapter, channel_calls, group_calls) = adapter(tenant_id, ["members"], []);
        #[cfg(not(feature = "mod-groups"))]
        let (adapter, channel_calls) = adapter(tenant_id, ["members"]);

        let error = adapter
            .resolve_forum_audience_facts(
                user_context(tenant_id, Uuid::new_v4(), Some("members")),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: false,
                    channel_slugs: vec!["members".to_string()],
                    group_ids: Vec::new(),
                },
            )
            .await
            .expect_err("foreign user facts lookup must fail closed");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert!(
            channel_calls
                .lock()
                .expect("channel fact call recorder should stay available")
                .is_empty()
        );
        #[cfg(feature = "mod-groups")]
        assert!(
            group_calls
                .lock()
                .expect("group fact call recorder should stay available")
                .is_empty()
        );
    }
}
