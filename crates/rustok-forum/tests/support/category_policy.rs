use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{CategoryService, CreateCategoryInput, ForumError, UpdateCategoryTopicPolicyInput};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use uuid::Uuid;

use super::{TestResult, test_error};

pub async fn exercise_category_topic_policy(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();

    let service = CategoryService::new(db.clone());
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let category = service
        .create(
            tenant_id,
            security.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Test Category".to_string(),
                slug: "test-category".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?;
    let category_id = category.id;

    let default_policy = service
        .topic_policy(tenant_id, category_id, security.clone())
        .await?;
    assert!(default_policy.allows_topics);

    let disabled = service
        .set_topic_policy(
            tenant_id,
            category_id,
            security.clone(),
            UpdateCategoryTopicPolicyInput {
                allows_topics: false,
            },
        )
        .await?;
    assert!(!disabled.allows_topics);

    let blocked_topic_id = Uuid::new_v4();
    let blocked = db
        .execute_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO forum_topics \
             (id, tenant_id, category_id, status, is_pinned, is_locked, reply_count) \
             VALUES (?, ?, ?, 'open', FALSE, FALSE, 0)",
            [blocked_topic_id.into(), tenant_id.into(), category_id.into()],
        ))
        .await;
    let error = blocked.expect_err("disabled category accepted a topic insert");
    let error_message = format!("{error:?}");
    if !error_message.contains("does not allow topic creation") && !error.to_string().contains("does not allow topic creation") {
        return Err(test_error(format!(
            "unexpected category topic policy error: {error_message}"
        )));
    }

    match service
        .set_topic_policy(
            foreign_tenant_id,
            category_id,
            security.clone(),
            UpdateCategoryTopicPolicyInput {
                allows_topics: true,
            },
        )
        .await
    {
        Err(ForumError::CategoryNotFound(id)) if id == category_id => {}
        Err(error) => {
            return Err(test_error(format!(
                "expected tenant-scoped category not found, got {error}"
            )));
        }
        Ok(_) => return Err(test_error("foreign tenant updated category topic policy")),
    }

    service
        .set_topic_policy(
            tenant_id,
            category_id,
            security,
            UpdateCategoryTopicPolicyInput {
                allows_topics: true,
            },
        )
        .await?;
    let allowed_topic_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO forum_topics \
         (id, tenant_id, category_id, status, is_pinned, is_locked, reply_count) \
         VALUES (?, ?, ?, 'open', FALSE, FALSE, 0)",
        [allowed_topic_id.into(), tenant_id.into(), category_id.into()],
    ))
    .await?;

    Ok(())
}
