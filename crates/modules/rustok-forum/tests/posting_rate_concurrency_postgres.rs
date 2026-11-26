//! Concurrent posting-rate enforcement on Postgres.
//!
//! Topic create takes the tenant category-tree advisory lock before the
//! per-author cooldown check. Two overlapping creates by the same user must
//! serialize on that lock so exactly one write commits and the other is
//! `RateLimited`. SQLite cannot exercise this: `lock_category_tree_in_tx` is a
//! no-op there.

mod support;

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
    StaticModuleSettingsTransactionReader,
};
use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumSettingsProviders,
    TopicService,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use support::TestResult;
use support::postgres::PostgresForumTestDb;

struct CooldownSettings;

fn snapshot(module_slug: &str) -> Option<StaticModuleSettingsSnapshot> {
    Some(StaticModuleSettingsSnapshot {
        enabled: Some(module_slug == "forum"),
        settings: serde_json::json!({
            "rate_limit_new_topic_seconds": 60,
            "rate_limit_new_reply_seconds": 60,
        }),
    })
}

#[async_trait]
impl StaticModuleSettingsReader for CooldownSettings {
    async fn settings(
        &self,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(snapshot(module_slug))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for CooldownSettings {
    async fn settings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(snapshot(module_slug))
    }
}

fn settings_providers() -> ForumSettingsProviders {
    let reader = Arc::new(CooldownSettings);
    ForumSettingsProviders::default().with_static_readers(
        SharedStaticModuleSettingsReader(reader.clone()),
        SharedStaticModuleSettingsTransactionReader(reader),
    )
}

fn topic_input(category_id: Uuid, slug: &str) -> CreateTopicInput {
    CreateTopicInput {
        locale: "en".into(),
        category_id,
        title: format!("Concurrent {slug}"),
        slug: Some(slug.into()),
        body: rustok_api::RichTextDocument::single_paragraph("Body"),
        metadata: serde_json::json!({}),
        tags: Vec::new(),
        channel_slugs: None,
    }
}

#[tokio::test]
async fn concurrent_topic_creates_by_the_same_author_honor_the_cooldown() -> TestResult<()> {
    let Some(database) = PostgresForumTestDb::setup("posting_rate_concurrency").await? else {
        return Ok(());
    };
    let outcome = exercise(&database).await;
    database.cleanup().await?;
    outcome
}

async fn exercise(database: &PostgresForumTestDb) -> TestResult<()> {
    let tenant = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author_id = Uuid::new_v4();
    let author = SecurityContext::new(UserRole::Customer, Some(author_id));
    let category = CategoryService::new(database.db.clone())
        .create(
            tenant,
            admin,
            CreateCategoryInput {
                locale: "en".into(),
                name: "Rate lock".into(),
                slug: "rate-lock".into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;

    let first_db = database.peer().await?;
    let second_db = database.peer().await?;
    let first_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(first_db.clone())));
    let second_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(second_db.clone())));
    let first_author = author.clone();
    let second_author = author.clone();
    let first_input = topic_input(category.id, "rate-one");
    let second_input = topic_input(category.id, "rate-two");

    let first = tokio::spawn(async move {
        TopicService::new(first_db, first_bus)
            .with_settings_providers(settings_providers())
            .create(tenant, first_author, first_input)
            .await
    });
    let second = tokio::spawn(async move {
        TopicService::new(second_db, second_bus)
            .with_settings_providers(settings_providers())
            .create(tenant, second_author, second_input)
            .await
    });

    let first_result = first.await?;
    let second_result = second.await?;
    let outcomes = [first_result, second_result];
    let successes = outcomes.iter().filter(|result| result.is_ok()).count();
    let limited = outcomes
        .iter()
        .filter(|result| matches!(result, Err(ForumError::RateLimited { .. })))
        .count();
    assert_eq!(
        successes, 1,
        "exactly one concurrent create must commit: {outcomes:?}"
    );
    assert_eq!(
        limited, 1,
        "the loser must be rate-limited after the winner commits: {outcomes:?}"
    );
    Ok(())
}
