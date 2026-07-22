mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CreateReplyCommandInput, CreateTopicInput, ForumQuoteCommandService,
    ForumQuoteReferenceInput, ForumQuoteTargetKindInput, ReplyService, SetForumQuotesInput,
    TopicService, UpdateReplyInput,
};
use rustok_outbox::{entity as outbox_entity, OutboxTransport, SysEvents, TransactionalEventBus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    Statement, TransactionTrait,
};
use tokio::time::sleep;
use uuid::Uuid;

use support::postgres::{execute, PostgresForumTestDb};
use support::{test_error, TestResult};

const LOCALE: &str = "en";
const ORIGINAL_REPLY_BODY: &str = "Original quoted reply";

struct QuoteFixture {
    tenant_id: Uuid,
    reply_id: Uuid,
    author_id: Uuid,
}

#[tokio::test]
async fn d1_replacement_wins_before_stale_d2_preserve_on_postgres() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("mention_quote_cas").await? else {
        return Ok(());
    };

    let outcome = async {
        let fixture = create_quote_fixture(&context).await?;
        let revision_count_before = relation_revision_count(
            &context.db,
            fixture.tenant_id,
            "reply",
            fixture.reply_id,
        )
        .await?;

        let blocker_db = context.peer().await?;
        let blocker = blocker_db.begin().await?;
        blocker
            .query_one(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT id FROM forum_replies \
                     WHERE tenant_id = '{}' AND id = '{}' FOR UPDATE",
                    fixture.tenant_id, fixture.reply_id
                ),
            ))
            .await?
            .ok_or_else(|| test_error("reply lock source was not found"))?;

        let d1_name = unique_application_name("forum_d1_clear");
        let d1_db = context.peer().await?;
        set_application_name(&d1_db, &d1_name).await?;
        let d1_security = admin_security(fixture.author_id);
        let tenant_id = fixture.tenant_id;
        let reply_id = fixture.reply_id;
        let d1 = tokio::spawn(async move {
            ForumQuoteCommandService::new(d1_db)
                .set_reply_quotes(
                    tenant_id,
                    reply_id,
                    d1_security,
                    SetForumQuotesInput {
                        locale: LOCALE.to_string(),
                        quotes: Vec::new(),
                    },
                )
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        wait_until_lock_wait(&context.db, &d1_name).await?;

        let d2_name = unique_application_name("forum_d2_preserve");
        let d2_db = context.peer().await?;
        set_application_name(&d2_db, &d2_name).await?;
        let d2_security = admin_security(fixture.author_id);
        let tenant_id = fixture.tenant_id;
        let reply_id = fixture.reply_id;
        let d2 = tokio::spawn(async move {
            let service = ReplyService::new(d2_db.clone(), event_bus(d2_db));
            match service
                .update(
                    tenant_id,
                    reply_id,
                    d2_security,
                    UpdateReplyInput {
                        locale: LOCALE.to_string(),
                        content: Some("Stale D2 body must roll back".to_string()),
                        content_format: None,
                        content_json: None,
                    },
                )
                .await
            {
                Ok(_) => Err("stale D2 preserve unexpectedly committed".to_string()),
                Err(error) => Ok((error.stable_code().to_string(), error.is_retryable())),
            }
        });
        wait_until_lock_wait(&context.db, &d2_name).await?;

        blocker.commit().await?;

        d1.await
            .map_err(|error| test_error(format!("D1 task failed to join: {error}")))?
            .map_err(test_error)?;
        let (code, retryable) = d2
            .await
            .map_err(|error| test_error(format!("D2 task failed to join: {error}")))?
            .map_err(test_error)?;
        if code != "FORUM_RELATION_REVISION_CONFLICT" || !retryable {
            return Err(test_error(format!(
                "unexpected D2 conflict: code={code}, retryable={retryable}"
            )));
        }

        let persisted_body = scalar_string(
            &context.db,
            format!(
                "SELECT body AS value FROM forum_reply_bodies \
                 WHERE tenant_id = '{}' AND reply_id = '{}' AND locale = '{}'",
                fixture.tenant_id, fixture.reply_id, LOCALE
            ),
        )
        .await?;
        if persisted_body != ORIGINAL_REPLY_BODY {
            return Err(test_error(format!(
                "stale D2 body escaped rollback: {persisted_body}"
            )));
        }

        let revision_count_after = relation_revision_count(
            &context.db,
            fixture.tenant_id,
            "reply",
            fixture.reply_id,
        )
        .await?;
        if revision_count_after != revision_count_before + 1 {
            return Err(test_error(format!(
                "expected only D1 to append one revision: before={revision_count_before}, after={revision_count_after}"
            )));
        }
        if latest_quote_count(
            &context.db,
            fixture.tenant_id,
            "reply",
            fixture.reply_id,
        )
        .await?
            != 0
        {
            return Err(test_error(
                "D1 explicit clear was replaced by the stale preserved quote set",
            ));
        }

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn soft_deleted_reply_rejects_d1_and_d2_without_mutating_relation_history() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("mention_quote_soft_delete").await? else {
        return Ok(());
    };

    let outcome = async {
        let fixture = create_quote_fixture(&context).await?;
        let security = admin_security(fixture.author_id);
        let reply_service = ReplyService::new(context.db.clone(), event_bus(context.db.clone()));
        reply_service
            .delete(
                fixture.tenant_id,
                fixture.reply_id,
                security.clone(),
            )
            .await?;

        let revision_count_before = relation_revision_count(
            &context.db,
            fixture.tenant_id,
            "reply",
            fixture.reply_id,
        )
        .await?;
        let quote_count_before = latest_quote_count(
            &context.db,
            fixture.tenant_id,
            "reply",
            fixture.reply_id,
        )
        .await?;

        let d1_error = ForumQuoteCommandService::new(context.db.clone())
            .set_reply_quotes(
                fixture.tenant_id,
                fixture.reply_id,
                security.clone(),
                SetForumQuotesInput {
                    locale: LOCALE.to_string(),
                    quotes: Vec::new(),
                },
            )
            .await
            .expect_err("D1 must reject a soft-deleted reply");
        if d1_error.stable_code() != "FORUM_REPLY_DELETED" {
            return Err(test_error(format!(
                "unexpected D1 soft-delete error: {}",
                d1_error.stable_code()
            )));
        }

        let d2_error = reply_service
            .update(
                fixture.tenant_id,
                fixture.reply_id,
                security,
                UpdateReplyInput {
                    locale: LOCALE.to_string(),
                    content: Some("Deleted reply edit".to_string()),
                    content_format: None,
                    content_json: None,
                },
            )
            .await
            .expect_err("D2 must reject a soft-deleted reply");
        if d2_error.stable_code() != "FORUM_REPLY_DELETED" {
            return Err(test_error(format!(
                "unexpected D2 soft-delete error: {}",
                d2_error.stable_code()
            )));
        }

        let revision_count_after = relation_revision_count(
            &context.db,
            fixture.tenant_id,
            "reply",
            fixture.reply_id,
        )
        .await?;
        let quote_count_after = latest_quote_count(
            &context.db,
            fixture.tenant_id,
            "reply",
            fixture.reply_id,
        )
        .await?;
        if revision_count_after != revision_count_before || quote_count_after != quote_count_before {
            return Err(test_error(format!(
                "soft-delete rejection mutated relation history: revisions {revision_count_before}->{revision_count_after}, quotes {quote_count_before}->{quote_count_after}"
            )));
        }
        if quote_count_after != 1 {
            return Err(test_error(
                "soft deletion must preserve the immutable quoted revision history",
            ));
        }

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn mention_owner_event_commits_with_notifications_not_composed() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("mention_notifications_off").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();
        seed_category(&context.db, tenant_id, category_id).await?;
        let security = admin_security(author_id);

        let topic = TopicService::new(context.db.clone(), event_bus(context.db.clone()))
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: LOCALE.to_string(),
                    category_id,
                    title: "Notifications-off mention proof".to_string(),
                    slug: Some("notifications-off-mention-proof".to_string()),
                    body: "Owner topic".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: Vec::new(),
                    channel_slugs: None,
                },
            )
            .await?;
        let reply = ReplyService::new(context.db.clone(), event_bus(context.db.clone()))
            .create_command(
                tenant_id,
                security,
                topic.id,
                CreateReplyCommandInput {
                    locale: LOCALE.to_string(),
                    content: "@moderators please review".to_string(),
                    content_format: "markdown".to_string(),
                    content_json: None,
                    parent_reply_id: None,
                    quotes: Vec::new(),
                },
            )
            .await?;

        let journal_row = context
            .db
            .query_one(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT event_id FROM forum_domain_events \
                     WHERE tenant_id = '{tenant_id}' \
                       AND aggregate_type = 'reply' \
                       AND aggregate_id = '{}' \
                       AND event_type = 'forum.mention.audience_added' \
                     ORDER BY sequence_no DESC LIMIT 1",
                    reply.id
                ),
            ))
            .await?
            .ok_or_else(|| test_error("Forum mention event was not written to the owner journal"))?;
        let event_id: Uuid = journal_row.try_get("", "event_id")?;
        let outbox_event = SysEvents::find_by_id(event_id)
            .filter(outbox_entity::Column::EventType.eq("forum.mention.audience_added"))
            .one(&context.db)
            .await?
            .ok_or_else(|| test_error("Forum mention event was not written to the outbox"))?;
        if outbox_event.schema_version != 1 {
            return Err(test_error(format!(
                "unexpected mention event schema version: {}",
                outbox_event.schema_version
            )));
        }

        let audience_count = scalar_i64(
            &context.db,
            format!(
                "SELECT COUNT(*)::bigint AS value FROM forum_audience_mentions \
                 WHERE tenant_id = '{tenant_id}' \
                   AND source_kind = 'reply' \
                   AND source_id = '{}' \
                   AND audience = 'moderators'",
                reply.id
            ),
        )
        .await?;
        if audience_count != 1 {
            return Err(test_error(format!(
                "expected one persisted moderators audience, got {audience_count}"
            )));
        }

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

async fn create_quote_fixture(context: &PostgresForumTestDb) -> TestResult<QuoteFixture> {
    let tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    seed_category(&context.db, tenant_id, category_id).await?;
    let security = admin_security(author_id);

    let topic = TopicService::new(context.db.clone(), event_bus(context.db.clone()))
        .create(
            tenant_id,
            security.clone(),
            CreateTopicInput {
                locale: LOCALE.to_string(),
                category_id,
                title: "Quoted source".to_string(),
                slug: Some("quoted-source".to_string()),
                body: "Quoted source body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?;
    let quoted_topic_revision_id =
        latest_relation_revision_id(&context.db, tenant_id, "topic", topic.id).await?;

    let reply = ReplyService::new(context.db.clone(), event_bus(context.db.clone()))
        .create_command(
            tenant_id,
            security,
            topic.id,
            CreateReplyCommandInput {
                locale: LOCALE.to_string(),
                content: ORIGINAL_REPLY_BODY.to_string(),
                content_format: "markdown".to_string(),
                content_json: None,
                parent_reply_id: None,
                quotes: vec![ForumQuoteReferenceInput {
                    target_kind: ForumQuoteTargetKindInput::Topic,
                    target_id: topic.id,
                    revision_id: quoted_topic_revision_id,
                }],
            },
        )
        .await?;

    if latest_quote_count(&context.db, tenant_id, "reply", reply.id).await? != 1 {
        return Err(test_error("fixture reply did not persist its initial quote"));
    }

    Ok(QuoteFixture {
        tenant_id,
        reply_id: reply.id,
        author_id,
    })
}

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<()> {
    execute(
        db,
        format!(
            "INSERT INTO forum_categories \
                 (id, tenant_id, position, moderated, topic_count, reply_count) \
             VALUES ('{category_id}', '{tenant_id}', 0, FALSE, 0, 0)"
        ),
    )
    .await
}

fn admin_security(author_id: Uuid) -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(author_id))
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn unique_application_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

async fn set_application_name(db: &DatabaseConnection, name: &str) -> TestResult<()> {
    execute(db, format!("SET application_name TO '{name}'")).await
}

async fn wait_until_lock_wait(db: &DatabaseConnection, application_name: &str) -> TestResult<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT EXISTS (\
                         SELECT 1 FROM pg_stat_activity \
                         WHERE application_name = '{application_name}' \
                           AND wait_event_type = 'Lock'\
                     ) AS blocked"
                ),
            ))
            .await?
            .ok_or_else(|| test_error("pg_stat_activity wait query returned no row"))?;
        let blocked: bool = row.try_get("", "blocked")?;
        if blocked {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(test_error(format!(
                "session {application_name} did not enter a PostgreSQL lock wait"
            )));
        }
        sleep(Duration::from_millis(20)).await;
    }
}

async fn latest_relation_revision_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT revision_id::bigint AS value FROM forum_relation_revisions \
             WHERE tenant_id = '{tenant_id}' \
               AND target_kind = '{target_kind}' \
               AND target_id = '{target_id}' \
               AND locale = '{LOCALE}' \
             ORDER BY revision_id DESC LIMIT 1"
        ),
    )
    .await
}

async fn relation_revision_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*)::bigint AS value FROM forum_relation_revisions \
             WHERE tenant_id = '{tenant_id}' \
               AND target_kind = '{target_kind}' \
               AND target_id = '{target_id}' \
               AND locale = '{LOCALE}'"
        ),
    )
    .await
}

async fn latest_quote_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*)::bigint AS value FROM forum_quotes \
             WHERE tenant_id = '{tenant_id}' \
               AND source_kind = '{target_kind}' \
               AND source_id = '{target_id}' \
               AND source_locale = '{LOCALE}' \
               AND source_revision_id = (\
                   SELECT revision_id FROM forum_relation_revisions \
                   WHERE tenant_id = '{tenant_id}' \
                     AND target_kind = '{target_kind}' \
                     AND target_id = '{target_id}' \
                     AND locale = '{LOCALE}' \
                   ORDER BY revision_id DESC LIMIT 1\
               )"
        ),
    )
    .await
}

async fn scalar_i64(db: &DatabaseConnection, sql: impl Into<String>) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            sql.into(),
        ))
        .await?
        .ok_or_else(|| test_error("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn scalar_string(db: &DatabaseConnection, sql: impl Into<String>) -> TestResult<String> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            sql.into(),
        ))
        .await?
        .ok_or_else(|| test_error("string query returned no row"))?;
    Ok(row.try_get("", "value")?)
}
