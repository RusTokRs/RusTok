use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_forum::{
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumPostingAction, ForumPostingCandidateMetrics,
    ForumPostingPolicyCompositionRequest, ForumPostingPolicyDecisionReason,
    ForumPostingPolicyEvaluator, ForumPostingPolicyFactKind,
    ForumPostingPolicyFactsComposer, ForumPostingPolicyOwnerFactPort,
    ForumPostingPolicyOwnerFactRequest, ForumPostingPolicyOwnerFactResponse,
    ForumPostingPolicyOwnerFactValue, ForumPostingPolicyOutcome,
    ForumPostingPolicyRules, ForumPostingTrustFactPort,
    ForumPostingWindowCount, ForumPostingWindowLimit,
    SharedForumAudienceFactsPort, SharedForumPostingPolicyOwnerFactPort,
};
use uuid::Uuid;

fn context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        "forum-posting-facts-test",
    )
    .with_deadline(Duration::from_secs(5))
}

fn request(
    tenant_id: Uuid,
    user_id: Uuid,
    action: ForumPostingAction,
) -> ForumPostingPolicyCompositionRequest {
    ForumPostingPolicyCompositionRequest {
        tenant_id,
        user_id,
        action,
        candidate: ForumPostingCandidateMetrics {
            body_bytes: 512,
            link_count: 1,
            mention_count: 1,
            attachment_count: 0,
        },
    }
}

#[derive(Clone)]
struct RecordingAudienceFactsPort {
    trust_level: u8,
    requests: Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
}

#[async_trait]
impl ForumAudienceFactsPort for RecordingAudienceFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        self.requests.lock().expect("audience request lock").push(request.clone());
        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: Some(self.trust_level),
            channel_memberships: Vec::new(),
            group_memberships: Vec::new(),
        })
    }
}

#[derive(Clone)]
struct ValueFactPort {
    fact: ForumPostingPolicyFactKind,
    value: ForumPostingPolicyOwnerFactValue,
    requests: Arc<Mutex<Vec<ForumPostingPolicyOwnerFactRequest>>>,
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ValueFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        self.fact
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        _context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        self.requests.lock().expect("fact request lock").push(request);
        Ok(ForumPostingPolicyOwnerFactResponse {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            action: request.action,
            fact: request.fact,
            value: self.value,
        })
    }
}

#[derive(Clone)]
struct ErrorFactPort {
    fact: ForumPostingPolicyFactKind,
    error: PortError,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ErrorFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        self.fact
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        _context: PortContext,
        _request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(self.error.clone())
    }
}

#[derive(Clone)]
struct WrongIdentityFactPort;

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for WrongIdentityFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        ForumPostingPolicyFactKind::AccountAgeSeconds
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        _context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        Ok(ForumPostingPolicyOwnerFactResponse {
            tenant_id: Uuid::new_v4(),
            user_id: request.user_id,
            action: request.action,
            fact: request.fact,
            value: ForumPostingPolicyOwnerFactValue::AccountAgeSeconds(86_400),
        })
    }
}

fn value_provider(
    fact: ForumPostingPolicyFactKind,
    value: ForumPostingPolicyOwnerFactValue,
) -> (
    SharedForumPostingPolicyOwnerFactPort,
    Arc<Mutex<Vec<ForumPostingPolicyOwnerFactRequest>>>,
) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(ValueFactPort {
            fact,
            value,
            requests: requests.clone(),
        }),
        requests,
    )
}

#[tokio::test]
async fn authoritative_trust_bridge_composes_exact_fact_and_evaluates() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let audience_requests = Arc::new(Mutex::new(Vec::new()));
    let audience: SharedForumAudienceFactsPort = Arc::new(RecordingAudienceFactsPort {
        trust_level: 25,
        requests: audience_requests.clone(),
    });
    let composer = ForumPostingPolicyFactsComposer::with_trust_audience_facts(audience);
    let rules = ForumPostingPolicyRules {
        minimum_trust_level: Some(20),
        maximum_links: Some(3),
        ..ForumPostingPolicyRules::default()
    };

    let input = composer
        .compose(
            context(tenant_id, user_id),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateReply),
        )
        .await
        .expect("authoritative trust should compose");

    assert_eq!(input.facts.required_facts, vec![ForumPostingPolicyFactKind::TrustLevel]);
    assert_eq!(input.facts.trust_level, Some(25));
    assert!(input.facts.unavailable_facts.is_empty());
    let recorded = audience_requests.lock().expect("audience request lock");
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].include_trust_level);
    assert!(recorded[0].channel_slugs.is_empty());
    assert!(recorded[0].group_ids.is_empty());
    drop(recorded);

    let decision = ForumPostingPolicyEvaluator::decide(&rules, input)
        .expect("composed input should evaluate");
    assert_eq!(decision.outcome, ForumPostingPolicyOutcome::Allowed);
}

#[tokio::test]
async fn missing_provider_is_explicit_and_never_synthesizes_zero() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let rules = ForumPostingPolicyRules {
        minimum_account_age_seconds: Some(86_400),
        ..ForumPostingPolicyRules::default()
    };
    let input = ForumPostingPolicyFactsComposer::default()
        .compose(
            context(tenant_id, user_id),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateTopic),
        )
        .await
        .expect("missing owner should remain explicit");

    assert_eq!(input.facts.account_age_seconds, None);
    assert_eq!(input.facts.unavailable_facts.len(), 1);
    assert_eq!(input.facts.unavailable_facts[0].fact, ForumPostingPolicyFactKind::AccountAgeSeconds);
    assert_eq!(input.facts.unavailable_facts[0].reason_code, "forum.posting_fact.provider_missing");
    assert!(!input.facts.unavailable_facts[0].retryable);
    assert_eq!(
        ForumPostingPolicyEvaluator::decide(&rules, input)
            .expect("unavailable facts should evaluate")
            .outcome,
        ForumPostingPolicyOutcome::Indeterminate
    );
}

#[tokio::test]
async fn retryable_capability_error_becomes_explicit_unavailable_fact() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider: SharedForumPostingPolicyOwnerFactPort = Arc::new(ErrorFactPort {
        fact: ForumPostingPolicyFactKind::AccountAgeSeconds,
        error: PortError::unavailable("profiles.account_age.unavailable", "unavailable"),
        calls: calls.clone(),
    });
    let composer = ForumPostingPolicyFactsComposer::new(vec![provider])
        .expect("one provider should register");
    let rules = ForumPostingPolicyRules {
        minimum_account_age_seconds: Some(86_400),
        ..ForumPostingPolicyRules::default()
    };
    let input = composer
        .compose(
            context(tenant_id, user_id),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateReply),
        )
        .await
        .expect("capability errors should remain fact state");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(input.facts.unavailable_facts[0].reason_code, "profiles.account_age.unavailable");
    assert!(input.facts.unavailable_facts[0].retryable);
}

#[tokio::test]
async fn forbidden_provider_error_is_not_hidden_as_unavailable() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let provider: SharedForumPostingPolicyOwnerFactPort = Arc::new(ErrorFactPort {
        fact: ForumPostingPolicyFactKind::AccountAgeSeconds,
        error: PortError::forbidden("profiles.account_age.forbidden", "forbidden"),
        calls: Arc::new(AtomicUsize::new(0)),
    });
    let composer = ForumPostingPolicyFactsComposer::new(vec![provider])
        .expect("one provider should register");
    let rules = ForumPostingPolicyRules {
        minimum_account_age_seconds: Some(86_400),
        ..ForumPostingPolicyRules::default()
    };
    let error = composer
        .compose(
            context(tenant_id, user_id),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateReply),
        )
        .await
        .expect_err("authorization failures must propagate");
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    assert_eq!(error.code, "profiles.account_age.forbidden");
}

#[tokio::test]
async fn invalid_provider_response_fails_as_invariant_violation() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let composer = ForumPostingPolicyFactsComposer::new(vec![Arc::new(WrongIdentityFactPort)])
        .expect("one provider should register");
    let rules = ForumPostingPolicyRules {
        minimum_account_age_seconds: Some(86_400),
        ..ForumPostingPolicyRules::default()
    };
    let error = composer
        .compose(
            context(tenant_id, user_id),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateReply),
        )
        .await
        .expect_err("mismatched owner identity must fail closed");
    assert_eq!(error.kind, PortErrorKind::InvariantViolation);
    assert_eq!(error.code, "forum.posting_facts.provider_response_invalid");
}

#[test]
fn duplicate_fact_providers_are_rejected() {
    let first = ForumPostingTrustFactPort::shared(Arc::new(RecordingAudienceFactsPort {
        trust_level: 10,
        requests: Arc::new(Mutex::new(Vec::new())),
    }));
    let second = ForumPostingTrustFactPort::shared(Arc::new(RecordingAudienceFactsPort {
        trust_level: 20,
        requests: Arc::new(Mutex::new(Vec::new())),
    }));
    let error = match ForumPostingPolicyFactsComposer::new(vec![first, second]) {
        Ok(_) => panic!("duplicate providers must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.kind, PortErrorKind::Conflict);
    assert_eq!(error.code, "forum.posting_facts.duplicate_provider");
}

#[tokio::test]
async fn exact_actor_context_is_checked_before_provider_access() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let calls = Arc::new(AtomicUsize::new(0));
    let provider: SharedForumPostingPolicyOwnerFactPort = Arc::new(ErrorFactPort {
        fact: ForumPostingPolicyFactKind::AccountAgeSeconds,
        error: PortError::unavailable("should.not.run", "should not run"),
        calls: calls.clone(),
    });
    let composer = ForumPostingPolicyFactsComposer::new(vec![provider])
        .expect("one provider should register");
    let rules = ForumPostingPolicyRules {
        minimum_account_age_seconds: Some(60),
        ..ForumPostingPolicyRules::default()
    };
    let error = composer
        .compose(
            context(tenant_id, Uuid::new_v4()),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateReply),
        )
        .await
        .expect_err("foreign actor must fail before provider access");
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn action_window_request_uses_exact_configured_window() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let (provider, requests) = value_provider(
        ForumPostingPolicyFactKind::ReplyCreatesWindow,
        ForumPostingPolicyOwnerFactValue::ReplyCreatesWindow(ForumPostingWindowCount {
            count: 2,
            window_seconds: 60,
        }),
    );
    let composer = ForumPostingPolicyFactsComposer::new(vec![provider])
        .expect("one provider should register");
    let rules = ForumPostingPolicyRules {
        reply_create_limit: Some(ForumPostingWindowLimit {
            maximum_count: 5,
            window_seconds: 60,
        }),
        ..ForumPostingPolicyRules::default()
    };
    let input = composer
        .compose(
            context(tenant_id, user_id),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateReply),
        )
        .await
        .expect("matching window fact should compose");

    assert_eq!(requests.lock().expect("fact request lock")[0].window_seconds, Some(60));
    assert_eq!(input.facts.reply_creates_window, Some(ForumPostingWindowCount {
        count: 2,
        window_seconds: 60,
    }));
    assert_eq!(
        ForumPostingPolicyEvaluator::decide(&rules, input)
            .expect("window should evaluate")
            .reason,
        ForumPostingPolicyDecisionReason::Allowed
    );
}

#[tokio::test]
async fn mismatched_window_response_is_an_invariant_violation() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let (provider, _) = value_provider(
        ForumPostingPolicyFactKind::ReplyCreatesWindow,
        ForumPostingPolicyOwnerFactValue::ReplyCreatesWindow(ForumPostingWindowCount {
            count: 2,
            window_seconds: 61,
        }),
    );
    let composer = ForumPostingPolicyFactsComposer::new(vec![provider])
        .expect("one provider should register");
    let rules = ForumPostingPolicyRules {
        reply_create_limit: Some(ForumPostingWindowLimit {
            maximum_count: 5,
            window_seconds: 60,
        }),
        ..ForumPostingPolicyRules::default()
    };
    let error = composer
        .compose(
            context(tenant_id, user_id),
            &rules,
            request(tenant_id, user_id, ForumPostingAction::CreateReply),
        )
        .await
        .expect_err("owner window mismatch must fail closed");
    assert_eq!(error.kind, PortErrorKind::InvariantViolation);
}
