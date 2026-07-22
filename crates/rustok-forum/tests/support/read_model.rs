use std::collections::HashSet;

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryCursorQuery, ForumError, ForumReadModelService, MAX_FORUM_READ_LIMIT, ReplyCursorQuery,
    TopicCursorQuery,
};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use uuid::Uuid;

use super::{TestResult, test_error};

pub async fn exercise_bounded_cursor_read_models(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let category_ids = seed_forum(db, tenant_a, 105).await?;
    let foreign_category = seed_category(db, tenant_b, 9000).await?;
    let topic_ids = seed_topics(db, tenant_a, category_ids[0], 105).await?;
    let reply_topic_id = topic_ids[0];
    let reply_ids = seed_replies(db, tenant_a, reply_topic_id, 105).await?;

    let service = ForumReadModelService::new(db.clone());
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let categories_first = service
        .list_categories(
            tenant_a,
            security.clone(),
            CategoryCursorQuery {
                limit: Some(50_000),
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(categories_first.items.len(), MAX_FORUM_READ_LIMIT as usize);
    assert!(categories_first.has_more);
    let categories_second = service
        .list_categories(
            tenant_a,
            security.clone(),
            CategoryCursorQuery {
                cursor: categories_first.next_cursor.clone(),
                limit: Some(100),
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(categories_second.items.len(), 5);
    assert!(!categories_second.has_more);
    assert_disjoint(
        categories_first.items.iter().map(|item| item.id),
        categories_second.items.iter().map(|item| item.id),
        "category cursor",
    )?;
    assert!(
        categories_first
            .items
            .iter()
            .chain(categories_second.items.iter())
            .all(|item| item.id != foreign_category),
        "category cursor leaked another tenant"
    );

    let topics_first = service
        .list_topics(
            tenant_a,
            security.clone(),
            TopicCursorQuery {
                limit: Some(50_000),
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(topics_first.items.len(), MAX_FORUM_READ_LIMIT as usize);
    assert!(topics_first.has_more);
    let topics_second = service
        .list_topics(
            tenant_a,
            security.clone(),
            TopicCursorQuery {
                cursor: topics_first.next_cursor.clone(),
                limit: Some(100),
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(topics_second.items.len(), 5);
    assert!(!topics_second.has_more);
    assert_disjoint(
        topics_first.items.iter().map(|item| item.id),
        topics_second.items.iter().map(|item| item.id),
        "topic cursor",
    )?;

    let replies_first = service
        .list_replies(
            tenant_a,
            security.clone(),
            reply_topic_id,
            ReplyCursorQuery {
                limit: Some(50_000),
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(replies_first.items.len(), MAX_FORUM_READ_LIMIT as usize);
    assert!(replies_first.has_more);
    let replies_second = service
        .list_replies(
            tenant_a,
            security.clone(),
            reply_topic_id,
            ReplyCursorQuery {
                cursor: replies_first.next_cursor.clone(),
                limit: Some(100),
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(replies_second.items.len(), 5);
    assert!(!replies_second.has_more);
    assert_disjoint(
        replies_first.items.iter().map(|item| item.id),
        replies_second.items.iter().map(|item| item.id),
        "reply cursor",
    )?;

    let returned_replies = replies_first
        .items
        .iter()
        .chain(replies_second.items.iter())
        .map(|item| item.id)
        .collect::<HashSet<_>>();
    assert_eq!(
        returned_replies,
        reply_ids.into_iter().collect(),
        "reply cursor skipped or duplicated rows"
    );

    let invalid = service
        .list_topics(
            tenant_a,
            security,
            TopicCursorQuery {
                cursor: Some("not-a-cursor".to_string()),
                ..Default::default()
            },
        )
        .await;
    assert!(
        matches!(invalid, Err(ForumError::Validation(_))),
        "invalid cursor must be rejected"
    );

    Ok(())
}

async fn seed_forum(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    count: usize,
) -> TestResult<Vec<Uuid>> {
    let mut ids = Vec::with_capacity(count);
    for position in 0..count {
        ids.push(seed_category(db, tenant_id, position as i32).await?);
    }
    Ok(ids)
}

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    position: i32,
) -> TestResult<Uuid> {
    let category_id = Uuid::new_v4();
    let translation_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories
            (id, tenant_id, position, moderated, topic_count, reply_count)
         VALUES
            ('{category_id}', '{tenant_id}', {position}, FALSE, 0, 0);
         INSERT INTO forum_category_translations
            (id, category_id, tenant_id, locale, name, slug)
         VALUES
            ('{translation_id}', '{category_id}', '{tenant_id}', 'en',
             'Category {position}', 'category-{category_id}');"
    ))
    .await?;
    Ok(category_id)
}

async fn seed_topics(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    count: usize,
) -> TestResult<Vec<Uuid>> {
    let mut ids = Vec::with_capacity(count);
    for index in 0..count {
        let topic_id = Uuid::new_v4();
        let translation_id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            "INSERT INTO forum_topics
                (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
             VALUES
                ('{topic_id}', '{tenant_id}', '{category_id}', 'open', '{{}}', FALSE, FALSE, 0);
             INSERT INTO forum_topic_translations
                (id, topic_id, tenant_id, locale, title, body, body_format)
             VALUES
                ('{translation_id}', '{topic_id}', '{tenant_id}', 'en',
                 'Topic {index}', 'Body {index}', 'markdown');"
        ))
        .await?;
        ids.push(topic_id);
    }
    Ok(ids)
}

async fn seed_replies(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    count: usize,
) -> TestResult<Vec<Uuid>> {
    let mut ids = Vec::with_capacity(count);
    for index in 0..count {
        let reply_id = Uuid::new_v4();
        let body_id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            "INSERT INTO forum_replies
                (id, tenant_id, topic_id, status, position)
             VALUES
                ('{reply_id}', '{tenant_id}', '{topic_id}', 'approved', {});
             INSERT INTO forum_reply_bodies
                (id, reply_id, tenant_id, locale, body, body_format)
             VALUES
                ('{body_id}', '{reply_id}', '{tenant_id}', 'en',
                 'Reply {index}', 'markdown');",
            index + 1
        ))
        .await?;
        ids.push(reply_id);
    }
    Ok(ids)
}

fn assert_disjoint(
    left: impl Iterator<Item = Uuid>,
    right: impl Iterator<Item = Uuid>,
    label: &str,
) -> TestResult<()> {
    let left = left.collect::<HashSet<_>>();
    let right = right.collect::<HashSet<_>>();
    if !left.is_disjoint(&right) {
        return Err(test_error(format!("{label} returned duplicate rows")));
    }
    Ok(())
}
