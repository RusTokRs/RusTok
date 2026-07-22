use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{CategoryService, ForumError, UpdateCategoryTopicPolicyInput};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use uuid::Uuid;

use super::{TestResult, test_error};

pub async fn exercise_category_topic_policy(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories \
         (id, tenant_id, parent_id, position, moderated, topic_count, reply_count) \
         VALUES ('{category_id}', '{tenant_id}', NULL, 0, FALSE, 0, 0);"
    ))
    .await?;

    let service = CategoryService::new(db.clone());
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
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
        .execute_unprepared(&format!(
            "INSERT INTO forum_topics \
             (id, tenant_id, category_id, status, is_pinned, is_locked, reply_count) \
             VALUES ('{blocked_topic_id}', '{tenant_id}', '{category_id}', 'open', FALSE, FALSE, 0);"
        ))
        .await;
    let error = blocked.expect_err("disabled category accepted a topic insert");
    if !error.to_string().contains("does not allow topic creation") {
        return Err(test_error(format!(
            "unexpected category topic policy error: {error}"
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
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics \
         (id, tenant_id, category_id, status, is_pinned, is_locked, reply_count) \
         VALUES ('{allowed_topic_id}', '{tenant_id}', '{category_id}', 'open', FALSE, FALSE, 0);"
    ))
    .await?;

    Ok(())
}
