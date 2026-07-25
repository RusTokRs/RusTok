use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_core::{SecurityContext, UserRole};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub const MAX_FORUM_AUDIENCE_ROLES: usize = 4;
pub const MAX_FORUM_AUDIENCE_CHANNELS: usize = 32;
pub const MAX_FORUM_AUDIENCE_GROUPS: usize = 32;
pub const MAX_FORUM_AUDIENCE_EXPLICIT_USERS: usize = 100;
pub const MAX_FORUM_AUDIENCE_TRUST_LEVEL: u8 = 100;
pub const FORUM_AUDIENCE_FACTS_CAPABILITY: &str = "forum_audience_facts";
pub const FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE: &str =
    "FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE";

const MAX_FORUM_AUDIENCE_CHANNEL_SLUG_LEN: usize = 128;

/// Additional audience narrowing applied after the inherited category
/// `public` / `authenticated` floor.
///
/// Positive selectors are a union: a matching role, trust threshold, channel
/// membership, group membership or explicit user allow is sufficient. Explicit
/// deny always wins. An empty constraint set adds no narrowing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumAudienceConstraints {
    pub roles_any: Vec<UserRole>,
    pub minimum_trust_level: Option<u8>,
    pub channel_members_any: Vec<String>,
    pub group_members_any: Vec<Uuid>,
    pub allow_user_ids: Vec<Uuid>,
    pub deny_user_ids: Vec<Uuid>,
}

impl ForumAudienceConstraints {
    pub fn normalize(mut self) -> ForumResult<Self> {
        validate_raw_len(
            self.roles_any.len(),
            MAX_FORUM_AUDIENCE_ROLES,
            "roles",
        )?;
        validate_raw_len(
            self.channel_members_any.len(),
            MAX_FORUM_AUDIENCE_CHANNELS,
            "channel memberships",
        )?;
        validate_raw_len(
            self.group_members_any.len(),
            MAX_FORUM_AUDIENCE_GROUPS,
            "group memberships",
        )?;
        validate_raw_len(
            self.allow_user_ids.len(),
            MAX_FORUM_AUDIENCE_EXPLICIT_USERS,
            "explicit allow users",
        )?;
        validate_raw_len(
            self.deny_user_ids.len(),
            MAX_FORUM_AUDIENCE_EXPLICIT_USERS,
            "explicit deny users",
        )?;
        if self.minimum_trust_level > Some(MAX_FORUM_AUDIENCE_TRUST_LEVEL) {
            return Err(ForumError::Validation(format!(
                "Forum audience trust level must not exceed {MAX_FORUM_AUDIENCE_TRUST_LEVEL}"
            )));
        }

        self.roles_any.sort_by_key(UserRole::privilege_rank);
        self.roles_any.dedup();
        self.channel_members_any = normalize_channel_slugs(self.channel_members_any)?;
        normalize_uuid_list(&mut self.group_members_any);
        normalize_uuid_list(&mut self.allow_user_ids);
        normalize_uuid_list(&mut self.deny_user_ids);
        Ok(self)
    }

    pub fn requires_owner_facts(&self) -> bool {
        self.minimum_trust_level.is_some()
            || !self.channel_members_any.is_empty()
            || !self.group_members_any.is_empty()
    }

    pub fn has_positive_selectors(&self) -> bool {
        !self.roles_any.is_empty()
            || self.minimum_trust_level.is_some()
            || !self.channel_members_any.is_empty()
            || !self.group_members_any.is_empty()
            || !self.allow_user_ids.is_empty()
    }
}

/// Exact bounded request to owner capability adapters.
///
/// Providers must resolve only the requested actor and candidate memberships;
/// they must not discover or return additional channels or groups.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumAudienceFactsRequest {
    pub user_id: Uuid,
    pub include_trust_level: bool,
    pub channel_slugs: Vec<String>,
    pub group_ids: Vec<Uuid>,
}

impl ForumAudienceFactsRequest {
    pub fn for_constraints(
        user_id: Uuid,
        constraints: &ForumAudienceConstraints,
    ) -> ForumResult<Self> {
        let constraints = constraints.clone().normalize()?;
        Ok(Self {
            user_id,
            include_trust_level: constraints.minimum_trust_level.is_some(),
            channel_slugs: constraints.channel_members_any,
            group_ids: constraints.group_members_any,
        })
    }
}

/// Exact owner facts for one actor and one bounded request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumAudienceFacts {
    pub trust_level: Option<u8>,
    pub channel_memberships: Vec<String>,
    pub group_memberships: Vec<Uuid>,
}

impl ForumAudienceFacts {
    pub fn validate_for_request(
        mut self,
        request: &ForumAudienceFactsRequest,
    ) -> ForumResult<Self> {
        validate_raw_len(
            self.channel_memberships.len(),
            MAX_FORUM_AUDIENCE_CHANNELS,
            "resolved channel memberships",
        )?;
        validate_raw_len(
            self.group_memberships.len(),
            MAX_FORUM_AUDIENCE_GROUPS,
            "resolved group memberships",
        )?;
        if self.trust_level > Some(MAX_FORUM_AUDIENCE_TRUST_LEVEL) {
            return Err(ForumError::Validation(format!(
                "Forum audience resolved trust level must not exceed {MAX_FORUM_AUDIENCE_TRUST_LEVEL}"
            )));
        }
        if !request.include_trust_level && self.trust_level.is_some() {
            return Err(ForumError::Validation(
                "Forum audience facts returned an unrequested trust level".to_string(),
            ));
        }

        self.channel_memberships = normalize_channel_slugs(self.channel_memberships)?;
        normalize_uuid_list(&mut self.group_memberships);

        let requested_channels = request
            .channel_slugs
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        if self
            .channel_memberships
            .iter()
            .any(|slug| !requested_channels.contains(slug))
        {
            return Err(ForumError::Validation(
                "Forum audience facts returned an unrequested channel membership".to_string(),
            ));
        }

        let requested_groups = request.group_ids.iter().copied().collect::<HashSet<_>>();
        if self
            .group_memberships
            .iter()
            .any(|group_id| !requested_groups.contains(group_id))
        {
            return Err(ForumError::Validation(
                "Forum audience facts returned an unrequested group membership".to_string(),
            ));
        }
        Ok(self)
    }
}

/// Optional adapter boundary for trust/channel/group owner facts.
#[async_trait]
pub trait ForumAudienceFactsPort: Send + Sync {
    async fn resolve_forum_audience_facts(
        &self,
        context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError>;
}

pub type SharedForumAudienceFactsPort = Arc<dyn ForumAudienceFactsPort>;

/// Fail-closed capability composition. Provider absence is harmless for rules
/// that need only local role/explicit-user facts, but is a typed error for an
/// authenticated actor when trust or membership facts are required.
#[derive(Clone, Default)]
pub struct ForumAudienceFactsResolver {
    port: Option<SharedForumAudienceFactsPort>,
}

impl ForumAudienceFactsResolver {
    pub fn new(port: Option<SharedForumAudienceFactsPort>) -> Self {
        Self { port }
    }

    pub async fn resolve_for_constraints(
        &self,
        context: PortContext,
        security: &SecurityContext,
        constraints: &ForumAudienceConstraints,
    ) -> ForumResult<ForumAudienceFacts> {
        let constraints = constraints.clone().normalize()?;
        if !constraints.requires_owner_facts() {
            return Ok(ForumAudienceFacts::default());
        }

        let Some(user_id) = security.user_id else {
            return Ok(ForumAudienceFacts::default());
        };
        let request = ForumAudienceFactsRequest::for_constraints(user_id, &constraints)?;
        let Some(port) = &self.port else {
            return Err(ForumError::capability_unavailable(
                FORUM_AUDIENCE_FACTS_CAPABILITY,
                FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE,
            ));
        };

        context
            .require_policy(PortCallPolicy::read())
            .map_err(map_audience_port_error)?;
        port.resolve_forum_audience_facts(context, request.clone())
            .await
            .map_err(map_audience_port_error)?
            .validate_for_request(&request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumAudienceDecisionReason {
    Unrestricted,
    ExplicitDeny,
    ExplicitAllow,
    Role,
    TrustLevel,
    ChannelMembership,
    GroupMembership,
    AuthenticationRequired,
    NoMatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumAudienceDecision {
    pub allowed: bool,
    pub reason: ForumAudienceDecisionReason,
}

impl ForumAudienceDecision {
    const fn allow(reason: ForumAudienceDecisionReason) -> Self {
        Self {
            allowed: true,
            reason,
        }
    }

    const fn deny(reason: ForumAudienceDecisionReason) -> Self {
        Self {
            allowed: false,
            reason,
        }
    }
}

pub struct ForumAudienceEvaluator;

impl ForumAudienceEvaluator {
    pub fn decide(
        constraints: &ForumAudienceConstraints,
        security: &SecurityContext,
        facts: &ForumAudienceFacts,
    ) -> ForumResult<ForumAudienceDecision> {
        let constraints = constraints.clone().normalize()?;
        let user_id = security.user_id;

        if user_id.is_some_and(|id| constraints.deny_user_ids.binary_search(&id).is_ok()) {
            return Ok(ForumAudienceDecision::deny(
                ForumAudienceDecisionReason::ExplicitDeny,
            ));
        }
        if user_id.is_some_and(|id| constraints.allow_user_ids.binary_search(&id).is_ok()) {
            return Ok(ForumAudienceDecision::allow(
                ForumAudienceDecisionReason::ExplicitAllow,
            ));
        }
        if !constraints.has_positive_selectors() {
            return Ok(ForumAudienceDecision::allow(
                ForumAudienceDecisionReason::Unrestricted,
            ));
        }
        if security.is_public_read() {
            return Ok(ForumAudienceDecision::deny(
                ForumAudienceDecisionReason::AuthenticationRequired,
            ));
        }
        if constraints.roles_any.contains(&security.role) {
            return Ok(ForumAudienceDecision::allow(
                ForumAudienceDecisionReason::Role,
            ));
        }
        if constraints
            .minimum_trust_level
            .is_some_and(|minimum| facts.trust_level.is_some_and(|level| level >= minimum))
        {
            return Ok(ForumAudienceDecision::allow(
                ForumAudienceDecisionReason::TrustLevel,
            ));
        }
        if intersects_strings(
            &constraints.channel_members_any,
            &facts.channel_memberships,
        ) {
            return Ok(ForumAudienceDecision::allow(
                ForumAudienceDecisionReason::ChannelMembership,
            ));
        }
        if intersects_uuids(&constraints.group_members_any, &facts.group_memberships) {
            return Ok(ForumAudienceDecision::allow(
                ForumAudienceDecisionReason::GroupMembership,
            ));
        }

        Ok(ForumAudienceDecision::deny(
            ForumAudienceDecisionReason::NoMatch,
        ))
    }
}

fn validate_raw_len(actual: usize, maximum: usize, label: &str) -> ForumResult<()> {
    if actual > maximum {
        return Err(ForumError::Validation(format!(
            "Forum audience {label} must not exceed {maximum} candidates"
        )));
    }
    Ok(())
}

fn normalize_channel_slugs(slugs: Vec<String>) -> ForumResult<Vec<String>> {
    let mut normalized = Vec::with_capacity(slugs.len());
    let mut seen = HashSet::with_capacity(slugs.len());
    for slug in slugs {
        let slug = slug.trim();
        if slug.is_empty() {
            return Err(ForumError::Validation(
                "Forum audience channel slug must not be empty".to_string(),
            ));
        }
        if slug.chars().count() > MAX_FORUM_AUDIENCE_CHANNEL_SLUG_LEN {
            return Err(ForumError::Validation(format!(
                "Forum audience channel slug must not exceed {MAX_FORUM_AUDIENCE_CHANNEL_SLUG_LEN} characters"
            )));
        }
        let slug = slug.to_ascii_lowercase();
        if seen.insert(slug.clone()) {
            normalized.push(slug);
        }
    }
    normalized.sort();
    Ok(normalized)
}

fn normalize_uuid_list(ids: &mut Vec<Uuid>) {
    ids.sort_unstable();
    ids.dedup();
}

fn intersects_strings(left: &[String], right: &[String]) -> bool {
    left.iter().any(|candidate| right.contains(candidate))
}

fn intersects_uuids(left: &[Uuid], right: &[Uuid]) -> bool {
    left.iter().any(|candidate| right.contains(candidate))
}

fn map_audience_port_error(error: PortError) -> ForumError {
    ForumError::capability_failure(
        FORUM_AUDIENCE_FACTS_CAPABILITY,
        error.code,
        error.message,
        error.retryable,
    )
}
