use async_trait::async_trait;
use rustok_api::PortError;
use std::sync::Arc;
use uuid::Uuid;

pub const MAX_FORUM_OWNER_TENANT_PAGE_LIMIT: usize = 256;
pub const DEFAULT_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 64;
pub const MAX_FORUM_OWNER_REVISION_PAGE_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForumProjectionOwnerTenantHead {
    pub tenant_id: Uuid,
    pub latest_owner_revision: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForumProjectionOwnerTenantPageRequest {
    pub after_tenant_id: Option<Uuid>,
    pub limit: usize,
}

pub async fn resolve_forum_projection_owner_tenant_heads(
    port: Option<SharedForumProjectionOwnerRevisionSourcePort>,
    request: ForumProjectionOwnerTenantPageRequest,
) -> std::result::Result<Vec<ForumProjectionOwnerTenantHead>, PortError> {
    validate_tenant_page_request(request)?;
    let port = port.ok_or_else(|| {
        PortError::unavailable(
            "forum.search_projection_owner_revision.owner_unavailable",
            "Forum projection owner revision source is temporarily unavailable",
        )
    })?;
    let heads = port.list_owner_revision_tenants(request).await?;
    validate_tenant_head_page(request, &heads)?;
    Ok(heads)
}

fn validate_tenant_page_request(
    request: ForumProjectionOwnerTenantPageRequest,
) -> std::result::Result<(), PortError> {
    if request
        .after_tenant_id
        .is_some_and(|tenant_id| tenant_id.is_nil())
    {
        return Err(PortError::validation(
            "forum.search_projection_owner_revision.tenant_cursor_invalid",
            "Forum projection owner tenant cursor must not be nil",
        ));
    }
    if !(1..=MAX_FORUM_OWNER_TENANT_PAGE_LIMIT).contains(&request.limit) {
        return Err(PortError::validation(
            "forum.search_projection_owner_revision.tenant_limit_invalid",
            format!(
                "Forum projection owner tenant limit must be between 1 and {MAX_FORUM_OWNER_TENANT_PAGE_LIMIT}"
            ),
        ));
    }
    Ok(())
}

fn validate_tenant_head_page(
    request: ForumProjectionOwnerTenantPageRequest,
    heads: &[ForumProjectionOwnerTenantHead],
) -> std::result::Result<(), PortError> {
    if heads.len() > request.limit {
        return Err(owner_source_invariant(
            "owner returned more tenant heads than requested",
        ));
    }

    let mut previous_tenant_id = request.after_tenant_id;
    for head in heads {
        if head.tenant_id.is_nil() {
            return Err(owner_source_invariant(
                "owner tenant identity must not be nil",
            ));
        }
        if previous_tenant_id.is_some_and(|previous| head.tenant_id <= previous) {
            return Err(owner_source_invariant(
                "owner tenant heads must be strictly ordered after the requested cursor",
            ));
        }
        if head.latest_owner_revision <= 0 {
            return Err(owner_source_invariant(
                "owner tenant head revision must be positive",
            ));
        }
        previous_tenant_id = Some(head.tenant_id);
    }
    Ok(())
}

fn owner_source_invariant(message: &'static str) -> PortError {
    PortError::invariant_violation(
        "forum.search_projection_owner_revision.contract_invalid",
        message,
    )
}

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
