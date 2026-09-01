use std::collections::HashSet;

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub const MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicCanonicalResolution {
    pub requested_topic_id: Uuid,
    pub canonical_topic_id: Uuid,
    pub redirected: bool,
    pub hop_count: u32,
    pub merge_operation_ids: Vec<Uuid>,
}

#[derive(Clone, Copy)]
struct ForumTopicCanonicalEdge {
    operation_id: Uuid,
    target_topic_id: Uuid,
}

struct ForumTopicCanonicalStep {
    topic_exists: bool,
    edges: Vec<ForumTopicCanonicalEdge>,
}

pub(crate) struct ForumTopicCanonicalResolutionService {
    db: DatabaseConnection,
}

impl ForumTopicCanonicalResolutionService {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) async fn resolve_unchecked(
        &self,
        tenant_id: Uuid,
        requested_topic_id: Uuid,
    ) -> ForumResult<ForumTopicCanonicalResolution> {
        let mut canonical_topic_id = requested_topic_id;
        let mut merge_operation_ids = Vec::new();
        let mut visited = HashSet::from([requested_topic_id]);

        loop {
            let step =
                load_resolution_step(&self.db, tenant_id, canonical_topic_id, requested_topic_id)
                    .await?;

            match step.edges.as_slice() {
                [] => {
                    if !step.topic_exists {
                        return Err(ForumError::TopicNotFound(requested_topic_id));
                    }
                    break;
                }
                [edge] => {
                    if merge_operation_ids.len() >= MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS
                        || !visited.insert(edge.target_topic_id)
                    {
                        return Err(ForumError::TopicCanonicalResolutionConflict(
                            requested_topic_id,
                        ));
                    }
                    merge_operation_ids.push(edge.operation_id);
                    canonical_topic_id = edge.target_topic_id;
                }
                _ => {
                    return Err(ForumError::TopicCanonicalResolutionConflict(
                        requested_topic_id,
                    ));
                }
            }
        }

        Ok(ForumTopicCanonicalResolution {
            requested_topic_id,
            canonical_topic_id,
            redirected: requested_topic_id != canonical_topic_id,
            hop_count: u32::try_from(merge_operation_ids.len())
                .map_err(|_| ForumError::TopicCanonicalResolutionConflict(requested_topic_id))?,
            merge_operation_ids,
        })
    }
}

async fn load_resolution_step(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    requested_topic_id: Uuid,
) -> ForumResult<ForumTopicCanonicalStep> {
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT
                EXISTS (
                    SELECT 1
                    FROM forum_topics topic
                    WHERE topic.tenant_id = $1
                      AND topic.id = $2
                      AND topic.deleted_at IS NULL
                ) AS topic_exists,
                edge.operation_id,
                edge.target_topic_id
            FROM (SELECT 1) AS seed
            LEFT JOIN (
                SELECT operation_id, target_topic_id
                FROM forum_topic_merge_operations
                WHERE tenant_id = $1
                  AND source_topic_id = $2
                ORDER BY operation_id
                LIMIT 2
            ) AS edge ON TRUE
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT
                EXISTS (
                    SELECT 1
                    FROM forum_topics topic
                    WHERE topic.tenant_id = ?
                      AND topic.id = ?
                      AND topic.deleted_at IS NULL
                ) AS topic_exists,
                edge.operation_id,
                edge.target_topic_id
            FROM (SELECT 1) AS seed
            LEFT JOIN (
                SELECT operation_id, target_topic_id
                FROM forum_topic_merge_operations
                WHERE tenant_id = ?
                  AND source_topic_id = ?
                ORDER BY operation_id
                LIMIT 2
            ) AS edge ON 1 = 1
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                tenant_id.into(),
                topic_id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic canonical resolution does not support database backend {backend:?}"
            )));
        }
    };

    let rows = db.query_all_raw(statement).await?;
    let Some(first) = rows.first() else {
        return Err(ForumError::TopicCanonicalResolutionConflict(
            requested_topic_id,
        ));
    };
    let topic_exists = first.try_get("", "topic_exists")?;
    let mut edges = Vec::new();
    for row in rows {
        let operation_id: Option<Uuid> = row.try_get("", "operation_id")?;
        let target_topic_id: Option<Uuid> = row.try_get("", "target_topic_id")?;
        match (operation_id, target_topic_id) {
            (Some(operation_id), Some(target_topic_id)) => edges.push(ForumTopicCanonicalEdge {
                operation_id,
                target_topic_id,
            }),
            (None, None) => {}
            _ => {
                return Err(ForumError::TopicCanonicalResolutionConflict(
                    requested_topic_id,
                ));
            }
        }
    }

    Ok(ForumTopicCanonicalStep {
        topic_exists,
        edges,
    })
}
