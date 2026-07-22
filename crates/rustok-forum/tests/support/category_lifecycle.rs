use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{CategoryService, CategoryTreeQuery, ForumError};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use uuid::Uuid;

use super::{TestResult, test_error};

pub async fn exercise_category_subtree_lifecycle(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let root_id = seed_category(db, tenant_id, None, 0, "Root", "root").await?;
    let child_id = seed_category(db, tenant_id, Some(root_id), 0, "Child", "child").await?;
    let grandchild_id =
        seed_category(db, tenant_id, Some(child_id), 0, "Grandchild", "grandchild").await?;
    let foreign_root_id =
        seed_category(db, foreign_tenant_id, None, 0, "Foreign", "foreign").await?;

    let existing_topic_id = Uuid::new_v4();
    insert_topic(db, existing_topic_id, tenant_id, child_id).await?;

    let service = CategoryService::new(db.clone());
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let archived = service
        .archive_subtree(tenant_id, child_id, security.clone())
        .await?;
    assert!(archived.archived);
    assert_eq!(
        archived.affected_category_ids,
        vec![child_id, grandchild_id]
    );
    assert_eq!(archived.changed_count, 2);

    let tree = service
        .tree(
            tenant_id,
            security.clone(),
            CategoryTreeQuery {
                locale: Some("en".to_string()),
                ..Default::default()
            },
        )
        .await?;
    let child = &tree.roots[0].children[0];
    assert!(child.is_archived);
    assert!(!child.allows_topics);
    assert!(child.children[0].is_archived);
    assert!(!child.children[0].allows_topics);

    let blocked_topic_id = Uuid::new_v4();
    let blocked = insert_topic(db, blocked_topic_id, tenant_id, grandchild_id).await;
    assert_error_contains(blocked, "does not allow topic creation")?;

    let active_child_id = Uuid::new_v4();
    let active_child = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_categories \
             (id, tenant_id, parent_id, position, moderated, topic_count, reply_count) \
             VALUES ('{active_child_id}', '{tenant_id}', '{child_id}', 1, FALSE, 0, 0);"
        ))
        .await;
    assert_error_contains(active_child.map(|_| ()), "archived parent")?;

    assert_validation_contains(
        service
            .restore_subtree(tenant_id, grandchild_id, security.clone())
            .await,
        "archived ancestor",
    )?;

    match service
        .archive_subtree(tenant_id, foreign_root_id, security.clone())
        .await
    {
        Err(ForumError::CategoryNotFound(id)) if id == foreign_root_id => {}
        Err(error) => {
            return Err(test_error(format!(
                "expected tenant-scoped category not found, got {error}"
            )));
        }
        Ok(_) => return Err(test_error("foreign tenant category was archived")),
    }

    let restored = service
        .restore_subtree(tenant_id, child_id, security.clone())
        .await?;
    assert!(!restored.archived);
    assert_eq!(restored.changed_count, 2);

    let allowed_topic_id = Uuid::new_v4();
    insert_topic(db, allowed_topic_id, tenant_id, grandchild_id).await?;

    let direct_parent_archive = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_category_lifecycle \
             (category_id, tenant_id, archived_at, updated_at) \
             VALUES ('{root_id}', '{tenant_id}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);"
        ))
        .await;
    assert_error_contains(direct_parent_archive.map(|_| ()), "forum category")?;

    service
        .archive_subtree(tenant_id, root_id, security.clone())
        .await?;
    let partial_restore = db
        .execute_unprepared(&format!(
            "DELETE FROM forum_category_lifecycle WHERE category_id = '{grandchild_id}';"
        ))
        .await;
    assert_error_contains(partial_restore.map(|_| ()), "archived parent")?;

    let tenant_mismatch = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_category_lifecycle \
             (category_id, tenant_id, archived_at, updated_at) \
             VALUES ('{root_id}', '{foreign_tenant_id}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);"
        ))
        .await;
    assert_error_contains(tenant_mismatch.map(|_| ()), "lifecycle")?;

    let existing_count = topic_count(db, existing_topic_id).await?;
    assert_eq!(existing_count, 1, "archive mutated an existing topic");

    service
        .restore_subtree(tenant_id, root_id, security)
        .await?;
    Ok(())
}

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
    name: &str,
    slug: &str,
) -> TestResult<Uuid> {
    let category_id = Uuid::new_v4();
    let translation_id = Uuid::new_v4();
    let parent_sql = parent_id
        .map(|value| format!("'{value}'"))
        .unwrap_or_else(|| "NULL".to_string());
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories \
         (id, tenant_id, parent_id, position, moderated, topic_count, reply_count) \
         VALUES ('{category_id}', '{tenant_id}', {parent_sql}, {position}, FALSE, 0, 0); \
         INSERT INTO forum_category_translations \
         (id, category_id, tenant_id, locale, name, slug) \
         VALUES ('{translation_id}', '{category_id}', '{tenant_id}', 'en', '{name}', '{slug}');"
    ))
    .await?;
    Ok(category_id)
}

async fn insert_topic(
    db: &DatabaseConnection,
    topic_id: Uuid,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<()> {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics \
         (id, tenant_id, category_id, status, is_pinned, is_locked, reply_count) \
         VALUES ('{topic_id}', '{tenant_id}', '{category_id}', 'open', FALSE, FALSE, 0);"
    ))
    .await?;
    Ok(())
}

async fn topic_count(db: &DatabaseConnection, topic_id: Uuid) -> TestResult<i64> {
    let backend = db.get_database_backend();
    let row = db
        .query_one(Statement::from_string(
            backend,
            format!("SELECT COUNT(*) AS count FROM forum_topics WHERE id = '{topic_id}'"),
        ))
        .await?
        .ok_or_else(|| test_error("topic count query returned no row"))?;
    Ok(row.try_get("", "count")?)
}

fn assert_error_contains<T, E>(result: Result<T, E>, expected: &str) -> TestResult<()>
where
    E: std::fmt::Display,
{
    match result {
        Err(error) if error.to_string().contains(expected) => Ok(()),
        Err(error) => Err(test_error(format!(
            "expected error containing {expected:?}, got {error}"
        ))),
        Ok(_) => Err(test_error(format!(
            "expected error containing {expected:?}, got success"
        ))),
    }
}

fn assert_validation_contains<T>(result: Result<T, ForumError>, expected: &str) -> TestResult<()> {
    match result {
        Err(ForumError::Validation(message)) if message.contains(expected) => Ok(()),
        Err(error) => Err(test_error(format!(
            "expected validation containing {expected:?}, got {error}"
        ))),
        Ok(_) => Err(test_error(format!(
            "expected validation containing {expected:?}, got success"
        ))),
    }
}
