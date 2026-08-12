use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::PortError;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use rustok_core::{Error, Result};
use rustok_events::{DomainEvent, EventEnvelope};

use crate::SearchProjectionSource;
use crate::blog_projector::BlogSearchProjector;
use crate::forum_inbox::ForumProjectionInbox;
use crate::forum_owner_checkpoint::{
    ForumOwnerCheckpointReconciler, ForumProjectionOwnerTenantHead,
    ForumProjectionOwnerTenantPageRequest,
};
use crate::forum_projector::ForumSearchProjector;
use crate::projector::SearchProjector;

pub const DEFAULT_FORUM_SWEEP_TENANT_LIMIT: usize = 32;
pub const DEFAULT_FORUM_SWEEP_EVENT_LIMIT: usize = 64;
pub const DEFAULT_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 64;
pub const MAX_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 100;
const MAX_FORUM_SWEEP_TENANT_LIMIT: usize = 256;
const MAX_FORUM_SWEEP_EVENT_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumProjectionOwnerRevisionImpact {
    FullRebuild,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumProjectionOwnerRevisionRecord {
    pub owner_revision: i64,
    pub event_id: Uuid,
    pub event_type: String,
    pub impact: ForumProjectionOwnerRevisionImpact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForumProjectionOwnerRevisionRequest {
    pub tenant_id: Uuid,
    pub after_owner_revision: i64,
    pub limit: usize,
}

#[async_trait]
pub trait ForumProjectionOwnerRevisionSourcePort: Send + Sync {
    async fn list_owner_revisions(
        &self,
        request: ForumProjectionOwnerRevisionRequest,
    ) -> std::result::Result<Vec<ForumProjectionOwnerRevisionRecord>, PortError>;

    async fn list_owner_revision_tenants(
        &self,
        _request: ForumProjectionOwnerTenantPageRequest,
    ) -> std::result::Result<Vec<ForumProjectionOwnerTenantHead>, PortError> {
        Err(PortError::unavailable(
            "forum.search_projection_owner_revision.tenant_source_unavailable",
            "Forum projection owner tenant source is temporarily unavailable",
        ))
    }
}

pub type SharedForumProjectionOwnerRevisionSourcePort =
    Arc<dyn ForumProjectionOwnerRevisionSourcePort>;

/// Resolves a bounded page from the Forum-owned projection revision ledger and
/// verifies the neutral contract before reconciliation can consume it. Owner
/// revisions are an independent causal clock and are never compared numerically
/// with Search-owned inbox ingest sequences.
pub async fn resolve_forum_projection_owner_revisions(
    port: Option<SharedForumProjectionOwnerRevisionSourcePort>,
    request: ForumProjectionOwnerRevisionRequest,
) -> std::result::Result<Vec<ForumProjectionOwnerRevisionRecord>, PortError> {
    validate_owner_revision_request(request)?;
    let port = port.ok_or_else(|| {
        PortError::unavailable(
            "forum.search_projection_owner_revision.owner_unavailable",
            "Forum projection owner revision source is temporarily unavailable",
        )
    })?;
    let revisions = port.list_owner_revisions(request).await?;
    validate_owner_revision_page(request, &revisions)?;
    Ok(revisions)
}

fn validate_owner_revision_request(
    request: ForumProjectionOwnerRevisionRequest,
) -> std::result::Result<(), PortError> {
    if request.tenant_id.is_nil() {
        return Err(PortError::validation(
            "forum.search_projection_owner_revision.tenant_required",
            "Forum projection owner revision source requires a tenant",
        ));
    }
    if request.after_owner_revision < 0 {
        return Err(PortError::validation(
            "forum.search_projection_owner_revision.cursor_invalid",
            "Forum projection owner revision cursor must not be negative",
        ));
    }
    if !(1..=MAX_FORUM_OWNER_REVISION_PAGE_LIMIT).contains(&request.limit) {
        return Err(PortError::validation(
            "forum.search_projection_owner_revision.limit_invalid",
            format!(
                "Forum projection owner revision limit must be between 1 and {MAX_FORUM_OWNER_REVISION_PAGE_LIMIT}"
            ),
        ));
    }
    Ok(())
}

fn validate_owner_revision_page(
    request: ForumProjectionOwnerRevisionRequest,
    revisions: &[ForumProjectionOwnerRevisionRecord],
) -> std::result::Result<(), PortError> {
    if revisions.len() > request.limit {
        return Err(owner_revision_invariant(
            "owner returned more revisions than requested",
        ));
    }

    let mut expected_revision = request
        .after_owner_revision
        .checked_add(1)
        .ok_or_else(|| owner_revision_invariant("owner revision cursor is exhausted"))?;
    for (index, revision) in revisions.iter().enumerate() {
        if revision.owner_revision != expected_revision {
            return Err(owner_revision_invariant(
                "owner revisions must be contiguous and strictly ordered after the requested cursor",
            ));
        }
        if revision.event_id.is_nil() {
            return Err(owner_revision_invariant(
                "owner revision event identity must not be nil",
            ));
        }
        let event_type = revision.event_type.trim();
        if event_type != "index.reindex_requested" {
            return Err(owner_revision_invariant(
                "owner revision event type must be the registered Forum projection invalidation",
            ));
        }
        if revision.impact != ForumProjectionOwnerRevisionImpact::FullRebuild {
            return Err(owner_revision_invariant(
                "owner revision ledger rows must require projection reconciliation",
            ));
        }
        if index + 1 < revisions.len() {
            expected_revision = expected_revision
                .checked_add(1)
                .ok_or_else(|| owner_revision_invariant("owner revision page overflowed"))?;
        }
    }
    Ok(())
}

fn owner_revision_invariant(message: &'static str) -> PortError {
    PortError::invariant_violation(
        "forum.search_projection_owner_revision.contract_invalid",
        message,
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ForumProjectionSweepReport {
    pub due_tenants: usize,
    pub claimed_events: usize,
    pub completed_events: usize,
    pub failed_events: usize,
    pub recovered_processing_events: usize,
    pub owner_tenants_scanned: usize,
    pub owner_tenants_reconciled: usize,
    pub owner_tenants_blocked: usize,
    pub owner_tenants_failed: usize,
    pub owner_revisions_checkpointed: usize,
    pub owner_rebuilds: usize,
}

#[derive(Clone)]
pub struct ForumProjectionReconciler {
    db: DatabaseConnection,
    projector: SearchProjector,
    blog_projector: BlogSearchProjector,
    forum_projector: ForumSearchProjector,
    inbox: ForumProjectionInbox,
    owner_checkpoint: Option<ForumOwnerCheckpointReconciler>,
}

impl ForumProjectionReconciler {
    pub fn new(db: DatabaseConnection, forum_source: Arc<dyn SearchProjectionSource>) -> Self {
        let forum_projector = ForumSearchProjector::new(db.clone(), forum_source);
        Self {
            projector: SearchProjector::new(db.clone()),
            blog_projector: BlogSearchProjector::new(db.clone()),
            forum_projector,
            inbox: ForumProjectionInbox::new(db.clone()),
            owner_checkpoint: None,
            db,
        }
    }

    pub fn with_owner_revision_source(
        db: DatabaseConnection,
        forum_source: Arc<dyn SearchProjectionSource>,
        owner_source: SharedForumProjectionOwnerRevisionSourcePort,
    ) -> Self {
        let forum_projector = ForumSearchProjector::new(db.clone(), forum_source);
        let owner_checkpoint =
            ForumOwnerCheckpointReconciler::new(db.clone(), forum_projector.clone(), owner_source);
        Self {
            projector: SearchProjector::new(db.clone()),
            blog_projector: BlogSearchProjector::new(db.clone()),
            forum_projector,
            inbox: ForumProjectionInbox::new(db.clone()),
            owner_checkpoint: Some(owner_checkpoint),
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

        if let Some(owner_checkpoint) = &self.owner_checkpoint {
            let owner_report = owner_checkpoint
                .sweep_due(
                    tenant_limit,
                    event_limit.min(MAX_FORUM_OWNER_REVISION_PAGE_LIMIT),
                )
                .await?;
            report.recovered_processing_events += owner_report.recovered_processing_events;
            report.owner_tenants_scanned += owner_report.owner_tenants_scanned;
            report.owner_tenants_reconciled += owner_report.owner_tenants_reconciled;
            report.owner_tenants_blocked += owner_report.owner_tenants_blocked;
            report.owner_tenants_failed += owner_report.owner_tenants_failed;
            report.owner_revisions_checkpointed += owner_report.owner_revisions_checkpointed;
            report.owner_rebuilds += owner_report.owner_rebuilds;
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

#[cfg(test)]
mod owner_revision_tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{PortError, PortErrorKind};
    use uuid::Uuid;

    use super::{
        ForumProjectionOwnerRevisionImpact, ForumProjectionOwnerRevisionRecord,
        ForumProjectionOwnerRevisionRequest, ForumProjectionOwnerRevisionSourcePort,
        SharedForumProjectionOwnerRevisionSourcePort, resolve_forum_projection_owner_revisions,
    };

    struct FixedRevisionSource {
        revisions: Vec<ForumProjectionOwnerRevisionRecord>,
    }

    #[async_trait]
    impl ForumProjectionOwnerRevisionSourcePort for FixedRevisionSource {
        async fn list_owner_revisions(
            &self,
            _request: ForumProjectionOwnerRevisionRequest,
        ) -> Result<Vec<ForumProjectionOwnerRevisionRecord>, PortError> {
            Ok(self.revisions.clone())
        }
    }

    fn request(after_owner_revision: i64) -> ForumProjectionOwnerRevisionRequest {
        ForumProjectionOwnerRevisionRequest {
            tenant_id: Uuid::new_v4(),
            after_owner_revision,
            limit: 10,
        }
    }

    fn record(owner_revision: i64) -> ForumProjectionOwnerRevisionRecord {
        ForumProjectionOwnerRevisionRecord {
            owner_revision,
            event_id: Uuid::new_v4(),
            event_type: "index.reindex_requested".to_string(),
            impact: ForumProjectionOwnerRevisionImpact::FullRebuild,
        }
    }

    #[tokio::test]
    async fn owner_revision_port_requires_host_composition() {
        let error = resolve_forum_projection_owner_revisions(None, request(0))
            .await
            .expect_err("missing owner source must fail closed");
        assert_eq!(error.kind, PortErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn owner_revision_page_accepts_contiguous_tenant_ledger_sequence() {
        let port: SharedForumProjectionOwnerRevisionSourcePort = Arc::new(FixedRevisionSource {
            revisions: vec![record(4), record(5)],
        });
        let revisions = resolve_forum_projection_owner_revisions(Some(port), request(3))
            .await
            .expect("valid owner revisions should resolve");
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[1].owner_revision, 5);
    }

    #[tokio::test]
    async fn owner_revision_page_rejects_gap_or_replay() {
        let port: SharedForumProjectionOwnerRevisionSourcePort = Arc::new(FixedRevisionSource {
            revisions: vec![record(8), record(10)],
        });
        let error = resolve_forum_projection_owner_revisions(Some(port), request(7))
            .await
            .expect_err("owner revision gaps must fail closed");
        assert_eq!(error.kind, PortErrorKind::InvariantViolation);
    }
}
