use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_channel::{
    ChannelReadPort, ChannelReadProjection, ChannelReadRequest, ChannelReadSelector, ChannelService,
};
use rustok_forum::{
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    SharedForumAudienceFactsPort,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const INVALID_REQUEST_CODE: &str = "forum.audience_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.audience_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.audience_facts.actor_mismatch";
const CHANNEL_OWNER_RESPONSE_CODE: &str = "forum.audience_facts.channel_owner_response_invalid";
const PARTIAL_PROVIDER_CODE: &str = "forum.audience_facts.partial_provider_unavailable";

type SharedChannelReadPort = Arc<dyn ChannelReadPort>;

/// Host composition for exact Forum audience facts.
///
/// The trusted request channel is accepted only when it is one of the exact
/// requested slugs and the Channel owner still resolves it as active in the
/// same tenant. No other channel is listed or discovered. When Groups is
/// compiled, the historical exact-group adapter remains a separate owner bridge
/// and is called only when the channel fact did not already decide the positive
/// selector union. Trust remains unavailable and therefore fails closed.
#[derive(Clone)]
pub(crate) struct ServerForumAudienceFactsPort {
    channels: SharedChannelReadPort,
    groups: Option<SharedForumAudienceFactsPort>,
}

impl ServerForumAudienceFactsPort {
    pub(crate) fn new(
        channels: SharedChannelReadPort,
        groups: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self { channels, groups }
    }

    pub(crate) fn from_db(
        db: DatabaseConnection,
        groups: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self::new(Arc::new(ChannelService::new(db)), groups)
    }

    pub(crate) fn shared(
        db: DatabaseConnection,
        groups: Option<SharedForumAudienceFactsPort>,
    ) -> SharedForumAudienceFactsPort {
        Arc::new(Self::from_db(db, groups))
    }

    async fn resolve_current_channel(
        &self,
        context: &PortContext,
        request: &ForumAudienceFactsRequest,
    ) -> Result<Vec<String>, PortError> {
        if request.channel_slugs.is_empty() {
            return Ok(Vec::new());
        }

        let Some(channel_slug) = context
            .channel
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase)
        else {
            return Ok(Vec::new());
        };

        if request.channel_slugs.binary_search(&channel_slug).is_err() {
            return Ok(Vec::new());
        }

        let projection = self
            .channels
            .read_channel(
                context.clone(),
                ChannelReadRequest {
                    selector: ChannelReadSelector::Slug(channel_slug.clone()),
                    include_inactive: false,
                },
            )
            .await?;
        let Some(projection) = projection else {
            return Ok(Vec::new());
        };
        validate_channel_projection(request, &channel_slug, &projection)?;
        Ok(vec![channel_slug])
    }

    async fn resolve_groups(
        &self,
        context: PortContext,
        request: &ForumAudienceFactsRequest,
    ) -> Result<Option<Vec<Uuid>>, PortError> {
        if request.group_ids.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let Some(groups) = &self.groups else {
            return Ok(None);
        };

        let group_request = ForumAudienceFactsRequest {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            include_trust_level: false,
            channel_slugs: Vec::new(),
            group_ids: request.group_ids.clone(),
        };
        let facts = groups
            .resolve_forum_audience_facts(context, group_request)
            .await?;
        if facts.tenant_id != request.tenant_id
            || facts.user_id != request.user_id
            || facts.trust_level.is_some()
            || !facts.channel_memberships.is_empty()
            || facts
                .group_memberships
                .iter()
                .any(|group_id| request.group_ids.binary_search(group_id).is_err())
        {
            return Err(PortError::invariant_violation(
                PARTIAL_PROVIDER_CODE,
                "Forum group audience facts returned an invalid bounded response",
            ));
        }
        Ok(Some(facts.group_memberships))
    }
}

#[async_trait]
impl ForumAudienceFactsPort for ServerForumAudienceFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        let request = normalize_request(request)?;
        validate_context(&context, &request)?;

        let channel_memberships = self.resolve_current_channel(&context, &request).await?;
        if !channel_memberships.is_empty() {
            return Ok(ForumAudienceFacts {
                tenant_id: request.tenant_id,
                user_id: request.user_id,
                trust_level: None,
                channel_memberships,
                group_memberships: Vec::new(),
            });
        }

        let group_memberships = self.resolve_groups(context, &request).await?;
        if let Some(group_memberships) = group_memberships {
            if !group_memberships.is_empty() {
                return Ok(ForumAudienceFacts {
                    tenant_id: request.tenant_id,
                    user_id: request.user_id,
                    trust_level: None,
                    channel_memberships: Vec::new(),
                    group_memberships,
                });
            }
            if !request.include_trust_level {
                return Ok(ForumAudienceFacts {
                    tenant_id: request.tenant_id,
                    user_id: request.user_id,
                    trust_level: None,
                    channel_memberships: Vec::new(),
                    group_memberships,
                });
            }
        }

        Err(partial_provider_unavailable())
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

fn validate_channel_projection(
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
            "Channel owner returned a different or inactive Forum audience channel",
        ));
    }
    Ok(())
}

fn partial_provider_unavailable() -> PortError {
    PortError::unavailable(
        PARTIAL_PROVIDER_CODE,
        "Forum trust or optional group audience facts are unavailable",
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::time::Duration;

    use chrono::Utc;
    use rustok_api::{PortActor, PortErrorKind};
    use rustok_channel::{ChannelDetailResponse, ChannelListRequest, ChannelResponse};

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
                    "test.channel_selector",
                    "Test Channel owner requires a slug selector",
                ));
            };
            self.calls
                .lock()
                .expect("channel call recorder should stay available")
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
                        updated_at: Utc::now(),
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

    #[derive(Clone)]
    struct StaticGroupFactsPort {
        active_groups: BTreeSet<Uuid>,
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl ForumAudienceFactsPort for StaticGroupFactsPort {
        async fn resolve_forum_audience_facts(
            &self,
            _context: PortContext,
            request: ForumAudienceFactsRequest,
        ) -> Result<ForumAudienceFacts, PortError> {
            *self
                .calls
                .lock()
                .expect("group call recorder should stay available") += 1;
            Ok(ForumAudienceFacts {
                tenant_id: request.tenant_id,
                user_id: request.user_id,
                trust_level: None,
                channel_memberships: Vec::new(),
                group_memberships: request
                    .group_ids
                    .into_iter()
                    .filter(|group_id| self.active_groups.contains(group_id))
                    .collect(),
            })
        }
    }

    fn context(tenant_id: Uuid, user_id: Uuid, channel: Option<&str>) -> PortContext {
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "forum-channel-facts-test",
        )
        .with_deadline(Duration::from_secs(2));
        match channel {
            Some(channel) => context.with_channel(channel.to_string()),
            None => context,
        }
    }

    fn adapter(
        tenant_id: Uuid,
        active_channels: impl IntoIterator<Item = &'static str>,
        active_groups: impl IntoIterator<Item = Uuid>,
    ) -> (
        ServerForumAudienceFactsPort,
        Arc<Mutex<Vec<String>>>,
        Arc<Mutex<usize>>,
    ) {
        let channel_calls = Arc::new(Mutex::new(Vec::new()));
        let channels: SharedChannelReadPort = Arc::new(StaticChannelReadPort {
            tenant_id,
            active_slugs: active_channels.into_iter().map(str::to_string).collect(),
            calls: channel_calls.clone(),
        });
        let group_calls = Arc::new(Mutex::new(0));
        let groups: SharedForumAudienceFactsPort = Arc::new(StaticGroupFactsPort {
            active_groups: active_groups.into_iter().collect(),
            calls: group_calls.clone(),
        });
        (
            ServerForumAudienceFactsPort::new(channels, Some(groups)),
            channel_calls,
            group_calls,
        )
    }

    #[tokio::test]
    async fn requested_active_current_channel_is_confirmed_without_group_calls() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let (adapter, channel_calls, group_calls) = adapter(tenant_id, ["members"], [group_id]);

        let facts = adapter
            .resolve_forum_audience_facts(
                context(tenant_id, user_id, Some("MEMBERS")),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: true,
                    channel_slugs: vec!["members".to_string()],
                    group_ids: vec![group_id],
                },
            )
            .await
            .expect("the exact active current channel should decide the union");

        assert_eq!(facts.channel_memberships, vec!["members".to_string()]);
        assert!(facts.group_memberships.is_empty());
        assert_eq!(
            *channel_calls
                .lock()
                .expect("channel call recorder should stay available"),
            vec!["members".to_string()]
        );
        assert_eq!(
            *group_calls
                .lock()
                .expect("group call recorder should stay available"),
            0
        );
    }

    #[tokio::test]
    async fn unrequested_current_channel_is_not_discovered_through_owner_reads() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let (adapter, channel_calls, _) = adapter(tenant_id, ["public"], []);

        let facts = adapter
            .resolve_forum_audience_facts(
                context(tenant_id, user_id, Some("public")),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: false,
                    channel_slugs: vec!["members".to_string()],
                    group_ids: Vec::new(),
                },
            )
            .await
            .expect("an unrequested current channel is an authoritative miss");

        assert!(facts.channel_memberships.is_empty());
        assert!(
            channel_calls
                .lock()
                .expect("channel call recorder should stay available")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn groups_are_consulted_only_after_channel_miss() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let active_group = Uuid::new_v4();
        let (adapter, _, group_calls) = adapter(tenant_id, [], [active_group]);

        let facts = adapter
            .resolve_forum_audience_facts(
                context(tenant_id, user_id, None),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: true,
                    channel_slugs: vec!["members".to_string()],
                    group_ids: vec![active_group],
                },
            )
            .await
            .expect("an active exact group should decide after a channel miss");

        assert_eq!(facts.group_memberships, vec![active_group]);
        assert_eq!(
            *group_calls
                .lock()
                .expect("group call recorder should stay available"),
            1
        );
    }

    #[tokio::test]
    async fn unresolved_trust_or_missing_optional_groups_remain_retryable() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let channels: SharedChannelReadPort = Arc::new(StaticChannelReadPort {
            tenant_id,
            active_slugs: BTreeSet::new(),
            calls: Arc::new(Mutex::new(Vec::new())),
        });
        let adapter = ServerForumAudienceFactsPort::new(channels, None);

        let error = adapter
            .resolve_forum_audience_facts(
                context(tenant_id, user_id, None),
                ForumAudienceFactsRequest {
                    tenant_id,
                    user_id,
                    include_trust_level: true,
                    channel_slugs: Vec::new(),
                    group_ids: vec![Uuid::new_v4()],
                },
            )
            .await
            .expect_err("unavailable dimensions must not become a false deny");

        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert!(error.retryable);
        assert_eq!(error.code, PARTIAL_PROVIDER_CODE);
    }
}
