use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseTransaction, EntityTrait, QueryFilter,
    Statement,
};
use uuid::Uuid;

use crate::entities::forum_topic;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

pub(crate) async fn lock_active_topic_read_state_write_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<forum_topic::Model> {
    let mut topics = lock_active_topic_read_state_writes_in_tx(txn, tenant_id, &[topic_id]).await?;
    topics.pop().ok_or(ForumError::TopicNotFound(topic_id))
}

pub(crate) async fn lock_active_topic_read_state_writes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<Vec<forum_topic::Model>> {
    let topics = lock_topic_rows_for_read_state_in_tx(txn, tenant_id, topic_ids).await?;
    if topics
        .iter()
        .any(|topic| topic.status == TopicStatus::Archived)
    {
        return Err(ForumError::Validation(
            "Forum topic read state cannot be changed after topic archival".to_string(),
        ));
    }
    Ok(topics)
}

pub(crate) async fn lock_topic_rows_for_read_state_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<Vec<forum_topic::Model>> {
    let mut ids = topic_ids.to_vec();
    ids.sort();
    ids.dedup();

    for topic_id in &ids {
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT id FROM forum_topics WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR SHARE",
                vec![tenant_id.into(), (*topic_id).into()],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT id FROM forum_topics WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
                vec![tenant_id.into(), (*topic_id).into()],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum topic read-state locking does not support database backend {backend:?}"
                )));
            }
        };
        if txn.query_one_raw(statement).await?.is_none() {
            return Err(ForumError::TopicNotFound(*topic_id));
        }
    }

    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let topics = forum_topic::Entity::find()
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .filter(forum_topic::Column::Id.is_in(ids.clone()))
        .all(txn)
        .await?;
    if topics.len() != ids.len() {
        return Err(ForumError::Validation(
            "Forum topic read-state lock lost a topic row".to_string(),
        ));
    }

    let mut by_id = topics
        .into_iter()
        .map(|topic| (topic.id, topic))
        .collect::<std::collections::HashMap<_, _>>();
    ids.into_iter()
        .map(|topic_id| {
            by_id
                .remove(&topic_id)
                .ok_or(ForumError::TopicNotFound(topic_id))
        })
        .collect()
}

pub(crate) async fn lock_topic_read_state_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<()> {
    let mut ids = topic_ids.to_vec();
    ids.sort();
    ids.dedup();

    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            for topic_id in ids {
                txn.execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 24))",
                    vec![format!("forum-topic-read-state:{tenant_id}:{topic_id}").into()],
                ))
                .await?;
            }
        }
        DatabaseBackend::Sqlite => {
            for topic_id in ids {
                txn.execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    r#"
                    INSERT INTO forum_topic_read_state_locks (
                        tenant_id, topic_id, touched_at
                    ) VALUES (?, ?, CURRENT_TIMESTAMP)
                    ON CONFLICT(tenant_id, topic_id)
                    DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                    "#,
                    vec![tenant_id.into(), topic_id.into()],
                ))
                .await?;
            }
        }
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic read-state locking does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}
