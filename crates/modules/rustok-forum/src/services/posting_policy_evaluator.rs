use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::audience::MAX_FORUM_AUDIENCE_TRUST_LEVEL;
use crate::error::{ForumError, ForumResult};

use super::posting_policy::{
    ForumPostingAction, ForumPostingPolicyDecision, ForumPostingPolicyDecisionReason,
    ForumPostingPolicyEvaluationInput, ForumPostingPolicyEvidence, ForumPostingPolicyFactKind,
    ForumPostingPolicyMeasureUnit, ForumPostingWindowCount,
};

/// Stable evaluation order. Required-fact availability is resolved before any
/// partial policy decision. Reserved duplicate-content and external-score reasons
/// are deliberately absent until their owner contracts exist.
pub const FORUM_POSTING_POLICY_PRECEDENCE: [ForumPostingPolicyDecisionReason; 16] = [
    ForumPostingPolicyDecisionReason::RequiredFactUnavailable,
    ForumPostingPolicyDecisionReason::ActiveFlags,
    ForumPostingPolicyDecisionReason::ModerationHistory,
    ForumPostingPolicyDecisionReason::TrustLevel,
    ForumPostingPolicyDecisionReason::AccountAge,
    ForumPostingPolicyDecisionReason::ReadingActivity,
    ForumPostingPolicyDecisionReason::ApprovedPosts,
    ForumPostingPolicyDecisionReason::Reputation,
    ForumPostingPolicyDecisionReason::TopicRateLimit,
    ForumPostingPolicyDecisionReason::ReplyRateLimit,
    ForumPostingPolicyDecisionReason::EditRateLimit,
    ForumPostingPolicyDecisionReason::BumpInterval,
    ForumPostingPolicyDecisionReason::LinkLimit,
    ForumPostingPolicyDecisionReason::MentionLimit,
    ForumPostingPolicyDecisionReason::AttachmentLimit,
    ForumPostingPolicyDecisionReason::Allowed,
];

const FACT_AVAILABILITY_PRECEDENCE: [ForumPostingPolicyFactKind; 11] = [
    ForumPostingPolicyFactKind::ActiveFlags,
    ForumPostingPolicyFactKind::RecentModerationActions,
    ForumPostingPolicyFactKind::TrustLevel,
    ForumPostingPolicyFactKind::AccountAgeSeconds,
    ForumPostingPolicyFactKind::TopicsRead,
    ForumPostingPolicyFactKind::ApprovedPosts,
    ForumPostingPolicyFactKind::Reputation,
    ForumPostingPolicyFactKind::TopicCreatesWindow,
    ForumPostingPolicyFactKind::ReplyCreatesWindow,
    ForumPostingPolicyFactKind::EditsWindow,
    ForumPostingPolicyFactKind::SecondsSinceLastBump,
];
const MAX_SIGNED_EVIDENCE: u64 = i64::MAX as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingWindowLimit {
    pub maximum_count: u32,
    pub window_seconds: u32,
}

impl ForumPostingWindowLimit {
    pub fn normalize(self) -> ForumResult<Self> {
        if self.maximum_count == 0 || self.window_seconds == 0 {
            return Err(ForumError::Validation(
                "Forum posting policy window limits require positive count and duration"
                    .to_string(),
            ));
        }
        Ok(self)
    }
}

/// Pure Forum-owned policy thresholds. This type is not persistence, transport,
/// distributed rate-limit execution or automatic trust configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumPostingPolicyRules {
    pub minimum_trust_level: Option<u8>,
    pub minimum_account_age_seconds: Option<u64>,
    pub minimum_topics_read: Option<u64>,
    pub minimum_approved_posts: Option<u64>,
    pub maximum_active_flags: Option<u32>,
    pub minimum_reputation: Option<i64>,
    pub maximum_recent_moderation_actions: Option<u32>,
    pub topic_create_limit: Option<ForumPostingWindowLimit>,
    pub reply_create_limit: Option<ForumPostingWindowLimit>,
    pub edit_limit: Option<ForumPostingWindowLimit>,
    pub minimum_seconds_between_bumps: Option<u64>,
    pub maximum_links: Option<u16>,
    pub maximum_mentions: Option<u16>,
    pub maximum_attachments: Option<u16>,
}

impl ForumPostingPolicyRules {
    pub fn normalize(mut self) -> ForumResult<Self> {
        if self.minimum_trust_level == Some(0)
            || self.minimum_trust_level > Some(MAX_FORUM_AUDIENCE_TRUST_LEVEL)
        {
            return Err(ForumError::Validation(format!(
                "Forum posting policy minimum trust must be between 1 and {MAX_FORUM_AUDIENCE_TRUST_LEVEL}"
            )));
        }
        validate_positive_unsigned_minimum(
            self.minimum_account_age_seconds,
            "minimum account age",
        )?;
        validate_positive_unsigned_minimum(self.minimum_topics_read, "minimum topics read")?;
        validate_positive_unsigned_minimum(self.minimum_approved_posts, "minimum approved posts")?;
        validate_positive_unsigned_minimum(
            self.minimum_seconds_between_bumps,
            "minimum bump interval",
        )?;
        self.topic_create_limit = self
            .topic_create_limit
            .map(ForumPostingWindowLimit::normalize)
            .transpose()?;
        self.reply_create_limit = self
            .reply_create_limit
            .map(ForumPostingWindowLimit::normalize)
            .transpose()?;
        self.edit_limit = self
            .edit_limit
            .map(ForumPostingWindowLimit::normalize)
            .transpose()?;
        Ok(self)
    }

    pub fn required_facts(
        &self,
        action: ForumPostingAction,
    ) -> ForumResult<Vec<ForumPostingPolicyFactKind>> {
        let rules = self.clone().normalize()?;
        Ok(required_facts_for_normalized_rules(&rules, action))
    }
}

pub struct ForumPostingPolicyEvaluator;

impl ForumPostingPolicyEvaluator {
    pub fn decide(
        rules: &ForumPostingPolicyRules,
        input: ForumPostingPolicyEvaluationInput,
    ) -> ForumResult<ForumPostingPolicyDecision> {
        let rules = rules.clone().normalize()?;
        let input = input.normalize()?;
        let required_facts = required_facts_for_normalized_rules(&rules, input.action);
        if input.facts.required_facts != required_facts {
            return Err(ForumError::Validation(
                "Forum posting policy input facts do not match the exact rules and action"
                    .to_string(),
            ));
        }

        if let Some(unavailable) = first_unavailable_fact(&input) {
            return Ok(ForumPostingPolicyDecision::indeterminate(
                unavailable.fact,
                unavailable.retryable,
            ));
        }

        if let Some(maximum) = rules.maximum_active_flags {
            let observed = required_u32(
                input.facts.active_flags,
                ForumPostingPolicyFactKind::ActiveFlags,
            )?;
            if observed > maximum {
                return count_denial(
                    ForumPostingPolicyDecisionReason::ActiveFlags,
                    observed,
                    maximum,
                    None,
                );
            }
        }

        if let Some(maximum) = rules.maximum_recent_moderation_actions {
            let observed = required_u32(
                input.facts.recent_moderation_actions,
                ForumPostingPolicyFactKind::RecentModerationActions,
            )?;
            if observed > maximum {
                return count_denial(
                    ForumPostingPolicyDecisionReason::ModerationHistory,
                    observed,
                    maximum,
                    None,
                );
            }
        }

        if let Some(minimum) = rules.minimum_trust_level {
            let observed = required_u8(
                input.facts.trust_level,
                ForumPostingPolicyFactKind::TrustLevel,
            )?;
            if observed < minimum {
                return ForumPostingPolicyDecision::denied(
                    ForumPostingPolicyDecisionReason::TrustLevel,
                    Some(ForumPostingPolicyEvidence {
                        observed: i64::from(observed),
                        threshold: i64::from(minimum),
                        unit: ForumPostingPolicyMeasureUnit::TrustLevel,
                    }),
                    None,
                );
            }
        }

        if let Some(minimum) = rules.minimum_account_age_seconds {
            let observed = required_u64(
                input.facts.account_age_seconds,
                ForumPostingPolicyFactKind::AccountAgeSeconds,
            )?;
            if observed < minimum {
                return unsigned_denial(
                    ForumPostingPolicyDecisionReason::AccountAge,
                    observed,
                    minimum,
                    ForumPostingPolicyMeasureUnit::Seconds,
                    None,
                );
            }
        }

        if let Some(minimum) = rules.minimum_topics_read {
            let observed = required_u64(
                input.facts.topics_read,
                ForumPostingPolicyFactKind::TopicsRead,
            )?;
            if observed < minimum {
                return unsigned_denial(
                    ForumPostingPolicyDecisionReason::ReadingActivity,
                    observed,
                    minimum,
                    ForumPostingPolicyMeasureUnit::Count,
                    None,
                );
            }
        }

        if let Some(minimum) = rules.minimum_approved_posts {
            let observed = required_u64(
                input.facts.approved_posts,
                ForumPostingPolicyFactKind::ApprovedPosts,
            )?;
            if observed < minimum {
                return unsigned_denial(
                    ForumPostingPolicyDecisionReason::ApprovedPosts,
                    observed,
                    minimum,
                    ForumPostingPolicyMeasureUnit::Count,
                    None,
                );
            }
        }

        if let Some(minimum) = rules.minimum_reputation {
            let observed = required_i64(
                input.facts.reputation,
                ForumPostingPolicyFactKind::Reputation,
            )?;
            if observed < minimum {
                return ForumPostingPolicyDecision::denied(
                    ForumPostingPolicyDecisionReason::Reputation,
                    Some(ForumPostingPolicyEvidence {
                        observed,
                        threshold: minimum,
                        unit: ForumPostingPolicyMeasureUnit::Reputation,
                    }),
                    None,
                );
            }
        }

        match input.action {
            ForumPostingAction::CreateTopic => {
                if let Some(limit) = rules.topic_create_limit {
                    let decision = evaluate_window(
                        input.facts.topic_creates_window,
                        limit,
                        ForumPostingPolicyFactKind::TopicCreatesWindow,
                        ForumPostingPolicyDecisionReason::TopicRateLimit,
                    )?;
                    if let Some(decision) = decision {
                        return Ok(decision);
                    }
                }
            }
            ForumPostingAction::CreateReply => {
                if let Some(limit) = rules.reply_create_limit {
                    let decision = evaluate_window(
                        input.facts.reply_creates_window,
                        limit,
                        ForumPostingPolicyFactKind::ReplyCreatesWindow,
                        ForumPostingPolicyDecisionReason::ReplyRateLimit,
                    )?;
                    if let Some(decision) = decision {
                        return Ok(decision);
                    }
                }
            }
            ForumPostingAction::EditTopic | ForumPostingAction::EditReply => {
                if let Some(limit) = rules.edit_limit {
                    let decision = evaluate_window(
                        input.facts.edits_window,
                        limit,
                        ForumPostingPolicyFactKind::EditsWindow,
                        ForumPostingPolicyDecisionReason::EditRateLimit,
                    )?;
                    if let Some(decision) = decision {
                        return Ok(decision);
                    }
                }
            }
            ForumPostingAction::BumpTopic => {
                if let Some(minimum) = rules.minimum_seconds_between_bumps {
                    let observed = required_u64(
                        input.facts.seconds_since_last_bump,
                        ForumPostingPolicyFactKind::SecondsSinceLastBump,
                    )?;
                    if observed < minimum {
                        return unsigned_denial(
                            ForumPostingPolicyDecisionReason::BumpInterval,
                            observed,
                            minimum,
                            ForumPostingPolicyMeasureUnit::Seconds,
                            Some(minimum - observed),
                        );
                    }
                }
            }
        }

        if let Some(maximum) = rules
            .maximum_links
            .filter(|&max| input.candidate.link_count > max)
        {
            return count_denial(
                ForumPostingPolicyDecisionReason::LinkLimit,
                u32::from(input.candidate.link_count),
                u32::from(maximum),
                None,
            );
        }
        if let Some(maximum) = rules
            .maximum_mentions
            .filter(|&max| input.candidate.mention_count > max)
        {
            return count_denial(
                ForumPostingPolicyDecisionReason::MentionLimit,
                u32::from(input.candidate.mention_count),
                u32::from(maximum),
                None,
            );
        }
        if let Some(maximum) = rules
            .maximum_attachments
            .filter(|&max| input.candidate.attachment_count > max)
        {
            return count_denial(
                ForumPostingPolicyDecisionReason::AttachmentLimit,
                u32::from(input.candidate.attachment_count),
                u32::from(maximum),
                None,
            );
        }

        Ok(ForumPostingPolicyDecision::allowed())
    }
}

fn required_facts_for_normalized_rules(
    rules: &ForumPostingPolicyRules,
    action: ForumPostingAction,
) -> Vec<ForumPostingPolicyFactKind> {
    let mut facts = Vec::new();
    push_if(
        &mut facts,
        rules.maximum_active_flags.is_some(),
        ForumPostingPolicyFactKind::ActiveFlags,
    );
    push_if(
        &mut facts,
        rules.maximum_recent_moderation_actions.is_some(),
        ForumPostingPolicyFactKind::RecentModerationActions,
    );
    push_if(
        &mut facts,
        rules.minimum_trust_level.is_some(),
        ForumPostingPolicyFactKind::TrustLevel,
    );
    push_if(
        &mut facts,
        rules.minimum_account_age_seconds.is_some(),
        ForumPostingPolicyFactKind::AccountAgeSeconds,
    );
    push_if(
        &mut facts,
        rules.minimum_topics_read.is_some(),
        ForumPostingPolicyFactKind::TopicsRead,
    );
    push_if(
        &mut facts,
        rules.minimum_approved_posts.is_some(),
        ForumPostingPolicyFactKind::ApprovedPosts,
    );
    push_if(
        &mut facts,
        rules.minimum_reputation.is_some(),
        ForumPostingPolicyFactKind::Reputation,
    );
    match action {
        ForumPostingAction::CreateTopic => push_if(
            &mut facts,
            rules.topic_create_limit.is_some(),
            ForumPostingPolicyFactKind::TopicCreatesWindow,
        ),
        ForumPostingAction::CreateReply => push_if(
            &mut facts,
            rules.reply_create_limit.is_some(),
            ForumPostingPolicyFactKind::ReplyCreatesWindow,
        ),
        ForumPostingAction::EditTopic | ForumPostingAction::EditReply => push_if(
            &mut facts,
            rules.edit_limit.is_some(),
            ForumPostingPolicyFactKind::EditsWindow,
        ),
        ForumPostingAction::BumpTopic => push_if(
            &mut facts,
            rules.minimum_seconds_between_bumps.is_some(),
            ForumPostingPolicyFactKind::SecondsSinceLastBump,
        ),
    }
    facts.sort_unstable();
    facts
}

fn first_unavailable_fact(
    input: &ForumPostingPolicyEvaluationInput,
) -> Option<&super::posting_policy::ForumPostingPolicyUnavailableFact> {
    FACT_AVAILABILITY_PRECEDENCE.iter().find_map(|fact| {
        input
            .facts
            .unavailable_facts
            .iter()
            .find(|item| item.fact == *fact)
    })
}

fn evaluate_window(
    observed: Option<ForumPostingWindowCount>,
    limit: ForumPostingWindowLimit,
    fact: ForumPostingPolicyFactKind,
    reason: ForumPostingPolicyDecisionReason,
) -> ForumResult<Option<ForumPostingPolicyDecision>> {
    let observed = observed.ok_or_else(|| missing_available_fact(fact))?;
    if observed.window_seconds != limit.window_seconds {
        return Err(ForumError::Validation(
            "Forum posting policy observation window does not match its configured rule"
                .to_string(),
        ));
    }
    if observed.count < limit.maximum_count {
        return Ok(None);
    }
    count_denial(
        reason,
        observed.count,
        limit.maximum_count,
        Some(u64::from(limit.window_seconds)),
    )
    .map(Some)
}

fn count_denial(
    reason: ForumPostingPolicyDecisionReason,
    observed: u32,
    threshold: u32,
    retry_after_seconds: Option<u64>,
) -> ForumResult<ForumPostingPolicyDecision> {
    ForumPostingPolicyDecision::denied(
        reason,
        Some(ForumPostingPolicyEvidence {
            observed: i64::from(observed),
            threshold: i64::from(threshold),
            unit: ForumPostingPolicyMeasureUnit::Count,
        }),
        retry_after_seconds,
    )
}

fn unsigned_denial(
    reason: ForumPostingPolicyDecisionReason,
    observed: u64,
    threshold: u64,
    unit: ForumPostingPolicyMeasureUnit,
    retry_after_seconds: Option<u64>,
) -> ForumResult<ForumPostingPolicyDecision> {
    ForumPostingPolicyDecision::denied(
        reason,
        Some(ForumPostingPolicyEvidence {
            observed: bounded_i64(observed)?,
            threshold: bounded_i64(threshold)?,
            unit,
        }),
        retry_after_seconds,
    )
}

fn required_u8(value: Option<u8>, fact: ForumPostingPolicyFactKind) -> ForumResult<u8> {
    value.ok_or_else(|| missing_available_fact(fact))
}

fn required_u32(value: Option<u32>, fact: ForumPostingPolicyFactKind) -> ForumResult<u32> {
    value.ok_or_else(|| missing_available_fact(fact))
}

fn required_u64(value: Option<u64>, fact: ForumPostingPolicyFactKind) -> ForumResult<u64> {
    value.ok_or_else(|| missing_available_fact(fact))
}

fn required_i64(value: Option<i64>, fact: ForumPostingPolicyFactKind) -> ForumResult<i64> {
    value.ok_or_else(|| missing_available_fact(fact))
}

fn missing_available_fact(fact: ForumPostingPolicyFactKind) -> ForumError {
    ForumError::Validation(format!(
        "Forum posting policy required fact {fact:?} is not available"
    ))
}

fn bounded_i64(value: u64) -> ForumResult<i64> {
    i64::try_from(value).map_err(|_| {
        ForumError::Validation(
            "Forum posting policy evidence exceeds the supported signed range".to_string(),
        )
    })
}

fn push_if(
    facts: &mut Vec<ForumPostingPolicyFactKind>,
    condition: bool,
    fact: ForumPostingPolicyFactKind,
) {
    if condition {
        facts.push(fact);
    }
}

fn validate_positive_unsigned_minimum(value: Option<u64>, label: &str) -> ForumResult<()> {
    if value == Some(0) {
        return Err(ForumError::Validation(format!(
            "Forum posting policy {label} must be greater than zero"
        )));
    }
    if value.is_some_and(|value| value > MAX_SIGNED_EVIDENCE) {
        return Err(ForumError::Validation(format!(
            "Forum posting policy {label} exceeds the supported evidence range"
        )));
    }
    Ok(())
}
