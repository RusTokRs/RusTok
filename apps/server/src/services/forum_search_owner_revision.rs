use async_trait::async_trait;
use rustok_api::{PortError, PortErrorKind};
use rustok_forum::{
    ForumError, ForumEventService, ForumProjectionOwnerRevisionImpact as ForumOwnerRevisionImpact,
};
use rustok_search::{
    ForumProjectionOwnerRevisionImpact, ForumProjectionOwnerRevisionRecord,
    ForumProjectionOwnerRevisionRequest, ForumProjectionOwnerRevisionSourcePort,
    ForumProjectionOwnerTenantHead, ForumProjectionOwnerTenantPageRequest,
    SharedForumProjectionOwnerRevisionSourcePort,
};
use sea_orm::DatabaseConnection;

pub(crate) struct ServerForumProjectionOwnerRevisionSourcePort {
    db: DatabaseConnection,
}

impl ServerForumProjectionOwnerRevisionSourcePort {
    pub(crate) fn shared(db: DatabaseConnection) -> SharedForumProjectionOwnerRevisionSourcePort {
        std::sync::Arc::new(Self { db })
    }
}

#[async_trait]
impl ForumProjectionOwnerRevisionSourcePort for ServerForumProjectionOwnerRevisionSourcePort {
    async fn list_owner_revisions(
        &self,
        request: ForumProjectionOwnerRevisionRequest,
    ) -> Result<Vec<ForumProjectionOwnerRevisionRecord>, PortError> {
        let revisions = ForumEventService::new(self.db.clone())
            .list_projection_owner_revisions(
                request.tenant_id,
                request.after_owner_revision,
                request.limit,
            )
            .await
            .map_err(map_forum_error)?;

        Ok(revisions
            .into_iter()
            .map(|revision| ForumProjectionOwnerRevisionRecord {
                owner_revision: revision.owner_revision,
                event_id: revision.event_id,
                event_type: revision.event_type,
                impact: match revision.impact {
                    ForumOwnerRevisionImpact::FullRebuild => {
                        ForumProjectionOwnerRevisionImpact::FullRebuild
                    }
                },
            })
            .collect())
    }

    async fn list_owner_revision_tenants(
        &self,
        request: ForumProjectionOwnerTenantPageRequest,
    ) -> Result<Vec<ForumProjectionOwnerTenantHead>, PortError> {
        let heads = ForumEventService::new(self.db.clone())
            .list_projection_owner_revision_tenants(request.after_tenant_id, request.limit)
            .await
            .map_err(map_forum_error)?;

        Ok(heads
            .into_iter()
            .map(|head| ForumProjectionOwnerTenantHead {
                tenant_id: head.tenant_id,
                latest_owner_revision: head.latest_owner_revision,
            })
            .collect())
    }
}

fn map_forum_error(error: ForumError) -> PortError {
    let stable_code = error.stable_code().to_ascii_lowercase();
    let retryable = error.is_retryable();
    match error {
        ForumError::Validation(message) => PortError::validation(stable_code, message),
        ForumError::Database(_)
        | ForumError::CapabilityUnavailable { .. }
        | ForumError::CapabilityFailure { .. }
        | ForumError::Internal(_) => PortError::new(
            PortErrorKind::Unavailable,
            stable_code,
            "Forum projection owner revision source is temporarily unavailable",
            retryable,
        ),
        _ => PortError::invariant_violation(
            stable_code,
            "Forum projection owner revision source could not be resolved safely",
        ),
    }
}
