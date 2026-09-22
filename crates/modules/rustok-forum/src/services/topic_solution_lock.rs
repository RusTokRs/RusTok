use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, Statement};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub(crate) async fn lock_topic_solution_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<()> {
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
                    "Forum topic solution locking does not support database backend {backend:?}"
                )));
            }
        };
        if txn.query_one_raw(statement).await?.is_none() {
            return Err(ForumError::TopicNotFound(*topic_id));
        }
    }

    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            for topic_id in ids {
                txn.execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 31))",
                    vec![format!("{tenant_id}:{topic_id}").into()],
                ))
                .await?;
            }
        }
        DatabaseBackend::Sqlite => {
            for topic_id in ids {
                txn.execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    r#"
                    INSERT INTO forum_topic_solution_locks (tenant_id, topic_id, touched_at)
                    VALUES (?, ?, CURRENT_TIMESTAMP)
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
                "Forum topic solution locking does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}
