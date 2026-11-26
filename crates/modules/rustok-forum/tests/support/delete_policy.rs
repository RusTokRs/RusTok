//! Shared direct-delete admission matrix, mounted by the remediation integration test.
//! Static and transactional policy intentionally differ; no timing/sleep race is needed.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
    StaticModuleSettingsTransactionReader,
};
use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, ForumError, ForumSettingsProviders, ModerationService, ReplyService,
    TopicService,
    entities::{forum_topic_revision, forum_topic_translation},
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, Statement,
};
use uuid::Uuid;

use super::{
    make_category, make_topic, posting_cooldown, reply_input, reply_restore_checkpoint, setup,
    support,
};

#[derive(Clone, Copy, Debug)]
enum Policy {
    Allow,
    Deny,
    Unavailable,
}

struct PolicyReader {
    tenant: Uuid,
    policy: Policy,
    reads: Arc<AtomicUsize>,
}

impl PolicyReader {
    fn snapshot(
        &self,
        tenant: Uuid,
        slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        assert_eq!(tenant, self.tenant);
        assert_eq!(slug, "forum");
        self.reads.fetch_add(1, Ordering::SeqCst);
        if matches!(self.policy, Policy::Unavailable) {
            return Err(PortError::unavailable(
                "settings_down",
                "settings temporarily unavailable",
            ));
        }
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: Some(true),
            settings: serde_json::json!({ "allow_user_content_deletion": matches!(self.policy, Policy::Allow) }),
        }))
    }
}

#[async_trait]
impl StaticModuleSettingsReader for PolicyReader {
    async fn settings(
        &self,
        tenant: Uuid,
        slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        self.snapshot(tenant, slug)
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for PolicyReader {
    async fn settings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        tenant: Uuid,
        slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        self.snapshot(tenant, slug)
    }
}

#[derive(Clone, Copy, Debug)]
enum Actor {
    Author,
    Staff,
    System,
    Other,
}

#[tokio::test]
async fn direct_delete_rechecks_policy_on_sqlite() {
    let (db, events, _) = setup().await;
    exercise_delete_admission(db, events).await;
}

#[tokio::test]
async fn direct_delete_rechecks_policy_on_postgres() -> support::TestResult<()> {
    let Some(context) = support::postgres::PostgresForumTestDb::setup("delete_policy").await?
    else {
        return Ok(());
    };
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(context.db.clone())));
    exercise_delete_admission(context.db.clone(), events).await;
    context.cleanup().await?;
    Ok(())
}

async fn exercise_delete_admission(db: DatabaseConnection, events: TransactionalEventBus) {
    use Actor::*;
    use Policy::*;
    for is_reply in [false, true] {
        for (preflight, transactional, actor, expected_reason) in [
            (Allow, Some(Deny), Author, None),
            (Allow, Some(Unavailable), Author, None),
            (Allow, None, Author, None),
            (Allow, None, Staff, None),
            (Allow, None, System, None),
            (Deny, Some(Allow), Author, None),
            (Allow, Some(Allow), Author, Some("user_request")),
            (Deny, Some(Deny), Staff, Some("moderation")),
            (Allow, Some(Unavailable), Staff, None),
            (Deny, Some(Deny), System, Some("system")),
            (Allow, Some(Allow), Other, None),
            (Unavailable, Some(Allow), Author, None),
        ] {
            let label = format!(
                "reply={is_reply}, preflight={preflight:?}, transactional={transactional:?}, actor={actor:?}"
            );
            let tenant = Uuid::new_v4();
            let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
            let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
            let create_policy = posting_cooldown::zero_cooldown_providers();
            let topics = TopicService::new(db.clone(), events.clone())
                .with_settings_providers(create_policy.clone());
            let replies = ReplyService::new(db.clone(), events.clone())
                .with_settings_providers(create_policy);
            let category = make_category(
                &CategoryService::new(db.clone()),
                tenant,
                &admin,
                "delete-policy",
                false,
            )
            .await;
            let topic = make_topic(&topics, tenant, &author, category, "delete-policy").await;
            let reply = replies
                .create(
                    tenant,
                    author.clone(),
                    topic,
                    reply_input("accepted body", None),
                )
                .await
                .unwrap();
            ModerationService::new(db.clone(), events.clone())
                .mark_solution(tenant, topic, reply.id, admin.clone())
                .await
                .unwrap();
            let static_reads = Arc::new(AtomicUsize::new(0));
            let tx_reads = Arc::new(AtomicUsize::new(0));
            let static_reader = SharedStaticModuleSettingsReader(Arc::new(PolicyReader {
                tenant,
                policy: preflight,
                reads: static_reads.clone(),
            }));
            let providers = if let Some(policy) = transactional {
                ForumSettingsProviders::default().with_static_readers(
                    static_reader,
                    SharedStaticModuleSettingsTransactionReader(Arc::new(PolicyReader {
                        tenant,
                        policy,
                        reads: tx_reads.clone(),
                    })),
                )
            } else {
                ForumSettingsProviders::default().with_static_reader(static_reader)
            };
            let security = match actor {
                Author => author,
                Staff => admin,
                System => SecurityContext::system(),
                Other => SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4())),
            };
            let before = checkpoint(&db, tenant, topic, reply.id).await;
            let result = if is_reply {
                ReplyService::new(db.clone(), events.clone())
                    .with_settings_providers(providers)
                    .delete(tenant, reply.id, security)
                    .await
            } else {
                TopicService::new(db.clone(), events.clone())
                    .with_settings_providers(providers)
                    .delete(tenant, topic, security)
                    .await
            };
            assert_eq!(
                result.is_ok(),
                expected_reason.is_some(),
                "{label}: {result:?}"
            );
            let denied_before_tx = matches!(actor, Other)
                || matches!(preflight, Unavailable)
                || (matches!(actor, Author) && matches!(preflight, Deny));
            assert_eq!(
                static_reads.load(Ordering::SeqCst),
                usize::from(!matches!(actor, Other)),
                "{label}"
            );
            assert_eq!(
                tx_reads.load(Ordering::SeqCst),
                usize::from(!denied_before_tx && transactional.is_some()),
                "{label}: direct delete must consult transactional policy after the write fence"
            );
            if !denied_before_tx && transactional.is_none() {
                assert!(
                    matches!(result, Err(ref error) if error.stable_code() == "FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE"),
                    "{label}: {result:?}"
                );
            }
            if let Some(reason) = expected_reason {
                let (table, column, id) = if is_reply {
                    ("forum_reply_delete_snapshots", "reply_id", reply.id)
                } else {
                    ("forum_topic_delete_snapshots", "topic_id", topic)
                };
                let saved = db
                    .query_one_raw(Statement::from_sql_and_values(
                        db.get_database_backend(),
                        format!(
                            "SELECT reason FROM {table} WHERE tenant_id = $1 AND {column} = $2"
                        ),
                        vec![tenant.into(), id.into()],
                    ))
                    .await
                    .unwrap()
                    .unwrap()
                    .try_get::<String>("", "reason")
                    .unwrap();
                assert_eq!(
                    saved, reason,
                    "{label}: snapshot reason must reflect admitted authority"
                );
            } else {
                assert_eq!(
                    checkpoint(&db, tenant, topic, reply.id).await,
                    before,
                    "{label}: denied deletion must preserve sources, snapshots, history, solution, counters and outbox"
                );
                if matches!(preflight, Unavailable)
                    || (!denied_before_tx && matches!(transactional, Some(Unavailable)))
                {
                    assert!(
                        matches!(result, Err(ForumError::CapabilityFailure { .. })),
                        "{label}: {result:?}"
                    );
                } else if !denied_before_tx && transactional.is_none() {
                    assert!(
                        matches!(result, Err(ForumError::CapabilityUnavailable { .. })),
                        "{label}: {result:?}"
                    );
                } else {
                    assert!(
                        matches!(result, Err(ForumError::Forbidden(_))),
                        "{label}: {result:?}"
                    );
                }
            }
        }
    }
}

async fn checkpoint(
    db: &DatabaseConnection,
    tenant: Uuid,
    topic: Uuid,
    reply: Uuid,
) -> serde_json::Value {
    let source = reply_restore_checkpoint(db, tenant, topic, reply).await;
    let translations = forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TenantId.eq(tenant))
        .filter(forum_topic_translation::Column::TopicId.eq(topic))
        .order_by_asc(forum_topic_translation::Column::Locale)
        .all(db)
        .await
        .unwrap();
    let revisions = forum_topic_revision::Entity::find()
        .filter(forum_topic_revision::Column::TenantId.eq(tenant))
        .filter(forum_topic_revision::Column::TopicId.eq(topic))
        .order_by_asc(forum_topic_revision::Column::Id)
        .all(db)
        .await
        .unwrap();
    let mut snapshots = Vec::new();
    for table in [
        "forum_topic_delete_snapshots",
        "forum_topic_reply_delete_snapshots",
        "forum_reply_delete_snapshots",
    ] {
        let count = db
            .query_one_raw(Statement::from_sql_and_values(
                db.get_database_backend(),
                format!("SELECT COUNT(*) AS count FROM {table} WHERE tenant_id = $1"),
                vec![tenant.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "count")
            .unwrap();
        snapshots.push(count);
    }
    serde_json::json!({ "source": source, "translations": translations, "revisions": revisions, "snapshots": snapshots })
}
