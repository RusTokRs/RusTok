use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE, ForumAudienceConstraints,
    ForumAudienceDecisionReason, ForumAudienceEvaluator, ForumAudienceFacts,
    ForumAudienceFactsPort, ForumAudienceFactsRequest, ForumAudienceFactsResolver, ForumError,
    MAX_FORUM_AUDIENCE_CHANNELS,
};
use uuid::Uuid;

#[derive(Clone)]
struct StaticFactsPort {
    facts: ForumAudienceFacts,
}

#[async_trait]
impl ForumAudienceFactsPort for StaticFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        _request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        Ok(self.facts.clone())
    }
}

fn read_context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        "forum-audience-contract",
    )
    .with_deadline(Duration::from_secs(1))
}

#[test]
fn audience_constraints_are_bounded_and_canonical() {
    let group_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let normalized = ForumAudienceConstraints {
        roles_any: vec![UserRole::Manager, UserRole::Manager, UserRole::Admin],
        minimum_trust_level: Some(7),
        channel_members_any: vec!["  Team  ".into(), "team".into()],
        group_members_any: vec![group_id, group_id],
        allow_user_ids: vec![user_id, user_id],
        deny_user_ids: vec![user_id, user_id],
    }
    .normalize()
    .expect("bounded audience constraints should normalize");

    assert_eq!(normalized.roles_any, vec![UserRole::Manager, UserRole::Admin]);
    assert_eq!(normalized.channel_members_any, vec!["team"]);
    assert_eq!(normalized.group_members_any, vec![group_id]);
    assert_eq!(normalized.allow_user_ids, vec![user_id]);
    assert_eq!(normalized.deny_user_ids, vec![user_id]);

    let oversized = ForumAudienceConstraints {
        channel_members_any: (0..=MAX_FORUM_AUDIENCE_CHANNELS)
            .map(|index| format!("channel-{index}"))
            .collect(),
        ..ForumAudienceConstraints::default()
    };
    assert!(matches!(
        oversized.normalize(),
        Err(ForumError::Validation(message))
            if message.contains("channel memberships")
    ));
}

#[test]
fn explicit_deny_wins_and_positive_selectors_are_a_union() {
    let user_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Manager, Some(user_id));
    let constraints = ForumAudienceConstraints {
        roles_any: vec![UserRole::Manager],
        minimum_trust_level: Some(50),
        allow_user_ids: vec![user_id],
        deny_user_ids: vec![user_id],
        ..ForumAudienceConstraints::default()
    };
    let denied = ForumAudienceEvaluator::decide(
        &constraints,
        &security,
        &ForumAudienceFacts::default(),
    )
    .expect("audience decision should resolve");
    assert!(!denied.allowed);
    assert_eq!(denied.reason, ForumAudienceDecisionReason::ExplicitDeny);

    let role_allowed = ForumAudienceEvaluator::decide(
        &ForumAudienceConstraints {
            roles_any: vec![UserRole::Manager],
            minimum_trust_level: Some(50),
            ..ForumAudienceConstraints::default()
        },
        &security,
        &ForumAudienceFacts::default(),
    )
    .expect("role audience should resolve");
    assert!(role_allowed.allowed);
    assert_eq!(role_allowed.reason, ForumAudienceDecisionReason::Role);

    let public_denied = ForumAudienceEvaluator::decide(
        &ForumAudienceConstraints {
            roles_any: vec![UserRole::Customer],
            ..ForumAudienceConstraints::default()
        },
        &SecurityContext::public_read(),
        &ForumAudienceFacts::default(),
    )
    .expect("public audience decision should resolve");
    assert!(!public_denied.allowed);
    assert_eq!(
        public_denied.reason,
        ForumAudienceDecisionReason::AuthenticationRequired
    );
}

#[test]
fn exact_owner_facts_reject_unrequested_memberships() {
    let user_id = Uuid::new_v4();
    let requested_group = Uuid::new_v4();
    let unrequested_group = Uuid::new_v4();
    let request = ForumAudienceFactsRequest::for_constraints(
        user_id,
        &ForumAudienceConstraints {
            minimum_trust_level: Some(1),
            channel_members_any: vec!["members".into()],
            group_members_any: vec![requested_group],
            ..ForumAudienceConstraints::default()
        },
    )
    .expect("facts request should normalize");

    assert!(matches!(
        ForumAudienceFacts {
            trust_level: Some(10),
            channel_memberships: vec!["other".into()],
            group_memberships: vec![requested_group],
        }
        .validate_for_request(&request),
        Err(ForumError::Validation(message))
            if message.contains("unrequested channel membership")
    ));
    assert!(matches!(
        ForumAudienceFacts {
            trust_level: Some(10),
            channel_memberships: vec!["members".into()],
            group_memberships: vec![unrequested_group],
        }
        .validate_for_request(&request),
        Err(ForumError::Validation(message))
            if message.contains("unrequested group membership")
    ));
}

#[tokio::test]
async fn resolver_is_fail_closed_and_requires_read_deadline_semantics() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let constraints = ForumAudienceConstraints {
        minimum_trust_level: Some(5),
        channel_members_any: vec!["members".into()],
        ..ForumAudienceConstraints::default()
    };

    let public_facts = ForumAudienceFactsResolver::default()
        .resolve_for_constraints(
            read_context(tenant_id, user_id),
            &SecurityContext::public_read(),
            &constraints,
        )
        .await
        .expect("public actors should fail closed without calling an optional provider");
    assert_eq!(public_facts, ForumAudienceFacts::default());

    let authenticated = SecurityContext::new(UserRole::Customer, Some(user_id));
    assert!(matches!(
        ForumAudienceFactsResolver::default()
            .resolve_for_constraints(
                read_context(tenant_id, user_id),
                &authenticated,
                &constraints,
            )
            .await,
        Err(ForumError::CapabilityUnavailable { code, .. })
            if code == FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE
    ));

    let resolver = ForumAudienceFactsResolver::new(Some(Arc::new(StaticFactsPort {
        facts: ForumAudienceFacts {
            trust_level: Some(8),
            channel_memberships: vec!["MEMBERS".into()],
            group_memberships: Vec::new(),
        },
    })));
    let facts = resolver
        .resolve_for_constraints(
            read_context(tenant_id, user_id),
            &authenticated,
            &constraints,
        )
        .await
        .expect("exact owner facts should resolve");
    assert_eq!(facts.trust_level, Some(8));
    assert_eq!(facts.channel_memberships, vec!["members"]);

    let missing_deadline = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        "forum-audience-contract-no-deadline",
    );
    assert!(matches!(
        resolver
            .resolve_for_constraints(missing_deadline, &authenticated, &constraints)
            .await,
        Err(ForumError::CapabilityFailure { source_code, .. })
            if source_code == "port.deadline_required"
    ));
}
