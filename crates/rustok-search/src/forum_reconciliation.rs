use std::sync::Arc;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use rustok_core::{Error, Result};
use rustok_events::{DomainEvent, EventEnvelope};

use crate::SearchProjectionSource;
use crate::blog_projector::BlogSearchProjector;
use crate::forum_inbox::ForumProjectionInbox;
use crate::forum_projector::ForumSearchProjector;
use crate::projector::SearchProjector;

pub const DEFAULT_FORUM_SWEEP_TENANT_LIMIT: usize = 32;
pub const DEFAULT_FORUM_SWEEP_EVENT_LIMIT: usize = 64;
const MAX_FORUM_SWEEP_TENANT_LIMIT: usize = 256;
const MAX_FORUM_SWEEP_EVENT_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ForumProjectionSweepReport {
    pub due_tenants: usize,
    pub claimed_events: usize,
    pub completed_events: usize,
    pub failed_events: usize,
}

#[derive(Clone)]
pub struct ForumProjectionReconciler {
    db: DatabaseConnection,
    projector: SearchProjector,
    blog_projector: BlogSearchProjector,
    forum_projector: ForumSearchProjector,
    inbox: ForumProjectionInbox,
}

impl ForumProjectionReconciler {
    pub fn new(db: DatabaseConnection, forum_source: Arc<dyn SearchProjectionSource>) -> Self {
        Self {
            projector: SearchProjector::new(db.clone()),
            blog_projector: BlogSearchProjector::new(db.clone()),
            forum_projector: ForumSearchProjector::new(db.clone(), forum_source),
            inbox: ForumProjectionInbox::new(db.clone()),
            db,
        }
    }

    pub fn supports_background_reconciliation(&self) -> bool {
        self.db.get_database_backend() == DbBackend::Postgres
    }

    pub async fn sweep_due(
        &self,
        tenant_limit: usize,
        event_limit: usize,
    ) -> Result<ForumProjectionSweepReport> {
        validate_limit("tenant", tenant_limit, MAX_FORUM_SWEEP_TENANT_LIMIT)?;
        validate_limit("event", event_limit, MAX_FORUM_SWEEP_EVENT_LIMIT)?;
        if !self.supports_background_reconciliation() {
            return Err(Error::External(
                "Forum projection background reconciliation requires PostgreSQL".to_string(),
            ));
        }

        let tenants = self.due_tenants(tenant_limit).await?;
        let mut report = ForumProjectionSweepReport {
            due_tenants: tenants.len(),
            ..ForumProjectionSweepReport::default()
        };
        for tenant_id in tenants {
            let tenant_report = self.reconcile_tenant(tenant_id, event_limit).await?;
            report.claimed_events += tenant_report.claimed_events;
            report.completed_events += tenant_report.completed_events;
            report.failed_events += tenant_report.failed_events;
        }
        Ok(report)
    }

    async fn due_tenants(&self, limit: usize) -> Result<Vec<Uuid>> {
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                WITH oldest AS (
                    SELECT DISTINCT ON (tenant_id)
                        tenant_id,
                        status,
                        next_attempt_at,
                        ingest_sequence
                    FROM search_projection_inbox
                    WHERE source_module = 'forum'
                      AND status IN ('pending', 'retryable_error')
                    ORDER BY tenant_id, ingest_sequence ASC
                )
                SELECT tenant_id
                FROM oldest
                WHERE status = 'pending'
                   OR next_attempt_at IS NULL
                   OR next_attempt_at <= CURRENT_TIMESTAMP
                ORDER BY ingest_sequence ASC
                LIMIT $1
                "#,
                vec![(limit as i64).into()],
            ))
            .await
            .map_err(Error::Database)?;
        rows.into_iter()
            .map(|row| {
                row.try_get::<Uuid>("", "tenant_id")
                    .map_err(Error::Database)
            })
            .collect()
    }

    async fn reconcile_tenant(
        &self,
        tenant_id: Uuid,
        event_limit: usize,
    ) -> Result<ForumProjectionSweepReport> {
        let mut report = ForumProjectionSweepReport::default();
        for _ in 0..event_limit {
            let Some(claim) = self.inbox.claim_next(tenant_id).await? else {
                break;
            };
            report.claimed_events += 1;
            let event_type = claim.envelope().event.event_type().to_string();
            match self.apply_event(claim.envelope()).await {
                Ok(()) => {
                    claim.complete().await?;
                    report.completed_events += 1;
                }
                Err(error) => {
                    tracing::warn!(
                        tenant_id = %tenant_id,
                        event_type = %event_type,
                        error = %error,
                        "Forum projection sweeper scheduled a durable retry"
                    );
                    claim.retry(&error).await?;
                    report.failed_events += 1;
                    break;
                }
            }
        }
        Ok(report)
    }

    async fn apply_event(&self, envelope: &EventEnvelope) -> Result<()> {
        match &envelope.event {
            DomainEvent::ForumTopicCreated { .. }
            | DomainEvent::ForumTopicReplied { .. }
            | DomainEvent::ForumTopicStatusChanged { .. }
            | DomainEvent::ForumTopicPinned { .. }
            | DomainEvent::ForumReplyStatusChanged { .. } => {
                self.forum_projector
                    .rebuild_tenant(envelope.tenant_id)
                    .await
            }
            DomainEvent::TenantModuleToggled {
                module_slug,
                enabled,
                ..
            } if module_slug == "forum" => {
                if *enabled {
                    self.forum_projector
                        .rebuild_tenant(envelope.tenant_id)
                        .await
                } else {
                    self.forum_projector.delete_tenant(envelope.tenant_id).await
                }
            }
            DomainEvent::LocaleEnabled { .. }
            | DomainEvent::LocaleDisabled { .. }
            | DomainEvent::TenantCreated { .. }
            | DomainEvent::TenantUpdated { .. } => self.rebuild_tenant(envelope.tenant_id).await,
            DomainEvent::ReindexRequested {
                target_type,
                target_id,
            } => match (target_type.as_str(), target_id) {
                ("search", _) => self.rebuild_tenant(envelope.tenant_id).await,
                ("forum", _) | ("forum_topic", Some(_)) => {
                    self.forum_projector
                        .rebuild_tenant(envelope.tenant_id)
                        .await
                }
                ("forum_category", Some(category_id)) => {
                    self.forum_projector
                        .refresh_entity(envelope.tenant_id, "forum_category", *category_id)
                        .await
                }
                _ => Err(Error::Validation(format!(
                    "Unsupported Forum projection inbox event `{}`",
                    envelope.event.event_type()
                ))),
            },
            _ => Err(Error::Validation(format!(
                "Unsupported Forum projection inbox event `{}`",
                envelope.event.event_type()
            ))),
        }
    }

    async fn rebuild_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.projector.rebuild_tenant(tenant_id).await?;
        self.blog_projector.rebuild_tenant(tenant_id).await?;
        self.forum_projector.rebuild_tenant(tenant_id).await
    }
}

fn validate_limit(label: &str, value: usize, maximum: usize) -> Result<()> {
    if value == 0 || value > maximum {
        return Err(Error::Validation(format!(
            "Forum projection sweep {label} limit must be between 1 and {maximum}"
        )));
    }
    Ok(())
}
