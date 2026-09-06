use std::collections::BTreeSet;

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{ForumDomainEventQuery, ForumEventService};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use uuid::Uuid;

use super::{TestResult, test_error};

pub const EXPECTED_FORUM_EVENT_TYPES: &[&str] = &[
    "forum.category.created",
    "forum.category.updated",
    "forum.category.deleted",
    "forum.topic.created",
    "forum.topic.updated",
    "forum.topic.deleted",
    "forum.topic.status_changed",
    "forum.topic.pinned_changed",
    "forum.topic.lock_changed",
    "forum.reply.created",
    "forum.reply.updated",
    "forum.reply.deleted",
    "forum.reply.status_changed",
    "forum.solution.marked",
    "forum.solution.unmarked",
    "forum.topic.vote_changed",
    "forum.reply.vote_changed",
    "forum.subscription.changed",
    "forum.topic.tags_changed",
];

pub async fn exercise_forum_event_contract(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let empty_category_id = Uuid::new_v4();
    let foreign_category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();
    let solution_reply_id = Uuid::new_v4();
    let term_id = Uuid::new_v4();

    let sql_uuid = |id: Uuid| -> String {
        match db.get_database_backend() {
            sea_orm::DatabaseBackend::Sqlite => {
                format!("X'{}'", id.simple().to_string().to_uppercase())
            }
            _ => format!("'{id}'"),
        }
    };

    let cat_id = sql_uuid(category_id);
    let t_id = sql_uuid(tenant_id);
    let foreign_t_id = sql_uuid(foreign_tenant_id);
    let u_id = sql_uuid(user_id);
    let empty_cat_id = sql_uuid(empty_category_id);
    let foreign_cat_id = sql_uuid(foreign_category_id);
    let top_id = sql_uuid(topic_id);
    let rep_id = sql_uuid(reply_id);
    let sol_rep_id = sql_uuid(solution_reply_id);
    let trm_id = sql_uuid(term_id);

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ({cat_id}, {t_id}, 0, FALSE, 0, 0);

UPDATE forum_categories
SET color = 'blue'
WHERE tenant_id = {t_id} AND id = {cat_id};

INSERT INTO forum_topics
    (id, tenant_id, category_id, author_id, status, metadata,
     is_pinned, is_locked, reply_count)
VALUES
    ({top_id}, {t_id}, {cat_id}, {u_id},
     'open', '{{}}', FALSE, FALSE, 0);

INSERT INTO forum_topic_translations
    (id, tenant_id, topic_id, locale, title, slug, body)
VALUES
    ({}, {t_id}, {top_id}, 'en',
     'Event contract', 'event-contract', 'Original body');

UPDATE forum_topics
SET metadata = '{{"contract":true}}'
WHERE tenant_id = {t_id} AND id = {top_id};

UPDATE forum_topic_translations
SET title = 'Updated event contract'
WHERE tenant_id = {t_id}
  AND topic_id = {top_id}
  AND locale = 'en';

UPDATE forum_topics
SET status = 'closed'
WHERE tenant_id = {t_id} AND id = {top_id};

UPDATE forum_topics
SET status = 'open',
     is_pinned = TRUE,
     is_locked = TRUE
WHERE tenant_id = {t_id} AND id = {top_id};

UPDATE forum_topics
SET is_locked = FALSE
WHERE tenant_id = {t_id} AND id = {top_id};

INSERT INTO forum_replies
    (id, tenant_id, topic_id, author_id, status, position)
VALUES
    ({rep_id}, {t_id}, {top_id}, {u_id}, 'approved', 1);

INSERT INTO forum_reply_bodies
    (id, tenant_id, reply_id, locale, body)
VALUES
    ({}, {t_id}, {rep_id}, 'en', 'Original reply');

UPDATE forum_reply_bodies
SET body = 'Updated reply'
WHERE tenant_id = {t_id}
  AND reply_id = {rep_id}
  AND locale = 'en';

UPDATE forum_replies
SET status = 'hidden'
WHERE tenant_id = {t_id} AND id = {rep_id};

INSERT INTO forum_replies
    (id, tenant_id, topic_id, author_id, status, position)
VALUES
    ({sol_rep_id}, {t_id}, {top_id}, {u_id}, 'approved', 2);

INSERT INTO forum_solutions
    (tenant_id, topic_id, reply_id, marked_by_user_id)
VALUES
    ({t_id}, {top_id}, {sol_rep_id}, {u_id});

DELETE FROM forum_solutions
WHERE tenant_id = {t_id} AND topic_id = {top_id};

INSERT INTO taxonomy_terms
    (id, tenant_id, kind, scope_type, scope_value, canonical_key, revision)
VALUES
    ({trm_id}, {t_id}, 'tag', 'module', 'forum', 'event-contract', 1);

INSERT INTO forum_topic_votes
    (topic_id, user_id, tenant_id, value)
VALUES
    ({top_id}, {u_id}, {t_id}, 1);
UPDATE forum_topic_votes
SET value = -1
WHERE topic_id = {top_id} AND user_id = {u_id} AND tenant_id = {t_id};
DELETE FROM forum_topic_votes
WHERE topic_id = {top_id} AND user_id = {u_id} AND tenant_id = {t_id};

INSERT INTO forum_reply_votes
    (reply_id, user_id, tenant_id, value)
VALUES
    ({sol_rep_id}, {u_id}, {t_id}, 1);
UPDATE forum_reply_votes
SET value = -1
WHERE reply_id = {sol_rep_id} AND user_id = {u_id} AND tenant_id = {t_id};
DELETE FROM forum_reply_votes
WHERE reply_id = {sol_rep_id} AND user_id = {u_id} AND tenant_id = {t_id};

INSERT INTO forum_category_subscriptions
    (category_id, user_id, tenant_id, level, notify_mentions, notify_replies, notify_new_topics, digest_mode, revision, updated_at)
VALUES
    ({cat_id}, {u_id}, {t_id}, 'watching', 1, 1, 1, 'disabled', 1, CURRENT_TIMESTAMP);
DELETE FROM forum_category_subscriptions
WHERE category_id = {cat_id} AND user_id = {u_id} AND tenant_id = {t_id};

DELETE FROM forum_topic_subscriptions
WHERE topic_id = {top_id} AND user_id = {u_id} AND tenant_id = {t_id};

INSERT INTO forum_topic_subscriptions
    (topic_id, user_id, tenant_id, level, notify_mentions, notify_replies, digest_mode, revision, updated_at)
VALUES
    ({top_id}, {u_id}, {t_id}, 'watching', 1, 1, 'disabled', 1, CURRENT_TIMESTAMP);
DELETE FROM forum_topic_subscriptions
WHERE topic_id = {top_id} AND user_id = {u_id} AND tenant_id = {t_id};

INSERT INTO forum_topic_tags
    (id, topic_id, term_id, tenant_id)
VALUES
    ({}, {top_id}, {trm_id}, {t_id});
DELETE FROM forum_topic_tags
WHERE topic_id = {top_id} AND term_id = {trm_id} AND tenant_id = {t_id};

DELETE FROM forum_replies
WHERE tenant_id = {t_id} AND id = {rep_id};

DELETE FROM forum_topics
WHERE tenant_id = {t_id} AND id = {top_id};

INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ({empty_cat_id}, {t_id}, 1, FALSE, 0, 0);
DELETE FROM forum_categories
WHERE tenant_id = {t_id} AND id = {empty_cat_id};

INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ({foreign_cat_id}, {foreign_t_id}, 0, FALSE, 0, 0);
"#,
            sql_uuid(Uuid::new_v4()),
            sql_uuid(Uuid::new_v4()),
            sql_uuid(Uuid::new_v4()),
        ),
    )
    .await?;

    let service = ForumEventService::new(db.clone());
    let events = service
        .list(
            tenant_id,
            admin_security(user_id),
            ForumDomainEventQuery {
                limit: Some(100),
                ..Default::default()
            },
        )
        .await?;

    if events.is_empty() {
        return Err(test_error("forum domain event stream is empty"));
    }

    let found_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<BTreeSet<_>>();
    let expected_types = EXPECTED_FORUM_EVENT_TYPES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if found_types != expected_types {
        return Err(test_error(format!(
            "forum event contract mismatch; missing={:?}, unexpected={:?}",
            expected_types.difference(&found_types).collect::<Vec<_>>(),
            found_types.difference(&expected_types).collect::<Vec<_>>(),
        )));
    }

    if events
        .windows(2)
        .any(|window| window[0].sequence_no >= window[1].sequence_no)
    {
        return Err(test_error(
            "forum domain event sequence must be strictly increasing",
        ));
    }
    if events.iter().any(|event| {
        event.tenant_id != tenant_id
            || event.schema_version != 1
            || !matches!(
                event.aggregate_type.as_str(),
                "category" | "topic" | "reply"
            )
    }) {
        return Err(test_error(
            "forum event stream leaked another tenant or invalid contract metadata",
        ));
    }

    let first_page = service
        .list(
            tenant_id,
            admin_security(user_id),
            ForumDomainEventQuery {
                limit: Some(5),
                ..Default::default()
            },
        )
        .await?;
    if first_page.len() != 5 {
        return Err(test_error(format!(
            "expected first event page size 5, got {}",
            first_page.len()
        )));
    }
    let cursor = first_page
        .last()
        .ok_or_else(|| test_error("first event page unexpectedly empty"))?
        .sequence_no;
    let second_page = service
        .list(
            tenant_id,
            admin_security(user_id),
            ForumDomainEventQuery {
                after_sequence: Some(cursor),
                limit: Some(5),
                ..Default::default()
            },
        )
        .await?;
    if second_page.is_empty() || second_page.iter().any(|event| event.sequence_no <= cursor) {
        return Err(test_error("forum event cursor did not advance"));
    }

    let topic_events = service
        .list(
            tenant_id,
            admin_security(user_id),
            ForumDomainEventQuery {
                aggregate_type: Some("topic".to_string()),
                aggregate_id: Some(topic_id),
                limit: Some(100),
                ..Default::default()
            },
        )
        .await?;
    if topic_events.is_empty()
        || topic_events
            .iter()
            .any(|event| event.aggregate_type != "topic" || event.aggregate_id != topic_id)
    {
        return Err(test_error("forum aggregate event filter is not isolated"));
    }

    for index in 0..110 {
        execute(
            db,
            format!(
                "UPDATE forum_categories
                 SET color = 'contract-{index}'
                 WHERE tenant_id = {t_id} AND id = {cat_id}"
            ),
        )
        .await?;
    }
    let capped = service
        .list(
            tenant_id,
            admin_security(user_id),
            ForumDomainEventQuery {
                limit: Some(10_000),
                ..Default::default()
            },
        )
        .await?;
    if capped.len() != 100 {
        return Err(test_error(format!(
            "forum event query must cap limit at 100, got {}",
            capped.len()
        )));
    }

    expect_rejected(
        db,
        "UPDATE forum_domain_events SET event_type = event_type WHERE sequence_no = 1",
        "forum event update",
    )
    .await?;
    expect_rejected(
        db,
        "DELETE FROM forum_domain_events WHERE sequence_no = 1",
        "forum event delete",
    )
    .await?;

    Ok(())
}

async fn execute(db: &DatabaseConnection, sql: impl AsRef<str>) -> TestResult<()> {
    db.execute_unprepared(sql.as_ref()).await?;
    Ok(())
}

async fn expect_rejected(
    db: &DatabaseConnection,
    sql: impl AsRef<str>,
    label: &str,
) -> TestResult<()> {
    if db.execute_unprepared(sql.as_ref()).await.is_ok() {
        return Err(test_error(format!("{label} must be rejected")));
    }
    Ok(())
}

fn admin_security(user_id: Uuid) -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(user_id))
}
