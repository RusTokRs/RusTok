mod support;

use std::sync::Arc;

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, ReplyService, UpdateCategoryInput,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use tokio::sync::Barrier;
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute, expect_rejected};
use support::{TestResult, test_error};

#[tokio::test]
async fn forum_02_unknown_topic_status_is_rejected() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("unknown_topic_status").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, false, false).await?;
        expect_rejected(
            &context.db,
            format!(
                "INSERT INTO forum_topics
                    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
                 VALUES
                    ('{}', '{}', '{}', 'definitely_not_a_status', '{{}}', FALSE, FALSE, 0)",
                Uuid::new_v4(),
                seed.tenant_id,
                seed.category_id
            ),
            "unknown topic status",
        )
        .await
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn forum_02_unknown_reply_status_is_rejected() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("unknown_reply_status").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, false, false).await?;
        expect_rejected(
            &context.db,
            format!(
                "INSERT INTO forum_replies
                    (id, tenant_id, topic_id, status, position)
                 VALUES
                    ('{}', '{}', '{}', 'definitely_not_a_status', 1)",
                Uuid::new_v4(),
                seed.tenant_id,
                seed.topic_id
            ),
            "unknown reply status",
        )
        .await
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn forum_03_category_create_rolls_back_when_translation_insert_fails() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_atomic_create").await? else {
        return Ok(());
    };
    let outcome = async {
        execute(
            &context.db,
            r#"
CREATE FUNCTION forum_test_reject_category_translation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forced category translation failure';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_test_reject_category_translation
BEFORE INSERT ON forum_category_translations
FOR EACH ROW EXECUTE FUNCTION forum_test_reject_category_translation();
"#,
        )
        .await?;

        let tenant_id = Uuid::new_v4();
        let service = CategoryService::new(context.db.clone());
        let result = service
            .create(
                tenant_id,
                admin_security(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "Atomic category".to_string(),
                    slug: "atomic-category".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await;
        if result.is_ok() {
            return Err(test_error(
                "forced translation failure must make category creation fail",
            ));
        }

        let count = scalar_i64(
            &context.db,
            format!(
                "SELECT COUNT(*) AS value FROM forum_categories WHERE tenant_id = '{tenant_id}'"
            ),
        )
        .await?;
        if count != 0 {
            return Err(test_error(format!(
                "category insert leaked after translation failure; remaining rows: {count}"
            )));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn forum_03_category_update_rolls_back_when_translation_update_fails() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_atomic_update").await? else {
        return Ok(());
    };
    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let service = CategoryService::new(context.db.clone());
        let category = service
            .create(
                tenant_id,
                admin_security(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "Original category".to_string(),
                    slug: "original-category".to_string(),
                    description: Some("original description".to_string()),
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(3),
                    moderated: false,
                },
            )
            .await?;

        execute(
            &context.db,
            r#"
CREATE FUNCTION forum_test_reject_category_translation_update()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forced category translation update failure';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_test_reject_category_translation_update
BEFORE UPDATE ON forum_category_translations
FOR EACH ROW EXECUTE FUNCTION forum_test_reject_category_translation_update();
"#,
        )
        .await?;

        let result = service
            .update(
                tenant_id,
                category.id,
                admin_security(),
                UpdateCategoryInput {
                    locale: "en".to_string(),
                    name: Some("Changed category".to_string()),
                    slug: Some("changed-category".to_string()),
                    description: Some("changed description".to_string()),
                    icon: None,
                    color: None,
                    position: Some(99),
                    moderated: Some(true),
                },
            )
            .await;
        if result.is_ok() {
            return Err(test_error(
                "forced translation update failure must make category update fail",
            ));
        }

        let position = scalar_i64(
            &context.db,
            format!(
                "SELECT position::bigint AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?;
        let moderated = scalar_i64(
            &context.db,
            format!(
                "SELECT CASE WHEN moderated THEN 1 ELSE 0 END AS value
                 FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?;
        let changed_translation_count = scalar_i64(
            &context.db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_translations
                 WHERE category_id = '{}' AND name = 'Changed category'",
                category.id
            ),
        )
        .await?;

        if position != 3 || moderated != 0 || changed_translation_count != 0 {
            return Err(test_error(format!(
                "category update leaked after translation failure: \
                 position={position}, moderated={moderated}, changed_translation_count={changed_translation_count}"
            )));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn forum_03_category_locale_insert_rolls_back_category_update() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_atomic_locale_insert").await? else {
        return Ok(());
    };
    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let service = CategoryService::new(context.db.clone());
        let category = service
            .create(
                tenant_id,
                admin_security(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "Original category".to_string(),
                    slug: "original-category".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(4),
                    moderated: false,
                },
            )
            .await?;

        execute(
            &context.db,
            r#"
CREATE FUNCTION forum_test_reject_new_category_locale()
RETURNS trigger AS $$
BEGIN
    IF NEW.locale = 'fr' THEN
        RAISE EXCEPTION 'forced new category locale failure';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_test_reject_new_category_locale
BEFORE INSERT ON forum_category_translations
FOR EACH ROW EXECUTE FUNCTION forum_test_reject_new_category_locale();
"#,
        )
        .await?;

        let result = service
            .update(
                tenant_id,
                category.id,
                admin_security(),
                UpdateCategoryInput {
                    locale: "fr".to_string(),
                    name: Some("Catégorie modifiée".to_string()),
                    slug: Some("categorie-modifiee".to_string()),
                    description: None,
                    icon: None,
                    color: None,
                    position: Some(77),
                    moderated: None,
                },
            )
            .await;
        if result.is_ok() {
            return Err(test_error(
                "forced new-locale insert failure must make category update fail",
            ));
        }

        let position = scalar_i64(
            &context.db,
            format!(
                "SELECT position::bigint AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?;
        let french_count = scalar_i64(
            &context.db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_translations
                 WHERE category_id = '{}' AND locale = 'fr'",
                category.id
            ),
        )
        .await?;

        if position != 4 || french_count != 0 {
            return Err(test_error(format!(
                "new locale failure leaked category state: position={position}, french_count={french_count}"
            )));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn forum_03_category_delete_rolls_back_translation_delete() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_atomic_delete").await? else {
        return Ok(());
    };
    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let service = CategoryService::new(context.db.clone());
        let category = service
            .create(
                tenant_id,
                admin_security(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "Protected category".to_string(),
                    slug: "protected-category".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await?;

        execute(
            &context.db,
            r#"
CREATE FUNCTION forum_test_reject_category_delete()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forced category delete failure';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_test_reject_category_delete
BEFORE DELETE ON forum_categories
FOR EACH ROW EXECUTE FUNCTION forum_test_reject_category_delete();
"#,
        )
        .await?;

        let result = service
            .delete(tenant_id, category.id, admin_security())
            .await;
        if result.is_ok() {
            return Err(test_error(
                "forced category delete failure must make category delete fail",
            ));
        }

        let category_count = scalar_i64(
            &context.db,
            format!(
                "SELECT COUNT(*) AS value FROM forum_categories WHERE id = '{}'",
                category.id
            ),
        )
        .await?;
        let translation_count = scalar_i64(
            &context.db,
            format!(
                "SELECT COUNT(*) AS value
                 FROM forum_category_translations
                 WHERE category_id = '{}'",
                category.id
            ),
        )
        .await?;

        if category_count != 1 || translation_count != 1 {
            return Err(test_error(format!(
                "category delete failure leaked partial deletion: \
                 categories={category_count}, translations={translation_count}"
            )));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
#[ignore = "FORUM-04: category hierarchy must reject cycles"]
async fn forum_04_category_cycle_is_rejected() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_cycle").await? else {
        return Ok(());
    };
    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let category_a = Uuid::new_v4();
        let category_b = Uuid::new_v4();
        execute(
            &context.db,
            format!(
                "INSERT INTO forum_categories
                    (id, tenant_id, position, moderated, topic_count, reply_count)
                 VALUES
                    ('{category_a}', '{tenant_id}', 0, FALSE, 0, 0),
                    ('{category_b}', '{tenant_id}', 0, FALSE, 0, 0);
                 UPDATE forum_categories SET parent_id = '{category_b}' WHERE id = '{category_a}';"
            ),
        )
        .await?;
        expect_rejected(
            &context.db,
            format!(
                "UPDATE forum_categories SET parent_id = '{category_a}' WHERE id = '{category_b}'"
            ),
            "category cycle",
        )
        .await
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
#[ignore = "FORUM-05: concurrent approved replies must preserve topic and category counters"]
async fn forum_05_concurrent_replies_preserve_public_counters() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("concurrent_counters").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, false, false).await?;
        install_reply_insert_delay(&context).await?;
        create_concurrent_replies(&context, &seed, 8).await?;

        let topic_count = scalar_i64(
            &context.db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_topics WHERE id = '{}'",
                seed.topic_id
            ),
        )
        .await?;
        let category_count = scalar_i64(
            &context.db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_categories WHERE id = '{}'",
                seed.category_id
            ),
        )
        .await?;

        if topic_count != 8 || category_count != 8 {
            return Err(test_error(format!(
                "lost counter update: topic={topic_count}, category={category_count}, expected=8"
            )));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
#[ignore = "FORUM-06: a locked topic must reject ordinary reply creation"]
async fn forum_06_locked_topic_rejects_reply_creation() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("locked_topic").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, false, true).await?;
        let service = ReplyService::new(context.db.clone(), event_bus(context.db.clone()));
        let result = service
            .create(
                seed.tenant_id,
                customer_security(),
                seed.topic_id,
                reply_input("locked reply"),
            )
            .await;
        if result.is_ok() {
            return Err(test_error("locked topic accepted an ordinary reply"));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
#[ignore = "FORUM-06: pending replies must not mutate public counters"]
async fn forum_06_pending_reply_does_not_change_public_counters() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("pending_counters").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, true, false).await?;
        let service = ReplyService::new(context.db.clone(), event_bus(context.db.clone()));
        let reply = service
            .create(
                seed.tenant_id,
                customer_security(),
                seed.topic_id,
                reply_input("pending reply"),
            )
            .await?;
        if reply.status != "pending" {
            return Err(test_error(format!(
                "moderated category produced unexpected reply status `{}`",
                reply.status
            )));
        }

        let topic_count = scalar_i64(
            &context.db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_topics WHERE id = '{}'",
                seed.topic_id
            ),
        )
        .await?;
        let category_count = scalar_i64(
            &context.db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_categories WHERE id = '{}'",
                seed.category_id
            ),
        )
        .await?;

        if topic_count != 0 || category_count != 0 {
            return Err(test_error(format!(
                "pending reply changed public counters: topic={topic_count}, category={category_count}"
            )));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
#[ignore = "FORUM-06: pending replies must not publish the public topic-replied event"]
async fn forum_06_pending_reply_does_not_emit_public_replied_event() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("pending_event").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, true, false).await?;
        let service = ReplyService::new(context.db.clone(), event_bus(context.db.clone()));
        service
            .create(
                seed.tenant_id,
                customer_security(),
                seed.topic_id,
                reply_input("pending event"),
            )
            .await?;

        let event_count = scalar_i64(
            &context.db,
            "SELECT COUNT(*) AS value
             FROM sys_events
             WHERE event_type = 'forum.topic.replied'",
        )
        .await?;
        if event_count != 0 {
            return Err(test_error(
                "pending reply persisted ForumTopicReplied before approval",
            ));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
#[ignore = "FORUM-07: concurrent reply allocation must produce unique contiguous positions"]
async fn forum_07_concurrent_reply_positions_are_unique_and_contiguous() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("concurrent_positions").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, false, false).await?;
        install_reply_insert_delay(&context).await?;
        create_concurrent_replies(&context, &seed, 8).await?;

        let rows = context
            .db
            .query_all(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT position::bigint AS value
                     FROM forum_replies
                     WHERE tenant_id = '{}' AND topic_id = '{}'
                     ORDER BY position",
                    seed.tenant_id, seed.topic_id
                ),
            ))
            .await?;
        let positions = rows
            .into_iter()
            .map(|row| row.try_get("", "value"))
            .collect::<Result<Vec<i64>, _>>()?;
        let expected = (1_i64..=8).collect::<Vec<_>>();
        if positions != expected {
            return Err(test_error(format!(
                "reply positions are not unique and contiguous: {positions:?}"
            )));
        }
        Ok(())
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[tokio::test]
#[ignore = "FORUM-07: duplicate reply positions must be rejected by the database"]
async fn forum_07_duplicate_reply_position_is_rejected() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("duplicate_position").await? else {
        return Ok(());
    };
    let outcome = async {
        let seed = seed_forum(&context, false, false).await?;
        execute(
            &context.db,
            format!(
                "INSERT INTO forum_replies
                    (id, tenant_id, topic_id, status, position)
                 VALUES
                    ('{}', '{}', '{}', 'approved', 1)",
                Uuid::new_v4(),
                seed.tenant_id,
                seed.topic_id
            ),
        )
        .await?;
        expect_rejected(
            &context.db,
            format!(
                "INSERT INTO forum_replies
                    (id, tenant_id, topic_id, status, position)
                 VALUES
                    ('{}', '{}', '{}', 'approved', 1)",
                Uuid::new_v4(),
                seed.tenant_id,
                seed.topic_id
            ),
            "duplicate reply position",
        )
        .await
    }
    .await;
    context.cleanup().await?;
    outcome
}

#[derive(Clone, Copy)]
struct ForumSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
}

async fn seed_forum(
    context: &PostgresForumTestDb,
    moderated: bool,
    locked: bool,
) -> TestResult<ForumSeed> {
    let seed = ForumSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
    };
    execute(
        &context.db,
        format!(
            "INSERT INTO forum_categories
                (id, tenant_id, position, moderated, topic_count, reply_count)
             VALUES
                ('{}', '{}', 0, {}, 1, 0);
             INSERT INTO forum_topics
                (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
             VALUES
                ('{}', '{}', '{}', 'open', '{{}}', FALSE, {}, 0);",
            seed.category_id,
            seed.tenant_id,
            if moderated { "TRUE" } else { "FALSE" },
            seed.topic_id,
            seed.tenant_id,
            seed.category_id,
            if locked { "TRUE" } else { "FALSE" },
        ),
    )
    .await?;
    Ok(seed)
}

async fn install_reply_insert_delay(context: &PostgresForumTestDb) -> TestResult<()> {
    execute(
        &context.db,
        r#"
CREATE FUNCTION forum_test_delay_reply_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_sleep(0.25);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_test_delay_reply_insert
BEFORE INSERT ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_test_delay_reply_insert();
"#,
    )
    .await
}

async fn create_concurrent_replies(
    context: &PostgresForumTestDb,
    seed: &ForumSeed,
    count: usize,
) -> TestResult<()> {
    let barrier = Arc::new(Barrier::new(count));
    let mut handles = Vec::with_capacity(count);

    for index in 0..count {
        let db = context.peer().await?;
        let barrier = barrier.clone();
        let seed = *seed;
        handles.push(tokio::spawn(async move {
            let service = ReplyService::new(db.clone(), event_bus(db));
            barrier.wait().await;
            service
                .create(
                    seed.tenant_id,
                    customer_security(),
                    seed.topic_id,
                    reply_input(&format!("concurrent reply {index}")),
                )
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }));
    }

    for handle in handles {
        let result = handle
            .await
            .map_err(|error| test_error(format!("reply task failed to join: {error}")))?;
        result.map_err(test_error)?;
    }
    Ok(())
}

fn event_bus(db: sea_orm::DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn admin_security() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn customer_security() -> SecurityContext {
    SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()))
}

fn reply_input(content: &str) -> CreateReplyInput {
    CreateReplyInput {
        locale: "en".to_string(),
        content: content.to_string(),
        content_format: "markdown".to_string(),
        content_json: None,
        parent_reply_id: None,
    }
}

async fn scalar_i64(db: &sea_orm::DatabaseConnection, sql: impl Into<String>) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            sql.into(),
        ))
        .await?
        .ok_or_else(|| test_error("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}
