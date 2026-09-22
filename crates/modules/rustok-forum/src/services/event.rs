use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use uuid::Uuid;

use crate::dto::{
    ForumDomainEventQuery, ForumDomainEventResponse, ForumProjectionOwnerRevisionImpact,
    ForumProjectionOwnerRevisionResponse, ForumProjectionOwnerTenantHeadResponse,
};
use crate::entities::forum_domain_event;
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

const DEFAULT_EVENT_LIMIT: u64 = 50;
const MAX_EVENT_LIMIT: u64 = 100;
pub const MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE: usize = 100;
pub const MAX_FORUM_PROJECTION_OWNER_TENANT_PAGE: usize = 256;
const FORUM_PROJECTION_INVALIDATION_EVENT_TYPE: &str = "index.reindex_requested";

#[derive(Clone)]
pub struct ForumEventService {
    db: DatabaseConnection,
}

impl ForumEventService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: ForumDomainEventQuery,
    ) -> ForumResult<Vec<ForumDomainEventResponse>> {
        enforce_scope(&security, Resource::ForumTopics, Action::List)?;

        let after_sequence = query.after_sequence.unwrap_or(0);
        if after_sequence < 0 {
            return Err(ForumError::Validation(
                "after_sequence must not be negative".to_string(),
            ));
        }

        let limit = query
            .limit
            .unwrap_or(DEFAULT_EVENT_LIMIT)
            .clamp(1, MAX_EVENT_LIMIT);

        let mut select = forum_domain_event::Entity::find()
            .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
            .filter(forum_domain_event::Column::SequenceNo.gt(after_sequence));

        if let Some(aggregate_type) = normalize_filter(query.aggregate_type, "aggregate_type")? {
            select = select.filter(forum_domain_event::Column::AggregateType.eq(aggregate_type));
        }
        if let Some(aggregate_id) = query.aggregate_id {
            select = select.filter(forum_domain_event::Column::AggregateId.eq(aggregate_id));
        }
        if let Some(event_type) = normalize_filter(query.event_type, "event_type")? {
            select = select.filter(forum_domain_event::Column::EventType.eq(event_type));
        }

        let events = select
            .order_by_asc(forum_domain_event::Column::SequenceNo)
            .limit(limit)
            .all(&self.db)
            .await?;

        Ok(events
            .into_iter()
            .map(|event| ForumDomainEventResponse {
                sequence_no: event.sequence_no,
                event_id: event.event_id,
                tenant_id: event.tenant_id,
                aggregate_type: event.aggregate_type,
                aggregate_id: event.aggregate_id,
                event_type: event.event_type,
                schema_version: event.schema_version,
                actor_id: event.actor_id,
                payload: event.payload,
                created_at: event.created_at.to_rfc3339(),
            })
            .collect())
    }

    /// Reads the append-only Forum projection revision ledger through a bounded
    /// owner API. The boundary exposes only causal revision and durable envelope
    /// identity; ledger targets, timestamps, actors and outbox payloads remain
    /// Forum-private.
    pub async fn list_projection_owner_revisions(
        &self,
        tenant_id: Uuid,
        after_owner_revision: i64,
        limit: usize,
    ) -> ForumResult<Vec<ForumProjectionOwnerRevisionResponse>> {
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "projection owner revision tenant must not be nil".to_string(),
            ));
        }
        if after_owner_revision < 0 {
            return Err(ForumError::Validation(
                "after_owner_revision must not be negative".to_string(),
            ));
        }
        if !(1..=MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE).contains(&limit) {
            return Err(ForumError::Validation(format!(
                "projection owner revision limit must be between 1 and {MAX_FORUM_PROJECTION_OWNER_REVISION_PAGE}"
            )));
        }
        self.ensure_projection_revision_source_available()?;

        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT revision, event_id
                FROM forum_projection_revision_ledger
                WHERE tenant_id = $1
                  AND revision > $2
                ORDER BY revision ASC
                LIMIT $3
                "#,
                vec![
                    tenant_id.into(),
                    after_owner_revision.into(),
                    (limit as i64).into(),
                ],
            ))
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok(ForumProjectionOwnerRevisionResponse {
                    owner_revision: row.try_get("", "revision")?,
                    event_id: row.try_get("", "event_id")?,
                    event_type: FORUM_PROJECTION_INVALIDATION_EVENT_TYPE.to_string(),
                    impact: ForumProjectionOwnerRevisionImpact::FullRebuild,
                })
            })
            .collect()
    }

    /// Pages tenant heads from the same owner ledger so Search can discover a
    /// tenant whose first durable invalidation delivery was lost. The stable UUID
    /// cursor exposes no ledger target or event payload.
    pub async fn list_projection_owner_revision_tenants(
        &self,
        after_tenant_id: Option<Uuid>,
        limit: usize,
    ) -> ForumResult<Vec<ForumProjectionOwnerTenantHeadResponse>> {
        if after_tenant_id.is_some_and(|tenant_id| tenant_id.is_nil()) {
            return Err(ForumError::Validation(
                "projection owner tenant cursor must not be nil".to_string(),
            ));
        }
        if !(1..=MAX_FORUM_PROJECTION_OWNER_TENANT_PAGE).contains(&limit) {
            return Err(ForumError::Validation(format!(
                "projection owner tenant limit must be between 1 and {MAX_FORUM_PROJECTION_OWNER_TENANT_PAGE}"
            )));
        }
        self.ensure_projection_revision_source_available()?;

        let (sql, values) = match after_tenant_id {
            Some(after_tenant_id) => (
                r#"
                SELECT tenant_id, MAX(revision) AS latest_owner_revision
                FROM forum_projection_revision_ledger
                WHERE tenant_id > $1
                GROUP BY tenant_id
                ORDER BY tenant_id ASC
                LIMIT $2
                "#,
                vec![after_tenant_id.into(), (limit as i64).into()],
            ),
            None => (
                r#"
                SELECT tenant_id, MAX(revision) AS latest_owner_revision
                FROM forum_projection_revision_ledger
                GROUP BY tenant_id
                ORDER BY tenant_id ASC
                LIMIT $1
                "#,
                vec![(limit as i64).into()],
            ),
        };
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok(ForumProjectionOwnerTenantHeadResponse {
                    tenant_id: row.try_get("", "tenant_id")?,
                    latest_owner_revision: row.try_get("", "latest_owner_revision")?,
                })
            })
            .collect()
    }

    fn ensure_projection_revision_source_available(&self) -> ForumResult<()> {
        if self.db.get_database_backend() == DbBackend::Postgres {
            Ok(())
        } else {
            Err(ForumError::capability_unavailable(
                "forum_projection_revision_source",
                "FORUM_PROJECTION_REVISION_SOURCE_UNAVAILABLE",
            ))
        }
    }
}

fn normalize_filter(value: Option<String>, field: &str) -> ForumResult<Option<String>> {
    value
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return Err(ForumError::Validation(format!("{field} must not be empty")));
            }
            if normalized.len() > 96 {
                return Err(ForumError::Validation(format!(
                    "{field} must not exceed 96 characters"
                )));
            }
            Ok(normalized)
        })
        .transpose()
}
