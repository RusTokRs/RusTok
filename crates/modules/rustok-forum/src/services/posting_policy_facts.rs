use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audience::{ForumAudienceFactsRequest, SharedForumAudienceFactsPort};
use crate::error::{ForumError, ForumResult};

use super::posting_policy::{
    ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyEvaluationInput,
    ForumPostingPolicyFactKind, ForumPostingPolicyFacts, ForumPostingPolicyUnavailableFact,
    ForumPostingWindowCount,
};
use super::posting_policy_evaluator::ForumPostingPolicyRules;
use super::user_trust::MAX_FORUM_USER_TRUST_LEVEL;

pub const FORUM_POSTING_POLICY_FACTS_CAPABILITY: &str = "forum_posting_policy_facts";
pub const FORUM_POSTING_POLICY_FACTS_CAPABILITY_UNAVAILABLE: &str =
    "FORUM_POSTING_POLICY_FACTS_CAPABILITY_UNAVAILABLE";

const INVALID_REQUEST_CODE: &str = "forum.posting_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.posting_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.posting_facts.actor_mismatch";
const DUPLICATE_PROVIDER_CODE: &str = "forum.posting_facts.duplicate_provider";
const PROVIDER_RESPONSE_CODE: &str = "forum.posting_facts.provider_response_invalid";
const TRUST_PROVIDER_RESPONSE_CODE: &str = "forum.posting_facts.trust_response_invalid";
const PROVIDER_MISSING_REASON_CODE: &str = "forum.posting_fact.provider_missing";
const PROVIDER_ERROR_REASON_CODE: &str = "forum.posting_fact.provider_error";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyCompositionRequest {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub action: ForumPostingAction,
    pub candidate: ForumPostingCandidateMetrics,
}

impl ForumPostingPolicyCompositionRequest {
    pub fn normalize(self) -> ForumResult<Self> {
        if self.tenant_id.is_nil() || self.user_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum posting fact composition requires non-nil tenant and user identities"
                    .to_string(),
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyOwnerFactRequest {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub action: ForumPostingAction,
    pub fact: ForumPostingPolicyFactKind,
    pub window_seconds: Option<u32>,
}

impl ForumPostingPolicyOwnerFactRequest {
    fn for_rules(
        request: ForumPostingPolicyCompositionRequest,
        rules: &ForumPostingPolicyRules,
        fact: ForumPostingPolicyFactKind,
    ) -> ForumResult<Self> {
        let window_seconds = match fact {
            ForumPostingPolicyFactKind::TopicCreatesWindow => {
                rules.topic_create_limit.map(|limit| limit.window_seconds)
            }
            ForumPostingPolicyFactKind::ReplyCreatesWindow => {
                rules.reply_create_limit.map(|limit| limit.window_seconds)
            }
            ForumPostingPolicyFactKind::EditsWindow => {
                rules.edit_limit.map(|limit| limit.window_seconds)
            }
            ForumPostingPolicyFactKind::TrustLevel
            | ForumPostingPolicyFactKind::AccountAgeSeconds
            | ForumPostingPolicyFactKind::TopicsRead
            | ForumPostingPolicyFactKind::ApprovedPosts
            | ForumPostingPolicyFactKind::ActiveFlags
            | ForumPostingPolicyFactKind::Reputation
            | ForumPostingPolicyFactKind::RecentModerationActions
            | ForumPostingPolicyFactKind::SecondsSinceLastBump => None,
        };

        Self {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            action: request.action,
            fact,
            window_seconds,
        }
        .normalize()
    }

    pub fn normalize(self) -> ForumResult<Self> {
        if self.tenant_id.is_nil() || self.user_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum posting owner fact request requires non-nil tenant and user identities"
                    .to_string(),
            ));
        }

        let window_fact = matches!(
            self.fact,
            ForumPostingPolicyFactKind::TopicCreatesWindow
                | ForumPostingPolicyFactKind::ReplyCreatesWindow
                | ForumPostingPolicyFactKind::EditsWindow
        );
        if window_fact != self.window_seconds.is_some() {
            return Err(ForumError::Validation(
                "Forum posting usage facts require exactly one configured observation window"
                    .to_string(),
            ));
        }
        if self.window_seconds == Some(0) {
            return Err(ForumError::Validation(
                "Forum posting usage fact window must be greater than zero".to_string(),
            ));
        }
        if !fact_supports_action(self.fact, self.action) {
            return Err(ForumError::Validation(
                "Forum posting owner fact does not apply to the requested action".to_string(),
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ForumPostingPolicyOwnerFactValue {
    TrustLevel(u8),
    AccountAgeSeconds(u64),
    TopicsRead(u64),
    ApprovedPosts(u64),
    ActiveFlags(u32),
    Reputation(i64),
    RecentModerationActions(u32),
    TopicCreatesWindow(ForumPostingWindowCount),
    ReplyCreatesWindow(ForumPostingWindowCount),
    EditsWindow(ForumPostingWindowCount),
    SecondsSinceLastBump(u64),
}

impl ForumPostingPolicyOwnerFactValue {
    pub const fn fact_kind(self) -> ForumPostingPolicyFactKind {
        match self {
            Self::TrustLevel(_) => ForumPostingPolicyFactKind::TrustLevel,
            Self::AccountAgeSeconds(_) => ForumPostingPolicyFactKind::AccountAgeSeconds,
            Self::TopicsRead(_) => ForumPostingPolicyFactKind::TopicsRead,
            Self::ApprovedPosts(_) => ForumPostingPolicyFactKind::ApprovedPosts,
            Self::ActiveFlags(_) => ForumPostingPolicyFactKind::ActiveFlags,
            Self::Reputation(_) => ForumPostingPolicyFactKind::Reputation,
            Self::RecentModerationActions(_) => ForumPostingPolicyFactKind::RecentModerationActions,
            Self::TopicCreatesWindow(_) => ForumPostingPolicyFactKind::TopicCreatesWindow,
            Self::ReplyCreatesWindow(_) => ForumPostingPolicyFactKind::ReplyCreatesWindow,
            Self::EditsWindow(_) => ForumPostingPolicyFactKind::EditsWindow,
            Self::SecondsSinceLastBump(_) => ForumPostingPolicyFactKind::SecondsSinceLastBump,
        }
    }

    fn normalize(self, request: &ForumPostingPolicyOwnerFactRequest) -> ForumResult<Self> {
        if self.fact_kind() != request.fact {
            return Err(ForumError::Validation(
                "Forum posting owner fact value does not match the requested fact".to_string(),
            ));
        }
        match self {
            Self::TrustLevel(level) if level > MAX_FORUM_USER_TRUST_LEVEL => {
                Err(ForumError::Validation(format!(
                    "Forum posting trust fact must not exceed {MAX_FORUM_USER_TRUST_LEVEL}"
                )))
            }
            Self::TopicCreatesWindow(window)
            | Self::ReplyCreatesWindow(window)
            | Self::EditsWindow(window) => {
                let window = window.normalize()?;
                if request.window_seconds != Some(window.window_seconds) {
                    return Err(ForumError::Validation(
                        "Forum posting owner fact window does not match the exact request"
                            .to_string(),
                    ));
                }
                Ok(match self {
                    Self::TopicCreatesWindow(_) => Self::TopicCreatesWindow(window),
                    Self::ReplyCreatesWindow(_) => Self::ReplyCreatesWindow(window),
                    Self::EditsWindow(_) => Self::EditsWindow(window),
                    _ => unreachable!("matched window variants above"),
                })
            }
            _ => Ok(self),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyOwnerFactResponse {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub action: ForumPostingAction,
    pub fact: ForumPostingPolicyFactKind,
    pub value: ForumPostingPolicyOwnerFactValue,
}

impl ForumPostingPolicyOwnerFactResponse {
    pub fn validate_for_request(
        mut self,
        request: &ForumPostingPolicyOwnerFactRequest,
    ) -> ForumResult<Self> {
        let request = request.normalize()?;
        if self.tenant_id != request.tenant_id
            || self.user_id != request.user_id
            || self.action != request.action
            || self.fact != request.fact
        {
            return Err(ForumError::Validation(
                "Forum posting owner fact returned a different request identity".to_string(),
            ));
        }
        self.value = self.value.normalize(&request)?;
        Ok(self)
    }
}

#[async_trait]
pub trait ForumPostingPolicyOwnerFactPort: Send + Sync {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind;

    async fn resolve_forum_posting_policy_fact(
        &self,
        context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError>;
}

pub type SharedForumPostingPolicyOwnerFactPort = Arc<dyn ForumPostingPolicyOwnerFactPort>;

#[derive(Clone, Default)]
pub struct ForumPostingPolicyFactsComposer {
    providers: Vec<SharedForumPostingPolicyOwnerFactPort>,
}

impl ForumPostingPolicyFactsComposer {
    pub fn new(
        mut providers: Vec<SharedForumPostingPolicyOwnerFactPort>,
    ) -> Result<Self, PortError> {
        providers.sort_by_key(|provider| provider.fact_kind());
        if providers
            .windows(2)
            .any(|pair| pair[0].fact_kind() == pair[1].fact_kind())
        {
            return Err(PortError::conflict(
                DUPLICATE_PROVIDER_CODE,
                "Forum posting policy fact providers must be unique by fact kind",
            ));
        }
        Ok(Self { providers })
    }

    pub fn with_trust_audience_facts(audience_facts: SharedForumAudienceFactsPort) -> Self {
        Self {
            providers: vec![Arc::new(ForumPostingTrustFactPort::new(audience_facts))],
        }
    }

    pub async fn compose(
        &self,
        context: PortContext,
        rules: &ForumPostingPolicyRules,
        request: ForumPostingPolicyCompositionRequest,
    ) -> Result<ForumPostingPolicyEvaluationInput, PortError> {
        let request = request.normalize().map_err(|_| {
            PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum posting fact composition request is invalid",
            )
        })?;
        validate_context(&context, request.tenant_id, request.user_id)?;
        let rules = rules.clone().normalize().map_err(|_| {
            PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum posting policy rules are invalid for fact composition",
            )
        })?;
        let required_facts = rules.required_facts(request.action).map_err(|_| {
            PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum posting policy required facts could not be derived",
            )
        })?;
        let mut facts = ForumPostingPolicyFacts {
            required_facts: required_facts.clone(),
            ..ForumPostingPolicyFacts::default()
        };

        for fact in required_facts {
            let owner_request = ForumPostingPolicyOwnerFactRequest::for_rules(
                request, &rules, fact,
            )
            .map_err(|_| {
                PortError::invariant_violation(
                    PROVIDER_RESPONSE_CODE,
                    "Forum posting policy produced an invalid owner fact request",
                )
            })?;
            let Some(provider) = self.provider_for(fact) else {
                facts
                    .unavailable_facts
                    .push(ForumPostingPolicyUnavailableFact {
                        fact,
                        retryable: false,
                        reason_code: PROVIDER_MISSING_REASON_CODE.to_string(),
                    });
                continue;
            };

            match provider
                .resolve_forum_posting_policy_fact(context.clone(), owner_request)
                .await
            {
                Ok(response) => {
                    let response = response.validate_for_request(&owner_request).map_err(|_| {
                        PortError::invariant_violation(
                            PROVIDER_RESPONSE_CODE,
                            "Forum posting policy owner fact response is invalid",
                        )
                    })?;
                    apply_value(&mut facts, response.value);
                }
                Err(error) if capability_error(&error.kind) => {
                    facts
                        .unavailable_facts
                        .push(unavailable_from_error(fact, error));
                }
                Err(error) => return Err(error),
            }
        }

        ForumPostingPolicyEvaluationInput {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            action: request.action,
            candidate: request.candidate,
            facts,
        }
        .normalize()
        .map_err(|_| {
            PortError::invariant_violation(
                PROVIDER_RESPONSE_CODE,
                "Forum posting policy fact composition produced an invalid input",
            )
        })
    }

    fn provider_for(
        &self,
        fact: ForumPostingPolicyFactKind,
    ) -> Option<&SharedForumPostingPolicyOwnerFactPort> {
        self.providers
            .binary_search_by_key(&fact, |provider| provider.fact_kind())
            .ok()
            .map(|index| &self.providers[index])
    }
}

#[derive(Clone)]
pub struct ForumPostingTrustFactPort {
    audience_facts: SharedForumAudienceFactsPort,
}

impl ForumPostingTrustFactPort {
    pub fn new(audience_facts: SharedForumAudienceFactsPort) -> Self {
        Self { audience_facts }
    }

    pub fn shared(
        audience_facts: SharedForumAudienceFactsPort,
    ) -> SharedForumPostingPolicyOwnerFactPort {
        Arc::new(Self::new(audience_facts))
    }
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ForumPostingTrustFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        ForumPostingPolicyFactKind::TrustLevel
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        let request = request.normalize().map_err(|_| {
            PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum posting trust fact request is invalid",
            )
        })?;
        validate_context(&context, request.tenant_id, request.user_id)?;
        if request.fact != ForumPostingPolicyFactKind::TrustLevel {
            return Err(PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum posting trust adapter accepts only trust-level facts",
            ));
        }

        let audience_request = ForumAudienceFactsRequest {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            include_trust_level: true,
            channel_slugs: Vec::new(),
            group_ids: Vec::new(),
        };
        let audience_facts = self
            .audience_facts
            .resolve_forum_audience_facts(context, audience_request.clone())
            .await?
            .validate_for_request(&audience_request)
            .map_err(|_| {
                PortError::invariant_violation(
                    TRUST_PROVIDER_RESPONSE_CODE,
                    "Forum authoritative trust adapter returned an invalid response",
                )
            })?;
        let trust_level = audience_facts.trust_level.ok_or_else(|| {
            PortError::invariant_violation(
                TRUST_PROVIDER_RESPONSE_CODE,
                "Forum authoritative trust adapter omitted the requested trust level",
            )
        })?;

        Ok(ForumPostingPolicyOwnerFactResponse {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            action: request.action,
            fact: request.fact,
            value: ForumPostingPolicyOwnerFactValue::TrustLevel(trust_level),
        })
    }
}

fn validate_context(
    context: &PortContext,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    if context.tenant_id != tenant_id.to_string() {
        return Err(PortError::validation(
            TENANT_MISMATCH_CODE,
            "Forum posting facts tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum posting facts require the exact requested user actor",
        ));
    }
    Ok(())
}

fn fact_supports_action(fact: ForumPostingPolicyFactKind, action: ForumPostingAction) -> bool {
    match fact {
        ForumPostingPolicyFactKind::TopicCreatesWindow => action == ForumPostingAction::CreateTopic,
        ForumPostingPolicyFactKind::ReplyCreatesWindow => action == ForumPostingAction::CreateReply,
        ForumPostingPolicyFactKind::EditsWindow => matches!(
            action,
            ForumPostingAction::EditTopic | ForumPostingAction::EditReply
        ),
        ForumPostingPolicyFactKind::SecondsSinceLastBump => action == ForumPostingAction::BumpTopic,
        ForumPostingPolicyFactKind::TrustLevel
        | ForumPostingPolicyFactKind::AccountAgeSeconds
        | ForumPostingPolicyFactKind::TopicsRead
        | ForumPostingPolicyFactKind::ApprovedPosts
        | ForumPostingPolicyFactKind::ActiveFlags
        | ForumPostingPolicyFactKind::Reputation
        | ForumPostingPolicyFactKind::RecentModerationActions => true,
    }
}

fn capability_error(kind: &PortErrorKind) -> bool {
    matches!(
        kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::NotFound
    )
}

fn unavailable_from_error(
    fact: ForumPostingPolicyFactKind,
    error: PortError,
) -> ForumPostingPolicyUnavailableFact {
    let candidate = ForumPostingPolicyUnavailableFact {
        fact,
        retryable: error.retryable,
        reason_code: error.code,
    };
    candidate
        .normalize()
        .unwrap_or(ForumPostingPolicyUnavailableFact {
            fact,
            retryable: error.retryable,
            reason_code: PROVIDER_ERROR_REASON_CODE.to_string(),
        })
}

fn apply_value(facts: &mut ForumPostingPolicyFacts, value: ForumPostingPolicyOwnerFactValue) {
    match value {
        ForumPostingPolicyOwnerFactValue::TrustLevel(value) => facts.trust_level = Some(value),
        ForumPostingPolicyOwnerFactValue::AccountAgeSeconds(value) => {
            facts.account_age_seconds = Some(value)
        }
        ForumPostingPolicyOwnerFactValue::TopicsRead(value) => facts.topics_read = Some(value),
        ForumPostingPolicyOwnerFactValue::ApprovedPosts(value) => {
            facts.approved_posts = Some(value)
        }
        ForumPostingPolicyOwnerFactValue::ActiveFlags(value) => facts.active_flags = Some(value),
        ForumPostingPolicyOwnerFactValue::Reputation(value) => facts.reputation = Some(value),
        ForumPostingPolicyOwnerFactValue::RecentModerationActions(value) => {
            facts.recent_moderation_actions = Some(value)
        }
        ForumPostingPolicyOwnerFactValue::TopicCreatesWindow(value) => {
            facts.topic_creates_window = Some(value)
        }
        ForumPostingPolicyOwnerFactValue::ReplyCreatesWindow(value) => {
            facts.reply_creates_window = Some(value)
        }
        ForumPostingPolicyOwnerFactValue::EditsWindow(value) => facts.edits_window = Some(value),
        ForumPostingPolicyOwnerFactValue::SecondsSinceLastBump(value) => {
            facts.seconds_since_last_bump = Some(value)
        }
    }
}
