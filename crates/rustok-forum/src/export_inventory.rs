use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use thiserror::Error;
use uuid::Uuid;

use super::{
    ForumExportReadTargetKind, ForumExportTargetPlanRequest,
    MAX_FORUM_EXPORT_PLAN_SOURCE_IDS_PER_FRAGMENT,
};

pub const MAX_FORUM_EXPORT_SOURCE_INVENTORY_LIMIT: u64 =
    MAX_FORUM_EXPORT_PLAN_SOURCE_IDS_PER_FRAGMENT as u64;

#[derive(Clone, Debug)]
pub struct ForumExportSourceInventoryRequest {
    pub tenant_id: Uuid,
    pub kind: ForumExportReadTargetKind,
    pub after_id: Option<Uuid>,
    pub limit: u64,
}

#[derive(Clone, Debug)]
pub struct ForumExportSourceInventoryPage {
    pub tenant_id: Uuid,
    pub kind: ForumExportReadTargetKind,
    pub ids: Vec<Uuid>,
    pub cursor: Option<Uuid>,
    pub has_more: bool,
}

impl ForumExportSourceInventoryPage {
    pub fn target_plan_request(&self) -> Option<ForumExportTargetPlanRequest> {
        if self.ids.is_empty() {
            return None;
        }

        let mut request = ForumExportTargetPlanRequest {
            tenant_id: self.tenant_id,
            category_ids: Vec::new(),
            topic_ids: Vec::new(),
            reply_ids: Vec::new(),
        };
        match self.kind {
            ForumExportReadTargetKind::Category => request.category_ids = self.ids.clone(),
            ForumExportReadTargetKind::Topic => request.topic_ids = self.ids.clone(),
            ForumExportReadTargetKind::Reply => request.reply_ids = self.ids.clone(),
        }
        Some(request)
    }
}

#[derive(Debug, Error)]
pub enum ForumExportSourceInventoryError {
    #[error("Forum export source inventory requires an authenticated operator context")]
    OperatorContextRequired,
    #[error("Forum export source inventory requires all-scope {resource}:manage")]
    AllManagePermissionRequired { resource: &'static str },
    #[error("Forum export source inventory requires a non-nil tenant id")]
    NilTenantId,
    #[error("Forum export source inventory cursor id must be non-nil")]
    NilCursorId,
    #[error("Forum export source inventory limit must be between 1 and {max}: {actual}")]
    InvalidLimit { max: u64, actual: u64 },
    #[error("Forum export source inventory does not support database backend {backend}")]
    UnsupportedBackend { backend: String },
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

#[derive(Clone)]
pub struct ForumExportSourceInventoryService {
    db: DatabaseConnection,
}

impl ForumExportSourceInventoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list_page(
        &self,
        security: &SecurityContext,
        request: &ForumExportSourceInventoryRequest,
    ) -> Result<ForumExportSourceInventoryPage, ForumExportSourceInventoryError> {
        validate_inventory_request(security, request)?;

        let fetch_limit = request.limit.saturating_add(1);
        let rows = self
            .db
            .query_all(inventory_statement(
                self.db.get_database_backend(),
                request.kind,
                request.tenant_id,
                request.after_id,
                fetch_limit,
            )?)
            .await?;
        let has_more = rows.len() > request.limit as usize;
        let mut ids = Vec::with_capacity(rows.len().min(request.limit as usize));
        for row in rows.into_iter().take(request.limit as usize) {
            ids.push(row.try_get("", "id")?);
        }
        let cursor = ids.last().copied().or(request.after_id);

        Ok(ForumExportSourceInventoryPage {
            tenant_id: request.tenant_id,
            kind: request.kind,
            ids,
            cursor,
            has_more,
        })
    }
}

fn validate_inventory_request(
    security: &SecurityContext,
    request: &ForumExportSourceInventoryRequest,
) -> Result<(), ForumExportSourceInventoryError> {
    if security.is_public_read() {
        return Err(ForumExportSourceInventoryError::OperatorContextRequired);
    }
    if request.tenant_id.is_nil() {
        return Err(ForumExportSourceInventoryError::NilTenantId);
    }
    if request.after_id.is_some_and(|id| id.is_nil()) {
        return Err(ForumExportSourceInventoryError::NilCursorId);
    }
    if request.limit == 0 || request.limit > MAX_FORUM_EXPORT_SOURCE_INVENTORY_LIMIT {
        return Err(ForumExportSourceInventoryError::InvalidLimit {
            max: MAX_FORUM_EXPORT_SOURCE_INVENTORY_LIMIT,
            actual: request.limit,
        });
    }

    let (resource, label) = match request.kind {
        ForumExportReadTargetKind::Category => (Resource::ForumCategories, "forum_categories"),
        ForumExportReadTargetKind::Topic => (Resource::ForumTopics, "forum_topics"),
        ForumExportReadTargetKind::Reply => (Resource::ForumReplies, "forum_replies"),
    };
    if !matches!(security.get_scope(resource, Action::Manage), PermissionScope::All) {
        return Err(ForumExportSourceInventoryError::AllManagePermissionRequired {
            resource: label,
        });
    }
    Ok(())
}

fn inventory_statement(
    backend: DatabaseBackend,
    kind: ForumExportReadTargetKind,
    tenant_id: Uuid,
    after_id: Option<Uuid>,
    limit: u64,
) -> Result<Statement, ForumExportSourceInventoryError> {
    let (initial_sqlite, after_sqlite, initial_postgres, after_postgres) = match kind {
        ForumExportReadTargetKind::Category => (
            CATEGORY_INVENTORY_SQLITE,
            CATEGORY_INVENTORY_AFTER_SQLITE,
            CATEGORY_INVENTORY_POSTGRES,
            CATEGORY_INVENTORY_AFTER_POSTGRES,
        ),
        ForumExportReadTargetKind::Topic => (
            TOPIC_INVENTORY_SQLITE,
            TOPIC_INVENTORY_AFTER_SQLITE,
            TOPIC_INVENTORY_POSTGRES,
            TOPIC_INVENTORY_AFTER_POSTGRES,
        ),
        ForumExportReadTargetKind::Reply => (
            REPLY_INVENTORY_SQLITE,
            REPLY_INVENTORY_AFTER_SQLITE,
            REPLY_INVENTORY_POSTGRES,
            REPLY_INVENTORY_AFTER_POSTGRES,
        ),
    };

    match (backend, after_id) {
        (DatabaseBackend::Sqlite, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            initial_sqlite,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Sqlite, Some(after_id)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            after_sqlite,
            vec![tenant_id.into(), after_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, None) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            initial_postgres,
            vec![tenant_id.into(), (limit as i64).into()],
        )),
        (DatabaseBackend::Postgres, Some(after_id)) => Ok(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            after_postgres,
            vec![tenant_id.into(), after_id.into(), (limit as i64).into()],
        )),
        (backend, _) => Err(ForumExportSourceInventoryError::UnsupportedBackend {
            backend: format!("{backend:?}"),
        }),
    }
}

const CATEGORY_INVENTORY_SQLITE: &str = r#"
SELECT c.id AS id
FROM forum_categories c
WHERE c.tenant_id = ?1
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = c.tenant_id
        AND lifecycle.category_id = c.id
  )
ORDER BY c.id
LIMIT ?2
"#;

const CATEGORY_INVENTORY_AFTER_SQLITE: &str = r#"
SELECT c.id AS id
FROM forum_categories c
WHERE c.tenant_id = ?1
  AND c.id > ?2
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = c.tenant_id
        AND lifecycle.category_id = c.id
  )
ORDER BY c.id
LIMIT ?3
"#;

const CATEGORY_INVENTORY_POSTGRES: &str = r#"
SELECT c.id AS id
FROM forum_categories c
WHERE c.tenant_id = $1
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = c.tenant_id
        AND lifecycle.category_id = c.id
  )
ORDER BY c.id
LIMIT $2
"#;

const CATEGORY_INVENTORY_AFTER_POSTGRES: &str = r#"
SELECT c.id AS id
FROM forum_categories c
WHERE c.tenant_id = $1
  AND c.id > $2
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = c.tenant_id
        AND lifecycle.category_id = c.id
  )
ORDER BY c.id
LIMIT $3
"#;

const TOPIC_INVENTORY_SQLITE: &str = r#"
SELECT topic.id AS id
FROM forum_topics topic
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE topic.tenant_id = ?1
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY topic.id
LIMIT ?2
"#;

const TOPIC_INVENTORY_AFTER_SQLITE: &str = r#"
SELECT topic.id AS id
FROM forum_topics topic
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE topic.tenant_id = ?1
  AND topic.id > ?2
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY topic.id
LIMIT ?3
"#;

const TOPIC_INVENTORY_POSTGRES: &str = r#"
SELECT topic.id AS id
FROM forum_topics topic
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE topic.tenant_id = $1
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY topic.id
LIMIT $2
"#;

const TOPIC_INVENTORY_AFTER_POSTGRES: &str = r#"
SELECT topic.id AS id
FROM forum_topics topic
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE topic.tenant_id = $1
  AND topic.id > $2
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY topic.id
LIMIT $3
"#;

const REPLY_INVENTORY_SQLITE: &str = r#"
SELECT reply.id AS id
FROM forum_replies reply
JOIN forum_topics topic
  ON topic.tenant_id = reply.tenant_id
 AND topic.id = reply.topic_id
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE reply.tenant_id = ?1
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY reply.id
LIMIT ?2
"#;

const REPLY_INVENTORY_AFTER_SQLITE: &str = r#"
SELECT reply.id AS id
FROM forum_replies reply
JOIN forum_topics topic
  ON topic.tenant_id = reply.tenant_id
 AND topic.id = reply.topic_id
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE reply.tenant_id = ?1
  AND reply.id > ?2
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY reply.id
LIMIT ?3
"#;

const REPLY_INVENTORY_POSTGRES: &str = r#"
SELECT reply.id AS id
FROM forum_replies reply
JOIN forum_topics topic
  ON topic.tenant_id = reply.tenant_id
 AND topic.id = reply.topic_id
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE reply.tenant_id = $1
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY reply.id
LIMIT $2
"#;

const REPLY_INVENTORY_AFTER_POSTGRES: &str = r#"
SELECT reply.id AS id
FROM forum_replies reply
JOIN forum_topics topic
  ON topic.tenant_id = reply.tenant_id
 AND topic.id = reply.topic_id
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
WHERE reply.tenant_id = $1
  AND reply.id > $2
  AND topic.deleted_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM forum_category_lifecycle lifecycle
      WHERE lifecycle.tenant_id = category.tenant_id
        AND lifecycle.category_id = category.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM forum_topic_merge_operations merge_operation
      WHERE merge_operation.tenant_id = topic.tenant_id
        AND merge_operation.source_topic_id = topic.id
  )
ORDER BY reply.id
LIMIT $3
"#;
