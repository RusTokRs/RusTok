use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audience::MAX_FORUM_AUDIENCE_TRUST_LEVEL;
use crate::error::{ForumError, ForumResult};

pub const MAX_FORUM_POSTING_POLICY_FACTS: usize = 11;
pub const MAX_FORUM_POSTING_UNAVAILABLE_REASON_CODE_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumPostingAction {
    CreateTopic,
    CreateReply,
    EditTopic,
    EditReply,
    BumpTopic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumPostingPolicyFactKind {
    TrustLevel,
    AccountAgeSeconds,
    TopicsRead,
    ApprovedPosts,
    ActiveFlags,
    Reputation,
    RecentModerationActions,
    TopicCreatesWindow,
    ReplyCreatesWindow,
    EditsWindow,
    SecondsSinceLastBump,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingWindowCount {
    pub count: u32,
    pub window_seconds: u32,
}

impl ForumPostingWindowCount {
    pub fn normalize(self) -> ForumResult<Self> {
        if self.window_seconds == 0 {
            return Err(ForumError::Validation(
                "Forum posting policy observation window must be greater than zero".to_string(),
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyUnavailableFact {
    pub fact: ForumPostingPolicyFactKind,
    pub retryable: bool,
    pub reason_code: String,
}

impl ForumPostingPolicyUnavailableFact {
    pub fn normalize(mut self) -> ForumResult<Self> {
        self.reason_code = normalize_reason_code(self.reason_code)?;
        Ok(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyFacts {
    pub required_facts: Vec<ForumPostingPolicyFactKind>,
    pub unavailable_facts: Vec<ForumPostingPolicyUnavailableFact>,
    pub trust_level: Option<u8>,
    pub account_age_seconds: Option<u64>,
    pub topics_read: Option<u64>,
    pub approved_posts: Option<u64>,
    pub active_flags: Option<u32>,
    pub reputation: Option<i64>,
    pub recent_moderation_actions: Option<u32>,
    pub topic_creates_window: Option<ForumPostingWindowCount>,
    pub reply_creates_window: Option<ForumPostingWindowCount>,
    pub edits_window: Option<ForumPostingWindowCount>,
    pub seconds_since_last_bump: Option<u64>,
}

impl ForumPostingPolicyFacts {
    pub fn normalize(mut self) -> ForumResult<Self> {
        if self.required_facts.len() > MAX_FORUM_POSTING_POLICY_FACTS
            || self.unavailable_facts.len() > MAX_FORUM_POSTING_POLICY_FACTS
        {
            return Err(ForumError::Validation(format!(
                "Forum posting policy facts must not exceed {MAX_FORUM_POSTING_POLICY_FACTS} entries"
            )));
        }

        self.required_facts.sort_unstable();
        reject_duplicate_fact_kinds(&self.required_facts, "required facts")?;

        self.unavailable_facts = self
            .unavailable_facts
            .into_iter()
            .map(ForumPostingPolicyUnavailableFact::normalize)
            .collect::<ForumResult<Vec<_>>>()?;
        self.unavailable_facts.sort_by_key(|item| item.fact);
        reject_duplicate_unavailable_facts(&self.unavailable_facts)?;

        if self.trust_level > Some(MAX_FORUM_AUDIENCE_TRUST_LEVEL) {
            return Err(ForumError::Validation(format!(
                "Forum posting policy trust level must not exceed {MAX_FORUM_AUDIENCE_TRUST_LEVEL}"
            )));
        }
        self.topic_creates_window = self
            .topic_creates_window
            .map(ForumPostingWindowCount::normalize)
            .transpose()?;
        self.reply_creates_window = self
            .reply_creates_window
            .map(ForumPostingWindowCount::normalize)
            .transpose()?;
        self.edits_window = self
            .edits_window
            .map(ForumPostingWindowCount::normalize)
            .transpose()?;

        for fact in ALL_FACT_KINDS {
            let required = self.required_facts.binary_search(&fact).is_ok();
            let available = self.has_available_fact(fact);
            let unavailable = self
                .unavailable_facts
                .binary_search_by_key(&fact, |item| item.fact)
                .is_ok();

            if required && available == unavailable {
                return Err(ForumError::Validation(format!(
                    "Forum posting policy required fact {fact:?} must be exactly one of available or unavailable"
                )));
            }
            if !required && (available || unavailable) {
                return Err(ForumError::Validation(format!(
                    "Forum posting policy fact {fact:?} must be declared required before it is supplied"
                )));
            }
        }

        Ok(self)
    }

    fn has_available_fact(&self, fact: ForumPostingPolicyFactKind) -> bool {
        match fact {
            ForumPostingPolicyFactKind::TrustLevel => self.trust_level.is_some(),
            ForumPostingPolicyFactKind::AccountAgeSeconds => self.account_age_seconds.is_some(),
            ForumPostingPolicyFactKind::TopicsRead => self.topics_read.is_some(),
            ForumPostingPolicyFactKind::ApprovedPosts => self.approved_posts.is_some(),
            ForumPostingPolicyFactKind::ActiveFlags => self.active_flags.is_some(),
            ForumPostingPolicyFactKind::Reputation => self.reputation.is_some(),
            ForumPostingPolicyFactKind::RecentModerationActions => {
                self.recent_moderation_actions.is_some()
            }
            ForumPostingPolicyFactKind::TopicCreatesWindow => self.topic_creates_window.is_some(),
            ForumPostingPolicyFactKind::ReplyCreatesWindow => self.reply_creates_window.is_some(),
            ForumPostingPolicyFactKind::EditsWindow => self.edits_window.is_some(),
            ForumPostingPolicyFactKind::SecondsSinceLastBump => {
                self.seconds_since_last_bump.is_some()
            }
        }
    }
}

const ALL_FACT_KINDS: [ForumPostingPolicyFactKind; MAX_FORUM_POSTING_POLICY_FACTS] = [
    ForumPostingPolicyFactKind::TrustLevel,
    ForumPostingPolicyFactKind::AccountAgeSeconds,
    ForumPostingPolicyFactKind::TopicsRead,
    ForumPostingPolicyFactKind::ApprovedPosts,
    ForumPostingPolicyFactKind::ActiveFlags,
    ForumPostingPolicyFactKind::Reputation,
    ForumPostingPolicyFactKind::RecentModerationActions,
    ForumPostingPolicyFactKind::TopicCreatesWindow,
    ForumPostingPolicyFactKind::ReplyCreatesWindow,
    ForumPostingPolicyFactKind::EditsWindow,
    ForumPostingPolicyFactKind::SecondsSinceLastBump,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingCandidateMetrics {
    pub body_bytes: u32,
    pub link_count: u16,
    pub mention_count: u16,
    pub attachment_count: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyEvaluationInput {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub action: ForumPostingAction,
    pub candidate: ForumPostingCandidateMetrics,
    pub facts: ForumPostingPolicyFacts,
}

impl ForumPostingPolicyEvaluationInput {
    pub fn normalize(mut self) -> ForumResult<Self> {
        if self.tenant_id.is_nil() || self.user_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum posting policy tenant and user identities must be non-nil".to_string(),
            ));
        }
        self.facts = self.facts.normalize()?;
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumPostingPolicyOutcome {
    Allowed,
    Denied,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumPostingPolicyDecisionReason {
    Allowed,
    RequiredFactUnavailable,
    TrustLevel,
    AccountAge,
    ReadingActivity,
    ApprovedPosts,
    ActiveFlags,
    Reputation,
    ModerationHistory,
    TopicRateLimit,
    ReplyRateLimit,
    LinkLimit,
    MentionLimit,
    AttachmentLimit,
    EditRateLimit,
    BumpInterval,
    DuplicateContent,
    ExternalSpamScore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumPostingPolicyMeasureUnit {
    Count,
    Seconds,
    TrustLevel,
    Reputation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyEvidence {
    pub observed: i64,
    pub threshold: i64,
    pub unit: ForumPostingPolicyMeasureUnit,
}

impl ForumPostingPolicyEvidence {
    fn validate(self) -> ForumResult<Self> {
        match self.unit {
            ForumPostingPolicyMeasureUnit::Count | ForumPostingPolicyMeasureUnit::Seconds => {
                if self.observed < 0 || self.threshold < 0 {
                    return Err(ForumError::Validation(
                        "Forum posting policy count and duration evidence must not be negative"
                            .to_string(),
                    ));
                }
            }
            ForumPostingPolicyMeasureUnit::TrustLevel => {
                let maximum = i64::from(MAX_FORUM_AUDIENCE_TRUST_LEVEL);
                if self.observed < 0
                    || self.threshold < 0
                    || self.observed > maximum
                    || self.threshold > maximum
                {
                    return Err(ForumError::Validation(format!(
                        "Forum posting policy trust evidence must be between 0 and {MAX_FORUM_AUDIENCE_TRUST_LEVEL}"
                    )));
                }
            }
            ForumPostingPolicyMeasureUnit::Reputation => {}
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyDecision {
    pub outcome: ForumPostingPolicyOutcome,
    pub reason: ForumPostingPolicyDecisionReason,
    pub fact: Option<ForumPostingPolicyFactKind>,
    pub evidence: Option<ForumPostingPolicyEvidence>,
    pub retry_after_seconds: Option<u64>,
    pub retryable: bool,
}

impl ForumPostingPolicyDecision {
    pub const fn allowed() -> Self {
        Self {
            outcome: ForumPostingPolicyOutcome::Allowed,
            reason: ForumPostingPolicyDecisionReason::Allowed,
            fact: None,
            evidence: None,
            retry_after_seconds: None,
            retryable: false,
        }
    }

    pub fn denied(
        reason: ForumPostingPolicyDecisionReason,
        evidence: Option<ForumPostingPolicyEvidence>,
        retry_after_seconds: Option<u64>,
    ) -> ForumResult<Self> {
        Self {
            outcome: ForumPostingPolicyOutcome::Denied,
            reason,
            fact: expected_fact(reason),
            evidence,
            retry_after_seconds,
            retryable: false,
        }
        .normalize()
    }

    pub const fn indeterminate(fact: ForumPostingPolicyFactKind, retryable: bool) -> Self {
        Self {
            outcome: ForumPostingPolicyOutcome::Indeterminate,
            reason: ForumPostingPolicyDecisionReason::RequiredFactUnavailable,
            fact: Some(fact),
            evidence: None,
            retry_after_seconds: None,
            retryable,
        }
    }

    pub fn normalize(self) -> ForumResult<Self> {
        match self.outcome {
            ForumPostingPolicyOutcome::Allowed => self.validate_allowed(),
            ForumPostingPolicyOutcome::Denied => self.validate_denied(),
            ForumPostingPolicyOutcome::Indeterminate => self.validate_indeterminate(),
        }
    }

    fn validate_allowed(self) -> ForumResult<Self> {
        if self != Self::allowed() {
            return Err(ForumError::Validation(
                "Forum posting policy allowed decisions cannot carry denial or retry metadata"
                    .to_string(),
            ));
        }
        Ok(self)
    }

    fn validate_denied(self) -> ForumResult<Self> {
        if matches!(
            self.reason,
            ForumPostingPolicyDecisionReason::Allowed
                | ForumPostingPolicyDecisionReason::RequiredFactUnavailable
        ) || self.retryable
        {
            return Err(ForumError::Validation(
                "Forum posting policy denied decision has an invalid reason or retryability"
                    .to_string(),
            ));
        }

        if self.fact != expected_fact(self.reason) {
            return Err(ForumError::Validation(
                "Forum posting policy denied decision fact does not match its reason".to_string(),
            ));
        }

        match expected_evidence_unit(self.reason) {
            Some(expected_unit) => {
                let evidence = self.evidence.ok_or_else(|| {
                    ForumError::Validation(
                        "Forum posting policy denied decision requires typed evidence".to_string(),
                    )
                })?;
                if evidence.unit != expected_unit {
                    return Err(ForumError::Validation(
                        "Forum posting policy denied decision evidence unit does not match its reason"
                            .to_string(),
                    ));
                }
                evidence.validate()?;
            }
            None if self.evidence.is_some() => {
                return Err(ForumError::Validation(
                    "Forum posting policy decision reason does not accept numeric evidence"
                        .to_string(),
                ));
            }
            None => {}
        }

        if temporal_reason(self.reason) {
            if self.retry_after_seconds.is_none_or(|value| value == 0) {
                return Err(ForumError::Validation(
                    "Forum posting policy temporal denial requires a positive retry delay"
                        .to_string(),
                ));
            }
        } else if self.retry_after_seconds.is_some() {
            return Err(ForumError::Validation(
                "Forum posting policy non-temporal denial cannot carry a retry delay".to_string(),
            ));
        }

        Ok(self)
    }

    fn validate_indeterminate(self) -> ForumResult<Self> {
        if self.reason != ForumPostingPolicyDecisionReason::RequiredFactUnavailable
            || self.fact.is_none()
            || self.evidence.is_some()
            || self.retry_after_seconds.is_some()
        {
            return Err(ForumError::Validation(
                "Forum posting policy indeterminate decision must identify one unavailable required fact"
                    .to_string(),
            ));
        }
        Ok(self)
    }
}

fn expected_fact(reason: ForumPostingPolicyDecisionReason) -> Option<ForumPostingPolicyFactKind> {
    match reason {
        ForumPostingPolicyDecisionReason::TrustLevel => {
            Some(ForumPostingPolicyFactKind::TrustLevel)
        }
        ForumPostingPolicyDecisionReason::AccountAge => {
            Some(ForumPostingPolicyFactKind::AccountAgeSeconds)
        }
        ForumPostingPolicyDecisionReason::ReadingActivity => {
            Some(ForumPostingPolicyFactKind::TopicsRead)
        }
        ForumPostingPolicyDecisionReason::ApprovedPosts => {
            Some(ForumPostingPolicyFactKind::ApprovedPosts)
        }
        ForumPostingPolicyDecisionReason::ActiveFlags => {
            Some(ForumPostingPolicyFactKind::ActiveFlags)
        }
        ForumPostingPolicyDecisionReason::Reputation => {
            Some(ForumPostingPolicyFactKind::Reputation)
        }
        ForumPostingPolicyDecisionReason::ModerationHistory => {
            Some(ForumPostingPolicyFactKind::RecentModerationActions)
        }
        ForumPostingPolicyDecisionReason::TopicRateLimit => {
            Some(ForumPostingPolicyFactKind::TopicCreatesWindow)
        }
        ForumPostingPolicyDecisionReason::ReplyRateLimit => {
            Some(ForumPostingPolicyFactKind::ReplyCreatesWindow)
        }
        ForumPostingPolicyDecisionReason::EditRateLimit => {
            Some(ForumPostingPolicyFactKind::EditsWindow)
        }
        ForumPostingPolicyDecisionReason::BumpInterval => {
            Some(ForumPostingPolicyFactKind::SecondsSinceLastBump)
        }
        ForumPostingPolicyDecisionReason::Allowed
        | ForumPostingPolicyDecisionReason::RequiredFactUnavailable
        | ForumPostingPolicyDecisionReason::LinkLimit
        | ForumPostingPolicyDecisionReason::MentionLimit
        | ForumPostingPolicyDecisionReason::AttachmentLimit
        | ForumPostingPolicyDecisionReason::DuplicateContent
        | ForumPostingPolicyDecisionReason::ExternalSpamScore => None,
    }
}

fn expected_evidence_unit(
    reason: ForumPostingPolicyDecisionReason,
) -> Option<ForumPostingPolicyMeasureUnit> {
    match reason {
        ForumPostingPolicyDecisionReason::TrustLevel => {
            Some(ForumPostingPolicyMeasureUnit::TrustLevel)
        }
        ForumPostingPolicyDecisionReason::AccountAge
        | ForumPostingPolicyDecisionReason::BumpInterval => {
            Some(ForumPostingPolicyMeasureUnit::Seconds)
        }
        ForumPostingPolicyDecisionReason::Reputation => {
            Some(ForumPostingPolicyMeasureUnit::Reputation)
        }
        ForumPostingPolicyDecisionReason::ReadingActivity
        | ForumPostingPolicyDecisionReason::ApprovedPosts
        | ForumPostingPolicyDecisionReason::ActiveFlags
        | ForumPostingPolicyDecisionReason::ModerationHistory
        | ForumPostingPolicyDecisionReason::TopicRateLimit
        | ForumPostingPolicyDecisionReason::ReplyRateLimit
        | ForumPostingPolicyDecisionReason::LinkLimit
        | ForumPostingPolicyDecisionReason::MentionLimit
        | ForumPostingPolicyDecisionReason::AttachmentLimit
        | ForumPostingPolicyDecisionReason::EditRateLimit => {
            Some(ForumPostingPolicyMeasureUnit::Count)
        }
        ForumPostingPolicyDecisionReason::Allowed
        | ForumPostingPolicyDecisionReason::RequiredFactUnavailable
        | ForumPostingPolicyDecisionReason::DuplicateContent
        | ForumPostingPolicyDecisionReason::ExternalSpamScore => None,
    }
}

fn temporal_reason(reason: ForumPostingPolicyDecisionReason) -> bool {
    matches!(
        reason,
        ForumPostingPolicyDecisionReason::TopicRateLimit
            | ForumPostingPolicyDecisionReason::ReplyRateLimit
            | ForumPostingPolicyDecisionReason::EditRateLimit
            | ForumPostingPolicyDecisionReason::BumpInterval
    )
}

fn reject_duplicate_fact_kinds(
    facts: &[ForumPostingPolicyFactKind],
    label: &str,
) -> ForumResult<()> {
    if facts.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ForumError::Validation(format!(
            "Forum posting policy {label} must be unique"
        )));
    }
    Ok(())
}

fn reject_duplicate_unavailable_facts(
    facts: &[ForumPostingPolicyUnavailableFact],
) -> ForumResult<()> {
    if facts.windows(2).any(|pair| pair[0].fact == pair[1].fact) {
        return Err(ForumError::Validation(
            "Forum posting policy unavailable facts must be unique".to_string(),
        ));
    }
    Ok(())
}

fn normalize_reason_code(reason_code: String) -> ForumResult<String> {
    let reason_code = reason_code.trim().to_ascii_lowercase();
    if reason_code.is_empty()
        || reason_code.len() > MAX_FORUM_POSTING_UNAVAILABLE_REASON_CODE_LENGTH
        || !reason_code.chars().enumerate().all(|(index, character)| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || (index > 0 && matches!(character, '_' | '.' | '-'))
        })
    {
        return Err(ForumError::Validation(
            "Forum posting policy unavailable reason code must be a bounded lowercase token"
                .to_string(),
        ));
    }
    Ok(reason_code)
}
