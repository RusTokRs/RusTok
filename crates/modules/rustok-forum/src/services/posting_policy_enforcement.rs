//! Create-path posting policy enforcement (plan item 7 / review finding D1).
//!
//! Tenant-configured window rules flow from `ForumContentLimits` into two
//! phases that straddle the create transaction:
//!
//! - [`prepare_topic_create_posting_policy`] /
//!   [`prepare_reply_create_posting_policy`] run **before** `begin()`: they
//!   skip non-`User` actors (the `posting_rate` exemption) and rules that
//!   require no facts, otherwise compose the evaluation input through the
//!   host-published facts composer. Fact reads therefore never race the
//!   writer lock. Missing composer/context/composition failure fail closed
//!   as [`ForumError::Internal`]; a missing caller identity fails closed as
//!   [`ForumError::Forbidden`].
//! - [`enforce_prepared_create_posting_policy`] runs **inside** the
//!   transaction, after authorization and the flat cooldown: it first rejects
//!   changed transactional rules, then makes a pure
//!   evaluator decision. Temporal denials map to [`ForumError::RateLimited`]
//!   (HTTP 429, stable `FORUM_RATE_LIMITED` code, like `posting_rate`);
//!   indeterminate retryable outcomes map to a bounded 429 hint;
//!   indeterminate non-retryable outcomes fail closed as
//!   [`ForumError::Internal`].
//!
//! Window rules are absent by default, so both phases are inert until a
//! tenant opts in via the `posting_policy_topic_create_limit` /
//! `posting_policy_reply_create_limit` module settings. Candidate-dependent
//! rules (links, mentions, attachments) are not settings-driven and stay
//! open scope — see
//! `DECISIONS/2026-11-27-forum-posting-policy-window-enforcement.md`.

use rustok_api::PortContext;
use rustok_core::security_context::{SecurityActorKind, SecurityContext};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

use super::posting_policy::{
    ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyDecision,
    ForumPostingPolicyDecisionReason, ForumPostingPolicyEvaluationInput, ForumPostingPolicyOutcome,
};
use super::posting_policy_evaluator::{ForumPostingPolicyEvaluator, ForumPostingPolicyRules};
use super::posting_policy_facts::{
    ForumPostingPolicyCompositionRequest, SharedForumPostingPolicyFactsComposer,
};

/// Bounded retry hint for transient fact-owner failures while the provider
/// recovers; keeps the create path uniformly 429-honest under load instead of
/// leaking internal availability into the public reason.
const FACT_UNAVAILABLE_RETRY_AFTER_SECONDS: u64 = 60;

/// A composed create-path policy for one write attempt.
///
/// `input` is `None` when the evaluation is inert: a non-`User` actor or
/// rules that require no facts. `rules` are always carried so the
/// in-transaction decision runs against the exact snapshot the facts were
/// composed for. `exempt` distinguishes non-user actors from unconfigured users:
/// the latter must still reject a rule enabled after preparation.
#[derive(Clone, Debug)]
pub(crate) struct PreparedCreatePostingPolicy {
    rules: ForumPostingPolicyRules,
    input: Option<ForumPostingPolicyEvaluationInput>,
    exempt: bool,
}

impl PreparedCreatePostingPolicy {
    fn inert() -> Self {
        Self {
            rules: ForumPostingPolicyRules::default(),
            input: None,
            exempt: true,
        }
    }
}

/// Prepare the `CreateTopic` posting policy before the transaction opens.
pub(crate) async fn prepare_topic_create_posting_policy(
    composer: Option<&SharedForumPostingPolicyFactsComposer>,
    context: Option<&PortContext>,
    security: &SecurityContext,
    tenant_id: Uuid,
    rules: ForumPostingPolicyRules,
) -> ForumResult<PreparedCreatePostingPolicy> {
    prepare_create_posting_policy(
        composer,
        context,
        security,
        tenant_id,
        ForumPostingAction::CreateTopic,
        rules,
    )
    .await
}

/// Prepare the `CreateReply` posting policy before the transaction opens.
pub(crate) async fn prepare_reply_create_posting_policy(
    composer: Option<&SharedForumPostingPolicyFactsComposer>,
    context: Option<&PortContext>,
    security: &SecurityContext,
    tenant_id: Uuid,
    rules: ForumPostingPolicyRules,
) -> ForumResult<PreparedCreatePostingPolicy> {
    prepare_create_posting_policy(
        composer,
        context,
        security,
        tenant_id,
        ForumPostingAction::CreateReply,
        rules,
    )
    .await
}

async fn prepare_create_posting_policy(
    composer: Option<&SharedForumPostingPolicyFactsComposer>,
    context: Option<&PortContext>,
    security: &SecurityContext,
    tenant_id: Uuid,
    action: ForumPostingAction,
    rules: ForumPostingPolicyRules,
) -> ForumResult<PreparedCreatePostingPolicy> {
    // System and service actors keep the posting-rate exemption: policy facts
    // model human authors, and the starter driver runs as System.
    if !matches!(security.actor_kind, SecurityActorKind::User) {
        return Ok(PreparedCreatePostingPolicy::inert());
    }
    // Default settings require no facts, so the composer is never touched —
    // the pipeline stays inert until a tenant configures window rules.
    if rules.required_facts(action)?.is_empty() {
        return Ok(PreparedCreatePostingPolicy {
            rules,
            input: None,
            exempt: false,
        });
    }
    // Configured rules without a published composer or request context must
    // not silently skip: fail closed before the write can start.
    let composer =
        composer.ok_or_else(|| posting_policy_unavailable("facts composer is not available"))?;
    let context = context
        .cloned()
        .ok_or_else(|| posting_policy_unavailable("request context is not available"))?;
    let user_id = security.user_id.ok_or_else(|| {
        ForumError::forbidden(
            "Forum posting policy requires the caller's user identity".to_string(),
        )
    })?;
    // Candidate metrics feed link/mention/attachment rules only; those rules
    // are not settings-driven, so the candidate stays defaulted (open scope).
    let request = ForumPostingPolicyCompositionRequest {
        tenant_id,
        user_id,
        action,
        candidate: ForumPostingCandidateMetrics::default(),
    };
    let input = composer
        .compose(context, &rules, request)
        .await
        .map_err(|error| {
            posting_policy_unavailable(&format!("fact composition failed ({})", error.code()))
        })?;
    Ok(PreparedCreatePostingPolicy {
        rules,
        input: Some(input),
        exempt: false,
    })
}

/// Enforce the composed decision inside the create transaction. Pure: no
/// fact reads, no database access, no ports.
pub(crate) fn enforce_prepared_create_posting_policy(
    prepared: &PreparedCreatePostingPolicy,
    current_rules: &ForumPostingPolicyRules,
) -> ForumResult<()> {
    if prepared.exempt {
        return Ok(());
    }
    // Never enforce a preflight rule snapshot against changed transactional
    // settings. In particular, None -> Some must not leave the write inert.
    // A new attempt must compose fresh facts for the new window; doing so here
    // would call external fact owners while holding the write transaction.
    if &prepared.rules != current_rules {
        return Err(posting_policy_unavailable(
            "posting policy settings changed during create preparation",
        ));
    }
    let Some(input) = prepared.input.clone() else {
        return Ok(());
    };
    let decision = ForumPostingPolicyEvaluator::decide(&prepared.rules, input)?;
    match decision.outcome {
        ForumPostingPolicyOutcome::Allowed => Ok(()),
        ForumPostingPolicyOutcome::Denied => Err(denied_error(&decision)),
        // Transient fact-owner failure: bounded retryability without leaking
        // internal availability into the public reason.
        ForumPostingPolicyOutcome::Indeterminate if decision.retryable => {
            Err(ForumError::RateLimited {
                retry_after_seconds: FACT_UNAVAILABLE_RETRY_AFTER_SECONDS,
            })
        }
        // Missing provider or missing required fact: fail closed as an
        // internal error (typed retryability is exhausted here).
        ForumPostingPolicyOutcome::Indeterminate => Err(posting_policy_unavailable(&format!(
            "required posting policy fact {} is unavailable",
            decision
                .fact
                .map(|fact| format!("{fact:?}"))
                .unwrap_or_else(|| "unknown".to_string())
        ))),
    }
}

fn denied_error(decision: &ForumPostingPolicyDecision) -> ForumError {
    match decision.reason {
        // The only delivered denials are temporal window limits; they behave
        // exactly like the flat cooldown (429 + stable FORUM_RATE_LIMITED).
        ForumPostingPolicyDecisionReason::TopicRateLimit
        | ForumPostingPolicyDecisionReason::ReplyRateLimit => ForumError::RateLimited {
            retry_after_seconds: decision
                .retry_after_seconds
                .unwrap_or(FACT_UNAVAILABLE_RETRY_AFTER_SECONDS),
        },
        reason => ForumError::forbidden(format!(
            "Forum posting policy denied the operation ({reason:?})"
        )),
    }
}

fn posting_policy_unavailable(detail: &str) -> ForumError {
    ForumError::Internal(rustok_core::Error::Internal(format!(
        "forum posting policy unavailable: {detail}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::posting_policy::ForumPostingPolicyFactKind;
    use crate::services::posting_policy_evaluator::ForumPostingWindowLimit;
    use rustok_api::PortActor;
    use rustok_core::security_context::{SecurityContext, UserRole};

    fn window_rules(maximum_count: u32, window_seconds: u32) -> ForumPostingPolicyRules {
        ForumPostingPolicyRules {
            topic_create_limit: Some(ForumPostingWindowLimit {
                maximum_count,
                window_seconds,
            }),
            ..ForumPostingPolicyRules::default()
        }
    }

    fn user_security() -> SecurityContext {
        SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()))
    }

    fn context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "forum-posting-policy-enforcement-test",
        )
        .with_deadline(std::time::Duration::from_secs(5))
    }

    #[tokio::test]
    async fn default_rules_keep_the_pipeline_inert() {
        let prepared = prepare_topic_create_posting_policy(
            None,
            None,
            &user_security(),
            Uuid::new_v4(),
            ForumPostingPolicyRules::default(),
        )
        .await
        .expect("default rules must not require the composer");
        assert!(prepared.input.is_none());
        enforce_prepared_create_posting_policy(&prepared, &ForumPostingPolicyRules::default())
            .expect("inert enforcement allows");
    }

    #[tokio::test]
    async fn system_actors_keep_the_posting_rate_exemption() {
        let mut security = SecurityContext::new(UserRole::Customer, None);
        // SecurityContext::new builds a User principal; rewrite the kind the
        // same way host system contexts are built.
        security.actor_kind = SecurityActorKind::System;
        let prepared = prepare_topic_create_posting_policy(
            None,
            None,
            &security,
            Uuid::new_v4(),
            window_rules(5, 86_400),
        )
        .await
        .expect("system actors must skip evaluation");
        assert!(prepared.input.is_none());
    }

    #[tokio::test]
    async fn configured_window_rules_fail_closed_without_a_composer() {
        let error = prepare_topic_create_posting_policy(
            None,
            None,
            &user_security(),
            Uuid::new_v4(),
            window_rules(5, 86_400),
        )
        .await
        .expect_err("configured rules must not skip evaluation");
        assert!(matches!(error, ForumError::Internal(_)));
    }

    #[tokio::test]
    async fn configured_window_rules_fail_closed_without_a_context() {
        let composer = SharedForumPostingPolicyFactsComposer::default();
        let error = prepare_topic_create_posting_policy(
            Some(&composer),
            None,
            &user_security(),
            Uuid::new_v4(),
            window_rules(5, 86_400),
        )
        .await
        .expect_err("configured rules must not skip evaluation");
        assert!(matches!(error, ForumError::Internal(_)));
    }

    #[tokio::test]
    async fn user_without_identity_fails_closed() {
        let composer = SharedForumPostingPolicyFactsComposer::default();
        let security = SecurityContext::new(UserRole::Customer, None);
        let tenant_id = Uuid::new_v4();
        let request_context = context(tenant_id, Uuid::new_v4());
        let error = prepare_topic_create_posting_policy(
            Some(&composer),
            Some(&request_context),
            &security,
            tenant_id,
            window_rules(5, 86_400),
        )
        .await
        .expect_err("a User principal without identity must not pass");
        assert!(matches!(error, ForumError::Forbidden(_)));
    }

    #[tokio::test]
    async fn configured_window_rules_require_the_window_fact() {
        let rules = window_rules(5, 86_400);
        let required = rules
            .required_facts(ForumPostingAction::CreateTopic)
            .expect("rules normalize");
        assert_eq!(
            required,
            vec![ForumPostingPolicyFactKind::TopicCreatesWindow]
        );
        // The preparation path reaches composition only with a composer and a
        // request context present; both are absent here, so it fails closed
        // instead of silently skipping the configured rule.
        let error = prepare_topic_create_posting_policy(
            None,
            None,
            &user_security(),
            Uuid::new_v4(),
            rules,
        )
        .await
        .expect_err("configured rules must not skip evaluation");
        assert!(matches!(error, ForumError::Internal(_)));
    }

    fn prepared_window(action: ForumPostingAction, count: u32) -> PreparedCreatePostingPolicy {
        use crate::services::posting_policy::{ForumPostingPolicyFacts, ForumPostingWindowCount};

        let limit = ForumPostingWindowLimit {
            maximum_count: 5,
            window_seconds: 60,
        };
        let mut rules = ForumPostingPolicyRules::default();
        let mut facts = ForumPostingPolicyFacts::default();
        let observed = Some(ForumPostingWindowCount {
            count,
            window_seconds: 60,
        });
        match action {
            ForumPostingAction::CreateTopic => {
                rules.topic_create_limit = Some(limit);
                facts.topic_creates_window = observed;
            }
            ForumPostingAction::CreateReply => {
                rules.reply_create_limit = Some(limit);
                facts.reply_creates_window = observed;
            }
            _ => panic!("only create actions belong in this fixture"),
        }
        facts.required_facts = rules.required_facts(action).expect("valid rules");
        PreparedCreatePostingPolicy {
            rules,
            input: Some(ForumPostingPolicyEvaluationInput {
                tenant_id: Uuid::new_v4(),
                user_id: Uuid::new_v4(),
                action,
                candidate: ForumPostingCandidateMetrics::default(),
                facts,
            }),
            exempt: false,
        }
    }

    #[test]
    fn window_decisions_allow_below_limit_and_rate_limit_at_or_above_it() {
        for action in [
            ForumPostingAction::CreateTopic,
            ForumPostingAction::CreateReply,
        ] {
            for count in [4, 5, 6] {
                let prepared = prepared_window(action, count);
                let result = enforce_prepared_create_posting_policy(&prepared, &prepared.rules);
                if count < 5 {
                    result.expect("below the window threshold");
                } else {
                    assert!(matches!(
                        result,
                        Err(ForumError::RateLimited {
                            retry_after_seconds: 60
                        })
                    ));
                }
            }
        }
    }

    #[test]
    fn unavailable_facts_preserve_retryability_mapping() {
        use crate::services::posting_policy::ForumPostingPolicyUnavailableFact;

        for retryable in [false, true] {
            let mut prepared = prepared_window(ForumPostingAction::CreateTopic, 0);
            let input = prepared.input.as_mut().expect("configured input");
            input.facts.topic_creates_window = None;
            input.facts.unavailable_facts = vec![ForumPostingPolicyUnavailableFact {
                fact: ForumPostingPolicyFactKind::TopicCreatesWindow,
                retryable,
                reason_code: "forum.posting_fact.provider_error".to_string(),
            }];
            let result = enforce_prepared_create_posting_policy(&prepared, &prepared.rules);
            if retryable {
                assert!(matches!(
                    result,
                    Err(ForumError::RateLimited {
                        retry_after_seconds: 60
                    })
                ));
            } else {
                assert!(matches!(result, Err(ForumError::Internal(_))));
            }
        }
    }

    #[tokio::test]
    async fn missing_window_provider_composes_but_cannot_authorize_a_write() {
        let composer = SharedForumPostingPolicyFactsComposer::default();
        let security = user_security();
        let tenant_id = Uuid::new_v4();
        let request_context = context(tenant_id, security.user_id.expect("user"));
        let prepared = prepare_topic_create_posting_policy(
            Some(&composer),
            Some(&request_context),
            &security,
            tenant_id,
            window_rules(5, 60),
        )
        .await
        .expect("missing provider becomes an unavailable fact");
        assert!(matches!(
            enforce_prepared_create_posting_policy(&prepared, &prepared.rules),
            Err(ForumError::Internal(_))
        ));
    }

    #[tokio::test]
    async fn newly_enabled_rules_cannot_bypass_an_inert_preparation() {
        for action in [
            ForumPostingAction::CreateTopic,
            ForumPostingAction::CreateReply,
        ] {
            let prepared = prepare_create_posting_policy(
                None,
                None,
                &user_security(),
                Uuid::new_v4(),
                action,
                ForumPostingPolicyRules::default(),
            )
            .await
            .expect("unconfigured preparation");
            let current = prepared_window(action, 0).rules;
            assert!(matches!(
                enforce_prepared_create_posting_policy(&prepared, &current),
                Err(ForumError::Internal(_))
            ));
        }
    }

    #[test]
    fn changed_or_disabled_windows_reject_prepared_facts() {
        let prepared = prepared_window(ForumPostingAction::CreateTopic, 0);
        for current in [
            window_rules(4, 60),
            window_rules(5, 120),
            ForumPostingPolicyRules::default(),
        ] {
            assert!(matches!(
                enforce_prepared_create_posting_policy(&prepared, &current),
                Err(ForumError::Internal(_))
            ));
        }
    }

    #[test]
    fn system_exemption_survives_settings_changes() {
        enforce_prepared_create_posting_policy(
            &PreparedCreatePostingPolicy::inert(),
            &window_rules(5, 60),
        )
        .expect("system exemption is independent of policy values");
    }
}
