use rustok_forum::{
    ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyDecision,
    ForumPostingPolicyDecisionReason, ForumPostingPolicyEvaluationInput,
    ForumPostingPolicyEvidence, ForumPostingPolicyFactKind, ForumPostingPolicyFacts,
    ForumPostingPolicyMeasureUnit, ForumPostingPolicyOutcome, ForumPostingPolicyUnavailableFact,
    ForumPostingWindowCount,
};
use uuid::Uuid;

fn input(facts: ForumPostingPolicyFacts) -> ForumPostingPolicyEvaluationInput {
    ForumPostingPolicyEvaluationInput {
        tenant_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        action: ForumPostingAction::CreateReply,
        candidate: ForumPostingCandidateMetrics {
            body_bytes: 512,
            link_count: 1,
            mention_count: 2,
            attachment_count: 0,
        },
        facts,
    }
}

#[test]
fn required_facts_are_exactly_available_or_explicitly_unavailable() {
    let normalized = input(ForumPostingPolicyFacts {
        required_facts: vec![
            ForumPostingPolicyFactKind::AccountAgeSeconds,
            ForumPostingPolicyFactKind::TrustLevel,
        ],
        unavailable_facts: vec![ForumPostingPolicyUnavailableFact {
            fact: ForumPostingPolicyFactKind::AccountAgeSeconds,
            retryable: true,
            reason_code: " Profiles.Age-Unavailable ".to_string(),
        }],
        trust_level: Some(25),
        ..ForumPostingPolicyFacts::default()
    })
    .normalize()
    .expect("available and unavailable facts should form one exact partition");

    assert_eq!(
        normalized.facts.required_facts,
        vec![
            ForumPostingPolicyFactKind::TrustLevel,
            ForumPostingPolicyFactKind::AccountAgeSeconds,
        ]
    );
    assert_eq!(
        normalized.facts.unavailable_facts[0].reason_code,
        "profiles.age-unavailable"
    );
    assert!(normalized.facts.unavailable_facts[0].retryable);
}

#[test]
fn missing_or_duplicated_required_fact_state_is_rejected() {
    let missing = input(ForumPostingPolicyFacts {
        required_facts: vec![ForumPostingPolicyFactKind::ApprovedPosts],
        ..ForumPostingPolicyFacts::default()
    })
    .normalize()
    .expect_err("a required fact cannot silently disappear");
    assert_eq!(missing.stable_code(), "FORUM_VALIDATION_FAILED");

    let duplicated = input(ForumPostingPolicyFacts {
        required_facts: vec![ForumPostingPolicyFactKind::TrustLevel],
        unavailable_facts: vec![ForumPostingPolicyUnavailableFact {
            fact: ForumPostingPolicyFactKind::TrustLevel,
            retryable: false,
            reason_code: "trust.snapshot_missing".to_string(),
        }],
        trust_level: Some(10),
        ..ForumPostingPolicyFacts::default()
    })
    .normalize()
    .expect_err("one fact cannot be both available and unavailable");
    assert_eq!(duplicated.stable_code(), "FORUM_VALIDATION_FAILED");
}

#[test]
fn undeclared_fact_and_invalid_window_are_rejected() {
    let undeclared = input(ForumPostingPolicyFacts {
        trust_level: Some(20),
        ..ForumPostingPolicyFacts::default()
    })
    .normalize()
    .expect_err("facts must be declared required before being supplied");
    assert_eq!(undeclared.stable_code(), "FORUM_VALIDATION_FAILED");

    let invalid_window = input(ForumPostingPolicyFacts {
        required_facts: vec![ForumPostingPolicyFactKind::ReplyCreatesWindow],
        reply_creates_window: Some(ForumPostingWindowCount {
            count: 3,
            window_seconds: 0,
        }),
        ..ForumPostingPolicyFacts::default()
    })
    .normalize()
    .expect_err("rate observations require a positive bounded window");
    assert_eq!(invalid_window.stable_code(), "FORUM_VALIDATION_FAILED");
}

#[test]
fn allowed_denied_and_indeterminate_decisions_have_distinct_shapes() {
    assert_eq!(
        ForumPostingPolicyDecision::allowed()
            .normalize()
            .expect("canonical allow decision should validate")
            .outcome,
        ForumPostingPolicyOutcome::Allowed
    );

    let denied = ForumPostingPolicyDecision::denied(
        ForumPostingPolicyDecisionReason::ReplyRateLimit,
        Some(ForumPostingPolicyEvidence {
            observed: 8,
            threshold: 5,
            unit: ForumPostingPolicyMeasureUnit::Count,
        }),
        Some(30),
    )
    .expect("temporal rate denial should carry count evidence and retry delay");
    assert_eq!(denied.outcome, ForumPostingPolicyOutcome::Denied);
    assert_eq!(
        denied.fact,
        Some(ForumPostingPolicyFactKind::ReplyCreatesWindow)
    );
    assert!(!denied.retryable);

    let indeterminate = ForumPostingPolicyDecision::indeterminate(
        ForumPostingPolicyFactKind::AccountAgeSeconds,
        true,
    )
    .normalize()
    .expect("unavailable required fact should remain a separate outcome");
    assert_eq!(
        indeterminate.outcome,
        ForumPostingPolicyOutcome::Indeterminate
    );
    assert_eq!(
        indeterminate.reason,
        ForumPostingPolicyDecisionReason::RequiredFactUnavailable
    );
    assert!(indeterminate.retryable);
}

#[test]
fn decision_reason_fact_evidence_and_retry_metadata_cannot_drift() {
    let wrong_unit = ForumPostingPolicyDecision::denied(
        ForumPostingPolicyDecisionReason::TrustLevel,
        Some(ForumPostingPolicyEvidence {
            observed: 5,
            threshold: 10,
            unit: ForumPostingPolicyMeasureUnit::Count,
        }),
        None,
    )
    .expect_err("trust denials require trust-level evidence");
    assert_eq!(wrong_unit.stable_code(), "FORUM_VALIDATION_FAILED");

    let missing_retry = ForumPostingPolicyDecision {
        outcome: ForumPostingPolicyOutcome::Denied,
        reason: ForumPostingPolicyDecisionReason::TopicRateLimit,
        fact: Some(ForumPostingPolicyFactKind::TopicCreatesWindow),
        evidence: Some(ForumPostingPolicyEvidence {
            observed: 4,
            threshold: 3,
            unit: ForumPostingPolicyMeasureUnit::Count,
        }),
        retry_after_seconds: None,
        retryable: false,
    }
    .normalize()
    .expect_err("temporal denials require a positive retry delay");
    assert_eq!(missing_retry.stable_code(), "FORUM_VALIDATION_FAILED");

    let wrong_fact = ForumPostingPolicyDecision {
        outcome: ForumPostingPolicyOutcome::Denied,
        reason: ForumPostingPolicyDecisionReason::AccountAge,
        fact: Some(ForumPostingPolicyFactKind::TrustLevel),
        evidence: Some(ForumPostingPolicyEvidence {
            observed: 60,
            threshold: 3_600,
            unit: ForumPostingPolicyMeasureUnit::Seconds,
        }),
        retry_after_seconds: None,
        retryable: false,
    }
    .normalize()
    .expect_err("decision fact must match its typed reason");
    assert_eq!(wrong_fact.stable_code(), "FORUM_VALIDATION_FAILED");
}
