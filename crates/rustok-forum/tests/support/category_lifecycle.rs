use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{CategoryService, CategoryTreeQuery, ForumError, TopicStatus};
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
        .execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO forum_categories \
             (id, tenant_id, parent_id, position, moderated, topic_count, reply_count) \
             VALUES (?, ?, ?, 1, FALSE, 0, 0)",
            [active_child_id.into(), tenant_id.into(), child_id.into()],
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
        .execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO forum_category_lifecycle \
             (category_id, tenant_id, archived_at, updated_at) \
             VALUES (?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [root_id.into(), tenant_id.into()],
        ))
        .await;
    assert_error_contains(direct_parent_archive.map(|_| ()), "forum category")?;

    service
        .archive_subtree(tenant_id, root_id, security.clone())
        .await?;
    let partial_restore = db
        .execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "DELETE FROM forum_category_lifecycle WHERE category_id = ?",
            [grandchild_id.into()],
        ))
        .await;
    assert_error_contains(partial_restore.map(|_| ()), "archived parent")?;

    let tenant_mismatch = db
        .execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO forum_category_lifecycle \
             (category_id, tenant_id, archived_at, updated_at) \
             VALUES (?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [root_id.into(), foreign_tenant_id.into()],
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
    use rustok_forum::CreateCategoryInput;
    let service = CategoryService::new(db.clone());
    let security = SecurityContext::system();
    let category = service
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: name.to_string(),
                slug: slug.to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id,
                position: Some(position),
                moderated: false,
            },
        )
        .await?;
    Ok(category.id)
}

async fn insert_topic(
    db: &DatabaseConnection,
    topic_id: Uuid,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<()> {
    use rustok_forum::entities::forum_topic;
    use sea_orm::{ActiveModelTrait, Set};

    let now = chrono::Utc::now();
    let model = forum_topic::ActiveModel {
        id: Set(topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(category_id),
        status: Set(TopicStatus::Open),
        is_pinned: Set(false),
        is_locked: Set(false),
        reply_count: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    };
    model.insert(db).await?;
    Ok(())
}

async fn topic_count(db: &DatabaseConnection, topic_id: Uuid) -> TestResult<i64> {
    use rustok_forum::entities::forum_topic;
    use sea_orm::{EntityTrait, PaginatorTrait};

    let count = forum_topic::Entity::find_by_id(topic_id)
        .count(db)
        .await?;
    Ok(count as i64)
}

fn assert_error_contains<T, E>(result: Result<T, E>, expected: &str) -> TestResult<()>
where
    E: std::fmt::Debug + std::fmt::Display,
{
    match result {
        Err(error) => {
            let debug_repr = format!("{error:?}");
            let display_repr = error.to_string();
            if debug_repr.contains(expected) || display_repr.contains(expected) {
                Ok(())
            } else {
                Err(test_error(format!(
                    "expected error containing {expected:?}, got {debug_repr}"
                )))
            }
        }
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
