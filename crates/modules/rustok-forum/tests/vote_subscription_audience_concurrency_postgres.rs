mod support;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumCategoryAudiencePolicyService, ForumError, SetForumCategoryAudiencePolicyInput,
    SubscriptionService, TopicService, VoteService,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, Statement,
    TransactionTrait,
};
use tokio::time::timeout;
use uuid::Uuid;

use support::TestResult;
use support::postgres::PostgresForumTestDb;

struct FixedTrustFacts;

#[async_trait]
impl ForumAudienceFactsPort for FixedTrustFacts {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: request.include_trust_level.then_some(8),
            ..ForumAudienceFacts::default()
        })
    }
}

fn viewer_context(tenant: Uuid, user: Uuid) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(user.to_string()),
        "en",
        "forum-transaction-audience-race",
    )
}

async fn lock_policy_scope(txn: &DatabaseTransaction, tenant: Uuid) -> TestResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        [tenant.to_string().into()],
    ))
    .await?;
    Ok(())
}

async fn raise_trust_threshold(
    txn: &DatabaseTransaction,
    tenant: Uuid,
    category: Uuid,
) -> TestResult<()> {
    let result = txn
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE forum_category_audience_policies
             SET minimum_trust_level = 9, updated_at = NOW()
             WHERE tenant_id = $1 AND category_id = $2",
            [tenant.into(), category.into()],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1, "test policy must exist");
    Ok(())
}

async fn row_count(
    db: &DatabaseConnection,
    tenant: Uuid,
    subject: Uuid,
    sql: &'static str,
) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [tenant.into(), subject.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("missing count row"))?;
    Ok(row.try_get("", "value")?)
}

#[tokio::test]
async fn policy_revocation_serializes_with_vote_and_subscription_writes() -> TestResult<()> {
    let Some(database) = PostgresForumTestDb::setup("write_audience_race").await? else {
        return Ok(());
    };
    let outcome = exercise(&database).await;
    database.cleanup().await?;
    outcome
}

async fn exercise(database: &PostgresForumTestDb) -> TestResult<()> {
    let tenant = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let user_id = Uuid::new_v4();
    let viewer = SecurityContext::new(UserRole::Customer, Some(user_id));
    let category = CategoryService::new(database.db.clone())
        .create(
            tenant,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Audience lock".into(),
                slug: "audience-lock".into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.db.clone())));
    let topic = TopicService::new(database.db.clone(), event_bus)
        .create(
            tenant,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Forum audience lock".into(),
                slug: Some("forum-audience-lock".into()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?;

    let policy_service = ForumCategoryAudiencePolicyService::new(database.db.clone());
    let input = |minimum_trust_level| SetForumCategoryAudiencePolicyInput {
        constraints: ForumAudienceConstraints {
            minimum_trust_level: Some(minimum_trust_level),
            ..Default::default()
        },
    };
    policy_service
        .set(tenant, category.id, admin.clone(), input(5))
        .await?;

    // Hold the policy writer scope before starting the vote. The vote's
    // transaction must wait; then it must see the newly committed rule.
    let holder = database.db.begin().await?;
    lock_policy_scope(&holder, tenant).await?;
    let vote_db = database.peer().await?;
    let vote_viewer = viewer.clone();
    let mut vote = tokio::spawn(async move {
        VoteService::new(vote_db)
            .with_audience_facts(Arc::new(FixedTrustFacts))
            .set_topic_vote(
                tenant,
                topic.id,
                vote_viewer,
                viewer_context(tenant, user_id),
                1,
            )
            .await
    });
    assert!(
        timeout(Duration::from_millis(120), &mut vote)
            .await
            .is_err(),
        "vote must wait while the policy writer owns the advisory lock"
    );
    raise_trust_threshold(&holder, tenant, category.id).await?;
    holder.commit().await?;
    let vote_result = timeout(Duration::from_secs(5), vote).await??;
    assert!(
        matches!(&vote_result, Err(ForumError::TopicNotFound(id)) if *id == topic.id),
        "the vote must be denied against the newly committed policy: {vote_result:?}"
    );
    assert_eq!(
        row_count(
            &database.db,
            tenant,
            topic.id,
            "SELECT COUNT(*)::BIGINT AS value FROM forum_topic_votes
             WHERE tenant_id = $1 AND topic_id = $2",
        )
        .await?,
        0
    );

    // Repeat with the subscription writer: it must honor the same policy
    // lock and must not persist a watching subscription after revocation.
    policy_service
        .set(tenant, category.id, admin, input(5))
        .await?;
    let holder = database.db.begin().await?;
    lock_policy_scope(&holder, tenant).await?;
    let subscription_db = database.peer().await?;
    let mut subscription = tokio::spawn(async move {
        SubscriptionService::new(subscription_db)
            .with_audience_facts(Arc::new(FixedTrustFacts))
            .set_topic_subscription(tenant, topic.id, viewer, viewer_context(tenant, user_id))
            .await
    });
    assert!(
        timeout(Duration::from_millis(120), &mut subscription)
            .await
            .is_err(),
        "subscription must wait while the policy owner holds its scope"
    );
    raise_trust_threshold(&holder, tenant, category.id).await?;
    holder.commit().await?;
    let result = timeout(Duration::from_secs(5), subscription).await??;
    assert!(
        matches!(&result, Err(ForumError::TopicNotFound(id)) if *id == topic.id),
        "subscription must use the committed restrictive policy: {result:?}"
    );
    assert_eq!(
        row_count(
            &database.db,
            tenant,
            topic.id,
            "SELECT COUNT(*)::BIGINT AS value FROM forum_topic_subscriptions
             WHERE tenant_id = $1 AND topic_id = $2",
        )
        .await?,
        0
    );
    Ok(())
}
