use std::collections::HashSet;

use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::forum_topic_merge_operation;
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
            let edges = forum_topic_merge_operation::Entity::find()
                .filter(forum_topic_merge_operation::Column::TenantId.eq(tenant_id))
                .filter(
                    forum_topic_merge_operation::Column::SourceTopicId.eq(canonical_topic_id),
                )
                .order_by_asc(forum_topic_merge_operation::Column::OperationId)
                .limit(2)
                .all(&self.db)
                .await?;

            match edges.as_slice() {
                [] => break,
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

        if !topic_exists(&self.db, tenant_id, canonical_topic_id).await? {
            return Err(ForumError::TopicNotFound(requested_topic_id));
        }

        Ok(ForumTopicCanonicalResolution {
            requested_topic_id,
            canonical_topic_id,
            redirected: requested_topic_id != canonical_topic_id,
            hop_count: u32::try_from(merge_operation_ids.len()).map_err(|_| {
                ForumError::TopicCanonicalResolutionConflict(requested_topic_id)
            })?,
            merge_operation_ids,
        })
    }
}

async fn topic_exists(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<bool> {
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT 1 FROM forum_topics WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 FROM forum_topics WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic canonical resolution does not support database backend {backend:?}"
            )));
        }
    };
    Ok(db.query_one(statement).await?.is_some())
}
