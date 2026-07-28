use rustok_forum::{
    ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyDecisionReason,
    ForumPostingPolicyEvaluationInput, ForumPostingPolicyFactKind, ForumPostingPolicyFacts,
    ForumPostingPolicyOutcome, ForumPostingPolicyRules, ForumPostingPolicyUnavailableFact,
    ForumPostingPolicyEvaluator, ForumPostingWindowCount, ForumPostingWindowLimit,
    FORUM_POSTING_POLICY_PRECEDENCE,
};
use uuid::Uuid;

fn rules() -> ForumPostingPolicyRules {
    ForumPostingPolicyRules {
        minimum_trust_level: Some(20),
        minimum_account_age_seconds: Some(86_400),
        minimum_topics_read: Some(10),
        minimum_approved_posts: Some(3),
        maximum_active_flags: Some(1),
        minimum_reputation: Some(0),
        maximum_recent_moderation_actions: Some(0),
        topic_create_limit: Some(ForumPostingWindowLimit {
            maximum_count: 2,
            window_seconds: 86_400,
        }),
        reply_create_limit: Some(ForumPostingWindowLimit {
            maximum_count: 5,
            window_seconds: 60,
        }),
        edit_limit: Some(ForumPostingWindowLimit {
            maximum_count: 4,
            window_seconds: 300,
        }),
        minimum_seconds_between_bumps: Some(600),
        maximum_links: Some(3),
        maximum_mentions: Some(5),
        maximum_attachments: Some(2),
    }
}

fn facts_for(
    rules: &ForumPostingPolicyRules,
    action: ForumPostingAction,
) -> ForumPostingPolicyFacts {
    let required_facts = rules
        .required_facts(action)
        .expect("test rules should derive exact required facts");
    let mut facts = ForumPostingPolicyFacts {
        required_facts,
        ..ForumPostingPolicyFacts::default()
    };
    for fact in facts.required_facts.clone() {
        match fact {
            ForumPostingPolicyFactKind::TrustLevel => facts.trust_level = Some(50),
            ForumPostingPolicyFactKind::AccountAgeSeconds => {
                facts.account_age_seconds = Some(172_800)
            }
            ForumPostingPolicyFactKind::TopicsRead => facts.topics_read = Some(20),
            ForumPostingPolicyFactKind::ApprovedPosts => facts.approved_posts = Some(5),
            ForumPostingPolicyFactKind::ActiveFlags => facts.active_flags = Some(0),
            ForumPostingPolicyFactKind::Reputation => facts.reputation = Some(10),
            ForumPostingPolicyFactKind::RecentModerationActions => {
                facts.recent_moderation_actions = Some(0)
            }
            ForumPostingPolicyFactKind::TopicCreatesWindow => {
                facts.topic_creates_window = Some(ForumPostingWindowCount {
                    count: 0,
                    window_seconds: rules
                        .topic_create_limit
                        .expect("topic limit should exist")
                        .window_seconds,
                })
            }
            ForumPostingPolicyFactKind::ReplyCreatesWindow => {
                facts.reply_creates_window = Some(ForumPostingWindowCount {
                    count: 0,
                    window_seconds: rules
                        .reply_create_limit
                        .expect("reply limit should exist")
                        .window_seconds,
                })
            }
            ForumPostingPolicyFactKind::EditsWindow => {
                facts.edits_window = Some(ForumPostingWindowCount {
                    count: 0,
                    window_seconds: rules
                        .edit_limit
                        .expect("edit limit should exist")
                        .window_seconds,
                })
            }
            ForumPostingPolicyFactKind::SecondsSinceLastBump => {
                facts.seconds_since_last_bump = rules.minimum_seconds_between_bumps
            }
        }
    }
    facts
}

fn input(
    action: ForumPostingAction,
    facts: ForumPostingPolicyFacts,
) -> ForumPostingPolicyEvaluationInput {
    ForumPostingPolicyEvaluationInput {
        tenant_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        action,
        candidate: ForumPostingCandidateMetrics {
            body_bytes: 512,
            link_count: 1,
            mention_count: 1,
            attachment_count: 0,
        },
        facts,
    }
}

#[test]
fn rules_derive_action_scoped_exact_required_facts() {
    let rules = rules();
    let reply = rules
        .required_facts(ForumPostingAction::CreateReply)
        .expect("reply facts should derive");
    assert!(reply.contains(&ForumPostingPolicyFactKind::ReplyCreatesWindow));
    assert!(!reply.contains(&ForumPostingPolicyFactKind::TopicCreatesWindow));
    assert!(!reply.contains(&ForumPostingPolicyFactKind::EditsWindow));
    assert!(!reply.contains(&ForumPostingPolicyFactKind::SecondsSinceLastBump));

    let bump = rules
        .required_facts(ForumPostingAction::BumpTopic)
        .expect("bump facts should derive");
    assert!(bump.contains(&ForumPostingPolicyFactKind::SecondsSinceLastBump));
    assert!(!bump.contains(&ForumPostingPolicyFactKind::ReplyCreatesWindow));
}

#[test]
fn caller_cannot_omit_or_add_required_facts() {
    let rules = rules();
    let mut omitted = facts_for(&rules, ForumPostingAction::CreateReply);
    omitted
        .required_facts
        .retain(|fact| *fact != ForumPostingPolicyFactKind::TrustLevel);
    omitted.trust_level = None;
    let error = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(ForumPostingAction::CreateReply, omitted),
    )
    .expect_err("caller-selected fact omission must fail");
    assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");

    let mut added = facts_for(&rules, ForumPostingAction::CreateReply);
    added
        .required_facts
        .push(ForumPostingPolicyFactKind::TopicCreatesWindow);
    added.topic_creates_window = Some(ForumPostingWindowCount {
        count: 0,
        window_seconds: 86_400,
    });
    let error = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(ForumPostingAction::CreateReply, added),
    )
    .expect_err("caller-selected extra fact must fail");
    assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");
}

#[test]
fn unavailable_facts_precede_partial_denials_in_stable_order() {
    let rules = rules();
    let mut facts = facts_for(&rules, ForumPostingAction::CreateReply);
    facts.trust_level = None;
    facts.active_flags = None;
    facts.unavailable_facts = vec![
        ForumPostingPolicyUnavailableFact {
            fact: ForumPostingPolicyFactKind::TrustLevel,
            retryable: false,
            reason_code: "forum.trust_snapshot_missing".to_string(),
        },
        ForumPostingPolicyUnavailableFact {
            fact: ForumPostingPolicyFactKind::ActiveFlags,
            retryable: true,
            reason_code: "forum.flags_unavailable".to_string(),
        },
    ];

    let decision = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(ForumPostingAction::CreateReply, facts),
    )
    .expect("unavailable facts should produce an incomplete decision");
    assert_eq!(decision.outcome, ForumPostingPolicyOutcome::Indeterminate);
    assert_eq!(
        decision.fact,
        Some(ForumPostingPolicyFactKind::ActiveFlags)
    );
    assert!(decision.retryable);
}

#[test]
fn safety_history_precedes_trust_and_eligibility_denials() {
    let rules = rules();
    let mut facts = facts_for(&rules, ForumPostingAction::CreateReply);
    facts.active_flags = Some(2);
    facts.recent_moderation_actions = Some(1);
    facts.trust_level = Some(1);
    facts.account_age_seconds = Some(1);

    let decision = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(ForumPostingAction::CreateReply, facts),
    )
    .expect("the first failed rule should decide");
    assert_eq!(decision.outcome, ForumPostingPolicyOutcome::Denied);
    assert_eq!(decision.reason, ForumPostingPolicyDecisionReason::ActiveFlags);
}

#[test]
fn action_window_limit_is_deterministic_and_window_bound() {
    let rules = rules();
    let mut facts = facts_for(&rules, ForumPostingAction::CreateReply);
    facts.reply_creates_window = Some(ForumPostingWindowCount {
        count: 5,
        window_seconds: 60,
    });
    let decision = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(ForumPostingAction::CreateReply, facts),
    )
    .expect("an exhausted snapshot should deny the next reply");
    assert_eq!(decision.reason, ForumPostingPolicyDecisionReason::ReplyRateLimit);
    assert_eq!(decision.retry_after_seconds, Some(60));

    let mut mismatched = facts_for(&rules, ForumPostingAction::CreateReply);
    mismatched.reply_creates_window = Some(ForumPostingWindowCount {
        count: 5,
        window_seconds: 61,
    });
    let error = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(ForumPostingAction::CreateReply, mismatched),
    )
    .expect_err("a differently shaped observation window must fail");
    assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");
}

#[test]
fn bump_interval_returns_exact_remaining_delay() {
    let rules = rules();
    let mut facts = facts_for(&rules, ForumPostingAction::BumpTopic);
    facts.seconds_since_last_bump = Some(450);
    let decision = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(ForumPostingAction::BumpTopic, facts),
    )
    .expect("early bump should be denied");
    assert_eq!(decision.reason, ForumPostingPolicyDecisionReason::BumpInterval);
    assert_eq!(decision.retry_after_seconds, Some(150));
}

#[test]
fn candidate_limits_follow_link_mention_attachment_order() {
    let rules = rules();
    let facts = facts_for(&rules, ForumPostingAction::CreateReply);
    let mut request = input(ForumPostingAction::CreateReply, facts);
    request.candidate.link_count = 4;
    request.candidate.mention_count = 6;
    request.candidate.attachment_count = 3;

    let decision = ForumPostingPolicyEvaluator::decide(&rules, request)
        .expect("the first candidate limit should decide");
    assert_eq!(decision.reason, ForumPostingPolicyDecisionReason::LinkLimit);
}

#[test]
fn passing_snapshot_is_allowed_and_body_size_is_not_invented_as_a_rule() {
    let rules = rules();
    let facts = facts_for(&rules, ForumPostingAction::CreateReply);
    let mut request = input(ForumPostingAction::CreateReply, facts.clone());
    request.candidate.body_bytes = 0;
    let first = ForumPostingPolicyEvaluator::decide(&rules, request)
        .expect("passing snapshot should evaluate");

    let mut request = input(ForumPostingAction::CreateReply, facts);
    request.candidate.body_bytes = u32::MAX;
    let second = ForumPostingPolicyEvaluator::decide(&rules, request)
        .expect("26D must not invent an uncontracted body-size rule");

    assert_eq!(first, second);
    assert_eq!(first, rustok_forum::ForumPostingPolicyDecision::allowed());
}

#[test]
fn empty_rules_allow_without_owner_facts() {
    let rules = ForumPostingPolicyRules::default();
    let decision = ForumPostingPolicyEvaluator::decide(
        &rules,
        input(
            ForumPostingAction::CreateTopic,
            ForumPostingPolicyFacts::default(),
        ),
    )
    .expect("empty rules should require no owner facts");
    assert_eq!(decision, rustok_forum::ForumPostingPolicyDecision::allowed());
}

#[test]
fn invalid_noop_or_unbounded_rules_fail_closed() {
    let error = ForumPostingPolicyRules {
        minimum_trust_level: Some(0),
        ..ForumPostingPolicyRules::default()
    }
    .normalize()
    .expect_err("zero trust minimum is a misleading no-op rule");
    assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");

    let error = ForumPostingPolicyRules {
        topic_create_limit: Some(ForumPostingWindowLimit {
            maximum_count: 0,
            window_seconds: 60,
        }),
        ..ForumPostingPolicyRules::default()
    }
    .normalize()
    .expect_err("zero-count windows are not rate-limit rules");
    assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");

    let error = ForumPostingPolicyRules {
        minimum_account_age_seconds: Some(i64::MAX as u64 + 1),
        ..ForumPostingPolicyRules::default()
    }
    .normalize()
    .expect_err("unsigned thresholds must fit typed signed evidence");
    assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");
}

#[test]
fn reserved_future_rules_are_not_in_current_precedence() {
    assert!(!FORUM_POSTING_POLICY_PRECEDENCE
        .contains(&ForumPostingPolicyDecisionReason::DuplicateContent));
    assert!(!FORUM_POSTING_POLICY_PRECEDENCE
        .contains(&ForumPostingPolicyDecisionReason::ExternalSpamScore));
    assert_eq!(
        FORUM_POSTING_POLICY_PRECEDENCE.first(),
        Some(&ForumPostingPolicyDecisionReason::RequiredFactUnavailable)
    );
    assert_eq!(
        FORUM_POSTING_POLICY_PRECEDENCE.last(),
        Some(&ForumPostingPolicyDecisionReason::Allowed)
    );
}
