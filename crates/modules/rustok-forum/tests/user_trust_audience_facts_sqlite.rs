use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{Permission, PortActor, PortContext, PortError, PortErrorKind};
use rustok_core::SecurityContext;
use rustok_forum::{
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumUserTrustAudienceFactsPort, ForumUserTrustService, SetForumUserTrustInput,
    SharedForumAudienceFactsPort,
};
use sea_orm::{ConnectionTrait, Database};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (sea_orm::DatabaseConnection, Uuid, Uuid, Uuid) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite trust facts database should connect");
    db.execute_unprepared(
        r#"
PRAGMA foreign_keys = ON;
CREATE TABLE users (
    id BLOB PRIMARY KEY NOT NULL,
    tenant_id BLOB NOT NULL,
    email TEXT NOT NULL
);
CREATE TABLE forum_user_stats (
    tenant_id BLOB NOT NULL,
    user_id BLOB NOT NULL,
    topic_count INTEGER NOT NULL DEFAULT 0,
    reply_count INTEGER NOT NULL DEFAULT 0,
    solution_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, user_id)
);
"#,
    )
    .await
    .expect("trust facts prerequisites should be created");

    let migration = rustok_forum::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20260728_000004_add_forum_user_trust_state")
        .expect("FORUM-26A trust migration should be registered");
    migration
        .up(&SchemaManager::new(&db))
        .await
        .expect("FORUM-26A trust migration should apply");

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO users (id, tenant_id, email) VALUES \
         (X'{}', X'{}', 'actor@example.invalid'), \
         (X'{}', X'{}', 'member@example.invalid')",
        actor_id.simple().to_string().to_uppercase(),
        tenant_id.simple().to_string().to_uppercase(),
        user_id.simple().to_string().to_uppercase(),
        tenant_id.simple().to_string().to_uppercase(),
    ))
    .await
    .expect("trust facts users should be inserted");

    (db, tenant_id, actor_id, user_id)
}

fn manager(actor_id: Uuid) -> SecurityContext {
    SecurityContext::from_permission_snapshot(Some(actor_id), &[Permission::FORUM_TOPICS_MANAGE])
}

fn context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        "forum-trust-facts-test",
    )
    .with_deadline(Duration::from_secs(2))
}

fn request(
    tenant_id: Uuid,
    user_id: Uuid,
    include_trust_level: bool,
    group_ids: Vec<Uuid>,
) -> ForumAudienceFactsRequest {
    ForumAudienceFactsRequest {
        tenant_id,
        user_id,
        include_trust_level,
        channel_slugs: Vec::new(),
        group_ids,
    }
}

#[derive(Clone)]
struct StaticMembershipFactsPort {
    active_group: Option<Uuid>,
    calls: Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
}

#[async_trait]
impl ForumAudienceFactsPort for StaticMembershipFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        self.calls
            .lock()
            .expect("membership call recorder should stay available")
            .push(request.clone());
        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: None,
            channel_memberships: Vec::new(),
            group_memberships: self
                .active_group
                .filter(|group_id| request.group_ids.binary_search(group_id).is_ok())
                .into_iter()
                .collect(),
        })
    }
}

fn membership_port(
    active_group: Option<Uuid>,
) -> (
    SharedForumAudienceFactsPort,
    Arc<Mutex<Vec<ForumAudienceFactsRequest>>>,
) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(StaticMembershipFactsPort {
            active_group,
            calls: calls.clone(),
        }),
        calls,
    )
}

#[tokio::test]
async fn absent_authoritative_state_is_zero_and_activity_counters_are_not_trust() {
    let (db, tenant_id, _, user_id) = setup().await;
    db.execute_unprepared(&format!(
        "INSERT INTO forum_user_stats \
         (tenant_id, user_id, topic_count, reply_count, solution_count) \
         VALUES ('{tenant_id}', '{user_id}', 900, 1200, 300)"
    ))
    .await
    .expect("activity counters should be stored independently");

    let facts = ForumUserTrustAudienceFactsPort::new(db)
        .resolve_forum_audience_facts(
            context(tenant_id, user_id),
            request(tenant_id, user_id, true, Vec::new()),
        )
        .await
        .expect("missing authoritative state should resolve to zero");

    assert_eq!(facts.trust_level, Some(0));
    assert!(facts.channel_memberships.is_empty());
    assert!(facts.group_memberships.is_empty());
}

#[tokio::test]
async fn managed_authoritative_state_is_published_as_exact_actor_trust() {
    let (db, tenant_id, actor_id, user_id) = setup().await;
    ForumUserTrustService::new(db.clone())
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            SetForumUserTrustInput {
                trust_level: 42,
                reason_code: "manual_review".to_string(),
                reason_summary: "Approved trust facts fixture".to_string(),
                idempotency_key: "trust-facts-42".to_string(),
            },
        )
        .await
        .expect("managed trust state should be stored");

    let facts = ForumUserTrustAudienceFactsPort::new(db)
        .resolve_forum_audience_facts(
            context(tenant_id, user_id),
            request(tenant_id, user_id, true, Vec::new()),
        )
        .await
        .expect("authoritative trust state should resolve");

    assert_eq!(facts.tenant_id, tenant_id);
    assert_eq!(facts.user_id, user_id);
    assert_eq!(facts.trust_level, Some(42));
}

#[tokio::test]
async fn membership_match_short_circuits_trust_storage_and_disables_delegated_trust() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("empty SQLite database should connect");
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let active_group = Uuid::new_v4();
    let (membership_facts, calls) = membership_port(Some(active_group));

    let facts = ForumUserTrustAudienceFactsPort::with_membership_facts(db, membership_facts)
        .resolve_forum_audience_facts(
            context(tenant_id, user_id),
            request(tenant_id, user_id, true, vec![active_group]),
        )
        .await
        .expect("membership match should decide without trust storage");

    assert_eq!(facts.group_memberships, vec![active_group]);
    assert_eq!(facts.trust_level, None);
    let delegated = calls
        .lock()
        .expect("membership call recorder should stay available");
    assert_eq!(delegated.len(), 1);
    assert!(!delegated[0].include_trust_level);
    assert_eq!(delegated[0].group_ids, vec![active_group]);
}

#[tokio::test]
async fn confirmed_membership_miss_falls_through_to_authoritative_trust() {
    let (db, tenant_id, actor_id, user_id) = setup().await;
    ForumUserTrustService::new(db.clone())
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            SetForumUserTrustInput {
                trust_level: 25,
                reason_code: "manual_review".to_string(),
                reason_summary: "Trust fallback fixture".to_string(),
                idempotency_key: "trust-facts-fallback".to_string(),
            },
        )
        .await
        .expect("managed trust state should be stored");
    let missing_group = Uuid::new_v4();
    let (membership_facts, _) = membership_port(None);

    let facts = ForumUserTrustAudienceFactsPort::with_membership_facts(db, membership_facts)
        .resolve_forum_audience_facts(
            context(tenant_id, user_id),
            request(tenant_id, user_id, true, vec![missing_group]),
        )
        .await
        .expect("confirmed membership miss should use trust");

    assert_eq!(facts.trust_level, Some(25));
    assert!(facts.group_memberships.is_empty());
}

#[tokio::test]
async fn membership_request_without_provider_and_foreign_actor_fail_closed() {
    let (db, tenant_id, _, user_id) = setup().await;
    let adapter = ForumUserTrustAudienceFactsPort::new(db);
    let unavailable = adapter
        .resolve_forum_audience_facts(
            context(tenant_id, user_id),
            request(tenant_id, user_id, true, vec![Uuid::new_v4()]),
        )
        .await
        .expect_err("unresolved membership must not become a negative fact");
    assert_eq!(unavailable.kind, PortErrorKind::Unavailable);
    assert!(unavailable.retryable);

    let forbidden = adapter
        .resolve_forum_audience_facts(
            context(tenant_id, Uuid::new_v4()),
            request(tenant_id, user_id, true, Vec::new()),
        )
        .await
        .expect_err("foreign actor trust lookup must fail closed");
    assert_eq!(forbidden.kind, PortErrorKind::Forbidden);
}
