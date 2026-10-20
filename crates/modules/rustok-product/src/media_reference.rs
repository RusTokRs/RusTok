//! Product-owned consumer projection of Media durable references. No Media tables
//! or owner service are imported here. The host adapts embedded/remote Media ports.
use crate::{
    CatalogService, CommerceError, CommerceResult,
    entities::{
        product, product_image, product_image_media_coverage, product_media_release,
        product_media_retention_readiness,
    },
};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{PortActor, PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set,
};
use serde::{Deserialize, Serialize};
use std::{future::Future, sync::Arc, time::Duration};
use uuid::Uuid;

pub const PRODUCT_MEDIA_PAGE_LIMIT: u64 = 100;
const DEADLINE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductMediaReferencePolicy {
    DurableRetention,
    Unavailable,
    ExternalOwner,
}
impl ProductMediaReferencePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DurableRetention => "durable_retention",
            Self::Unavailable => "unavailable",
            Self::ExternalOwner => "external_owner",
        }
    }
}
/// An explicitly consumer-local projection, not an alternate Media hold record.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductMediaReference {
    pub tenant_id: Uuid,
    pub media_id: Uuid,
    pub image_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductMediaReferencePage {
    pub references: Vec<ProductMediaReference>,
    pub next_image_id: Option<Uuid>,
    pub has_more: bool,
}
#[async_trait]
pub trait ProductMediaReferencePort: Send + Sync {
    async fn retain(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError>;
    async fn release(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError>;
    async fn lookup(
        &self,
        context: PortContext,
        image_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductMediaReference>, PortError>;
    async fn list(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReferencePage, PortError>;
}

fn reference_deadline(context: &PortContext, write: bool) -> Result<Duration, PortError> {
    context.require_policy(if write {
        PortCallPolicy::write()
    } else {
        PortCallPolicy::read()
    })?;
    Ok(Duration::from_millis(
        context
            .deadline_ms
            .unwrap_or(1)
            .min(DEADLINE.as_millis() as u64),
    ))
}
async fn bounded<T>(
    deadline: Duration,
    future: impl Future<Output = Result<T, PortError>>,
) -> Result<T, PortError> {
    tokio::time::timeout(deadline, future).await.map_err(|_| {
        PortError::timeout(
            "product.media_timeout",
            "Product Media operation exceeded its deadline",
        )
    })?
}
struct BoundedProductMediaReferencePort {
    inner: Arc<dyn ProductMediaReferencePort>,
}
pub(crate) fn bound_media_reference_port(
    inner: Arc<dyn ProductMediaReferencePort>,
) -> Arc<dyn ProductMediaReferencePort> {
    Arc::new(BoundedProductMediaReferencePort { inner })
}
#[async_trait]
impl ProductMediaReferencePort for BoundedProductMediaReferencePort {
    async fn retain(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        bounded(
            reference_deadline(&context, true)?,
            self.inner.retain(context, media_id, image_id),
        )
        .await
    }
    async fn release(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        bounded(
            reference_deadline(&context, true)?,
            self.inner.release(context, media_id, image_id),
        )
        .await
    }
    async fn lookup(
        &self,
        context: PortContext,
        image_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductMediaReference>, PortError> {
        bounded(
            reference_deadline(&context, false)?,
            self.inner.lookup(context, image_ids),
        )
        .await
    }
    async fn list(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReferencePage, PortError> {
        bounded(
            reference_deadline(&context, false)?,
            self.inner.list(context, after, limit),
        )
        .await
    }
}

/// Operator-only maintenance boundary, composed with the same owner as commands.
#[async_trait]
pub trait ProductMediaMaintenancePort: Send + Sync {
    async fn releases(
        &self,
        context: PortContext,
        after_image: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReleasePage, PortError>;
    async fn audit(
        &self,
        context: PortContext,
        after_image: Option<Uuid>,
        after_hold: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaAuditPage, PortError>;
    async fn backfill(
        &self,
        context: PortContext,
        after_image: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaBackfillPage, PortError>;
}
pub(crate) fn maintenance_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Core(error) => crate::error::core_port_error(&error),
        CommerceError::Conflict(_) => PortError::conflict(
            "product.conflict",
            "product request conflicts with the current state",
        ),
        CommerceError::Validation(_) => PortError::validation(
            "product.media_maintenance_invalid",
            "Product Media maintenance request is invalid",
        ),
        _ => PortError::unavailable(
            "product.media_maintenance_unavailable",
            "Product Media maintenance is unavailable",
        ),
    }
}
#[async_trait]
impl ProductMediaMaintenancePort for CatalogService {
    async fn releases(
        &self,
        context: PortContext,
        after_image: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReleasePage, PortError> {
        let deadline = reference_deadline(&context, false)?;
        require_maintenance_permission(&context, false)?;
        bounded(deadline, async {
            self.list_media_releases(context, after_image, limit)
                .await
                .map_err(maintenance_error)
        })
        .await
    }
    async fn audit(
        &self,
        context: PortContext,
        after_image: Option<Uuid>,
        after_hold: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaAuditPage, PortError> {
        let deadline = reference_deadline(&context, false)?;
        require_maintenance_permission(&context, false)?;
        bounded(deadline, async {
            self.audit_media_references(context, after_image, after_hold, limit)
                .await
                .map_err(maintenance_error)
        })
        .await
    }
    async fn backfill(
        &self,
        context: PortContext,
        after_image: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaBackfillPage, PortError> {
        let deadline = reference_deadline(&context, true)?;
        require_maintenance_permission(&context, true)?;
        bounded(deadline, async {
            self.backfill_media_references(context, after_image, limit)
                .await
                .map_err(maintenance_error)
        })
        .await
    }
}

tokio::task_local! { static MEDIA_CONTEXT: PortContext; }
pub(crate) async fn with_media_context<F: Future>(context: PortContext, future: F) -> F::Output {
    MEDIA_CONTEXT.scope(context, future).await
}
// Product-owned admitted operation context, shared by retention and cross-owner
// admission reads. Trusted direct service calls receive a tenant/actor-bound fallback.
pub(crate) fn operation_context(
    tenant: Uuid,
    actor: Uuid,
    image: Uuid,
) -> CommerceResult<PortContext> {
    if tenant.is_nil() || actor.is_nil() {
        return Err(storage_error());
    }
    let context = MEDIA_CONTEXT.try_with(Clone::clone).unwrap_or_else(|_| {
        PortContext::new(
            tenant.to_string(),
            PortActor::user(actor.to_string()),
            "und",
            image.to_string(),
        )
        .with_idempotency_key(format!("product-internal:{image}"))
        .with_deadline(DEADLINE)
    });
    if Uuid::parse_str(&context.tenant_id).ok() != Some(tenant)
        || Uuid::parse_str(&context.actor.id).ok() != Some(actor)
    {
        return Err(storage_error());
    }
    Ok(context)
}
fn storage_error() -> CommerceError {
    CommerceError::Database(sea_orm::DbErr::Custom(
        "product Media capability is unavailable".into(),
    ))
}
fn media_error(error: PortError) -> CommerceError {
    tracing::warn!(code = %error.code, retryable = error.retryable, "Product Media reference operation failed");
    match error.kind {
        PortErrorKind::NotFound => {
            CommerceError::Validation("product image Media asset was not found".into())
        }
        _ => storage_error(),
    }
}
fn tenant(context: &PortContext, write: bool) -> CommerceResult<Uuid> {
    context
        .require_policy(if write {
            PortCallPolicy::write()
        } else {
            PortCallPolicy::read()
        })
        .map_err(|_| {
            CommerceError::Validation("invalid Product Media maintenance context".into())
        })?;
    if context.actor.id.trim().is_empty()
        || context.actor.id.len() > 191
        || (context.actor.kind == PortActorKind::User
            && Uuid::parse_str(&context.actor.id)
                .ok()
                .is_none_or(|id| id.is_nil()))
    {
        return Err(CommerceError::Validation(
            "invalid Product Media actor".into(),
        ));
    }
    Uuid::parse_str(&context.tenant_id)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| CommerceError::Validation("invalid Product tenant".into()))
}
fn maintenance(context: &PortContext, write: bool) -> CommerceResult<Uuid> {
    let id = tenant(context, write)?;
    require_maintenance_permission(context, write).map_err(|_| {
        CommerceError::Validation("Product Media maintenance permission is required".into())
    })?;
    Ok(id)
}
fn require_maintenance_permission(context: &PortContext, write: bool) -> Result<(), PortError> {
    let permission = if write {
        rustok_api::Permission::PRODUCTS_MANAGE
    } else {
        rustok_api::Permission::PRODUCTS_READ
    };
    let claims: Vec<rustok_api::Permission> = context
        .claims
        .iter()
        .filter_map(|claim| claim.parse().ok())
        .collect();
    if !rustok_api::has_effective_permission(&claims, &permission) {
        return Err(PortError::forbidden(
            "product.media_maintenance_forbidden",
            "Product Media maintenance permission is required",
        ));
    }
    Ok(())
}
fn limit(value: u64) -> CommerceResult<u64> {
    if value == 0 || value > PRODUCT_MEDIA_PAGE_LIMIT {
        return Err(CommerceError::Validation(
            "Product Media page limit must be 1..100".into(),
        ));
    }
    Ok(value)
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProductMediaAuditIssue {
    pub kind: String,
    pub image_id: Uuid,
    pub media_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProductMediaAuditPage {
    pub issues: Vec<ProductMediaAuditIssue>,
    pub next_image_id: Option<Uuid>,
    pub next_hold_id: Option<Uuid>,
    pub images_have_more: bool,
    pub holds_have_more: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProductMediaBackfillPage {
    pub examined: u64,
    pub retained: u64,
    pub failed: Vec<Uuid>,
    pub next_image_id: Option<Uuid>,
    pub has_more: bool,
    pub legacy_images_remaining: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProductMediaReleaseReport {
    pub examined: u64,
    pub released: u64,
    pub failed: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProductMediaReleaseStatus {
    pub image_id: Uuid,
    pub media_id: Uuid,
    pub attempts: i32,
    pub next_attempt_at: chrono::DateTime<Utc>,
    pub last_error_code: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProductMediaReleasePage {
    pub jobs: Vec<ProductMediaReleaseStatus>,
    pub next_image_id: Option<Uuid>,
    pub has_more: bool,
}

impl CatalogService {
    /// Tenant-bounded operational inspection. Stored actor/claims/context never
    /// leave Product storage through this response.
    pub async fn list_media_releases(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        page_limit: u64,
    ) -> CommerceResult<ProductMediaReleasePage> {
        let tenant_id = maintenance(&context, false)?;
        let page_limit = limit(page_limit)?;
        let mut query = product_media_release::Entity::find()
            .filter(product_media_release::Column::TenantId.eq(tenant_id));
        if let Some(after) = after {
            query = query.filter(product_media_release::Column::ImageId.gt(after));
        }
        let mut rows = query
            .order_by_asc(product_media_release::Column::ImageId)
            .limit(page_limit + 1)
            .all(&self.db)
            .await?;
        let has_more = rows.len() > page_limit as usize;
        rows.truncate(page_limit as usize);
        let next_image_id = rows.last().map(|row| row.image_id);
        let jobs = rows
            .into_iter()
            .map(|row| ProductMediaReleaseStatus {
                image_id: row.image_id,
                media_id: row.media_id,
                attempts: row.attempts,
                next_attempt_at: row.next_attempt_at,
                last_error_code: row.last_error_code,
            })
            .collect();
        Ok(ProductMediaReleasePage {
            jobs,
            next_image_id,
            has_more,
        })
    }
    pub(crate) async fn has_unverified_media_images(
        &self,
        tenant_id: Uuid,
    ) -> CommerceResult<bool> {
        if product_media_retention_readiness::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .is_some()
        {
            return Ok(false);
        }
        let parents = product::Entity::find()
            .select_only()
            .column(product::Column::Id)
            .filter(product::Column::TenantId.eq(tenant_id))
            .into_query();
        let covered = product_image_media_coverage::Entity::find()
            .select_only()
            .column(product_image_media_coverage::Column::ImageId)
            .filter(product_image_media_coverage::Column::TenantId.eq(tenant_id))
            .into_query();
        Ok(product_image::Entity::find()
            .filter(product_image::Column::ProductId.in_subquery(parents))
            .filter(product_image::Column::Id.not_in_subquery(covered))
            .limit(1)
            .one(&self.db)
            .await?
            .is_some())
    }
    pub(crate) async fn mark_image_media_provisioned<C: ConnectionTrait>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        image_id: Uuid,
    ) -> CommerceResult<()> {
        product_image_media_coverage::Entity::insert(product_image_media_coverage::ActiveModel {
            image_id: Set(image_id),
            tenant_id: Set(tenant_id),
        })
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(product_image_media_coverage::Column::ImageId)
                .do_nothing()
                .to_owned(),
        )
        .try_insert()
        .exec(txn)
        .await?;
        Ok(())
    }
    pub(crate) async fn mark_media_retention_ready<C: ConnectionTrait>(
        &self,
        txn: &C,
        tenant_id: Uuid,
    ) -> CommerceResult<()> {
        product_media_retention_readiness::Entity::insert(
            product_media_retention_readiness::ActiveModel {
                tenant_id: Set(tenant_id),
            },
        )
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(
                product_media_retention_readiness::Column::TenantId,
            )
            .do_nothing()
            .to_owned(),
        )
        .try_insert()
        .exec(txn)
        .await?;
        Ok(())
    }
    fn reference_port(&self) -> CommerceResult<Arc<dyn ProductMediaReferencePort>> {
        self.media_reference_port.clone().ok_or_else(storage_error)
    }
    /// Called only after receipt admission, before the Product transaction. A
    /// rollback/ambiguous commit deliberately never compensates a retained hold.
    pub(crate) async fn retain_product_image(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        image_id: Uuid,
        media_id: Uuid,
    ) -> CommerceResult<()> {
        let context = operation_context(tenant_id, actor_id, image_id)?;
        self.reference_port()?
            .retain(context, media_id, image_id)
            .await
            .map_err(media_error)
    }
    /// Insert in the SAME transaction as relation deletion and receipt completion.
    pub(crate) async fn enqueue_product_media_release<C: ConnectionTrait>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Uuid,
        image: &product_image::Model,
    ) -> CommerceResult<()> {
        let context = operation_context(tenant_id, actor_id, image.id)?;
        let context = serde_json::to_value(context).map_err(|_| storage_error())?;
        product_media_release::ActiveModel {
            image_id: Set(image.id),
            tenant_id: Set(tenant_id),
            media_id: Set(image.media_id),
            context: Set(context),
            attempts: Set(0),
            next_attempt_at: Set(Utc::now()),
            last_error_code: Set(None),
        }
        .insert(txn)
        .await?;
        Ok(())
    }
    /// Crash-safe at-least-once delivery of committed removals. Multi-replica
    /// duplicate delivery is safe: Media receives the exact stored actor/request
    /// and deterministic caller key. Success is never undone by queue-ack failure.
    pub async fn process_media_releases(
        &self,
        batch_limit: u64,
    ) -> CommerceResult<ProductMediaReleaseReport> {
        process_product_media_releases(&self.db, self.reference_port()?, batch_limit).await
    }
    async fn image_reference_page(
        &self,
        tenant_id: Uuid,
        after: Option<Uuid>,
        page_limit: u64,
    ) -> CommerceResult<(Vec<product_image::Model>, bool)> {
        let parents = product::Entity::find()
            .select_only()
            .column(product::Column::Id)
            .filter(product::Column::TenantId.eq(tenant_id))
            .into_query();
        let mut query = product_image::Entity::find()
            .filter(product_image::Column::ProductId.in_subquery(parents));
        if let Some(after) = after {
            query = query.filter(product_image::Column::Id.gt(after));
        }
        let mut rows = query
            .order_by_asc(product_image::Column::Id)
            .limit(page_limit + 1)
            .all(&self.db)
            .await?;
        let more = rows.len() > page_limit as usize;
        rows.truncate(page_limit as usize);
        Ok((rows, more))
    }
    /// Bounded, diagnostic only. Two owner pages are not a distributed snapshot;
    /// in-flight retain can legitimately look orphaned. NEVER release from here.
    pub async fn audit_media_references(
        &self,
        context: PortContext,
        after_image: Option<Uuid>,
        after_hold: Option<Uuid>,
        page_limit: u64,
    ) -> CommerceResult<ProductMediaAuditPage> {
        let tenant_id = maintenance(&context, false)?;
        let page_limit = limit(page_limit)?;
        let port = self.reference_port()?;
        let (images, images_have_more) = self
            .image_reference_page(tenant_id, after_image, page_limit)
            .await?;
        let holds = port
            .lookup(context.clone(), images.iter().map(|row| row.id).collect())
            .await
            .map_err(media_error)?;
        let unique_hold_ids: std::collections::BTreeSet<_> =
            holds.iter().map(|hold| hold.image_id).collect();
        if unique_hold_ids.len() != holds.len()
            || holds.len() > images.len()
            || holds.iter().any(|hold| {
                hold.tenant_id != tenant_id || !images.iter().any(|image| image.id == hold.image_id)
            })
        {
            return Err(storage_error());
        }
        let mut issues = Vec::new();
        for image in &images {
            let hold = holds.iter().find(|hold| hold.image_id == image.id);
            let kind = match hold {
                None => Some("missing_media_hold"),
                Some(hold) if hold.media_id != image.media_id => Some("media_reference_mismatch"),
                _ => None,
            };
            if let Some(kind) = kind {
                issues.push(ProductMediaAuditIssue {
                    kind: kind.into(),
                    image_id: image.id,
                    media_id: image.media_id,
                });
            }
        }
        let page = port
            .list(context, after_hold, page_limit)
            .await
            .map_err(media_error)?;
        if page.next_image_id != page.references.last().map(|hold| hold.image_id)
            || page
                .references
                .windows(2)
                .any(|rows| rows[0].image_id >= rows[1].image_id)
            || (page.has_more && page.references.len() != page_limit as usize)
            || page.references.len() > page_limit as usize
            || page.references.iter().any(|hold| {
                hold.tenant_id != tenant_id
                    || after_hold.is_some_and(|after| hold.image_id <= after)
            })
        {
            return Err(storage_error());
        }
        // Resolve the bounded Media page in one tenant-scoped Product read,
        // rather than spending the maintenance deadline on one roundtrip per hold.
        let local_images: std::collections::BTreeMap<_, _> = if page.references.is_empty() {
            std::collections::BTreeMap::new()
        } else {
            product_image::Entity::find()
                .filter(
                    product_image::Column::Id
                        .is_in(page.references.iter().map(|hold| hold.image_id)),
                )
                .filter(
                    product_image::Column::ProductId.in_subquery(
                        product::Entity::find()
                            .select_only()
                            .column(product::Column::Id)
                            .filter(product::Column::TenantId.eq(tenant_id))
                            .into_query(),
                    ),
                )
                .all(&self.db)
                .await?
                .into_iter()
                .map(|image| (image.id, image))
                .collect()
        };
        for hold in &page.references {
            let image = local_images.get(&hold.image_id);
            let kind = match image {
                None => Some("orphan_media_hold"),
                Some(image) if image.media_id != hold.media_id => Some("media_reference_mismatch"),
                _ => None,
            };
            if let Some(kind) = kind {
                issues.push(ProductMediaAuditIssue {
                    kind: kind.into(),
                    image_id: hold.image_id,
                    media_id: hold.media_id,
                });
            }
        }
        Ok(ProductMediaAuditPage {
            issues,
            next_image_id: images.last().map(|image| image.id).or(after_image),
            next_hold_id: page.next_image_id.or(after_hold),
            images_have_more,
            holds_have_more: page.has_more,
        })
    }
    /// Explicit operator backfill, never a migration-time remote call or automatic
    /// orphan repair. Keep existing relation UUIDs; failures are returned per row.
    /// Caller must restart failed pages with the same key for uncertain writes, or
    /// deliberately choose a new repair key after a finalized failed receipt.
    pub async fn backfill_media_references(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        page_limit: u64,
    ) -> CommerceResult<ProductMediaBackfillPage> {
        let tenant_id = maintenance(&context, true)?;
        let page_limit = limit(page_limit)?;
        let port = self.reference_port()?;
        let (images, has_more) = self
            .image_reference_page(tenant_id, after, page_limit)
            .await?;
        let mut report = ProductMediaBackfillPage {
            examined: images.len() as u64,
            retained: 0,
            failed: Vec::new(),
            next_image_id: images.last().map(|image| image.id),
            has_more,
            legacy_images_remaining: true,
        };
        for image in images {
            match port.retain(context.clone(), image.media_id, image.id).await {
                Ok(()) => {
                    // The FK refuses a marker if the relation was concurrently
                    // deleted. The conservative retained orphan is only diagnosed.
                    if self
                        .mark_image_media_provisioned(&self.db, tenant_id, image.id)
                        .await
                        .is_ok()
                    {
                        report.retained += 1;
                    } else {
                        report.failed.push(image.id);
                    }
                }
                Err(error) => {
                    tracing::warn!(code = %error.code, image_id = %image.id, "Product Media backfill failed");
                    report.failed.push(image.id);
                }
            }
        }
        report.legacy_images_remaining = self.has_unverified_media_images(tenant_id).await?;
        if !report.legacy_images_remaining {
            self.mark_media_retention_ready(&self.db, tenant_id).await?;
        }
        Ok(report)
    }
}

/// Trusted owner-worker entry point; this is not a browser-facing operation.
pub async fn process_product_media_releases(
    db: &sea_orm::DatabaseConnection,
    port: Arc<dyn ProductMediaReferencePort>,
    batch_limit: u64,
) -> CommerceResult<ProductMediaReleaseReport> {
    let port = bound_media_reference_port(port);
    let batch_limit = limit(batch_limit)?;
    let jobs = product_media_release::Entity::find()
        .filter(product_media_release::Column::NextAttemptAt.lte(Utc::now()))
        .order_by_asc(product_media_release::Column::NextAttemptAt)
        .order_by_asc(product_media_release::Column::ImageId)
        .limit(batch_limit)
        .all(db)
        .await?;
    let mut report = ProductMediaReleaseReport::default();
    for job in jobs {
        report.examined += 1;
        let context: Result<PortContext, _> = serde_json::from_value(job.context.clone());
        let live = product_image::Entity::find_by_id(job.image_id)
            .one(db)
            .await?;
        let result = match context {
            Ok(mut context)
                if live.is_none() && tenant(&context, true).ok() == Some(job.tenant_id) =>
            {
                // Delivery is a new bounded attempt of an already committed
                // obligation, not part of the caller's expired request budget.
                // Actor, logical key, claims and tracing provenance stay stable.
                context.deadline_ms = Some(DEADLINE.as_millis() as u64);
                let release = port
                    .release(context.clone(), job.media_id, job.image_id)
                    .await;
                // A missing hold proves delivery, even after ambiguous release or
                // receipt expiry. Conversely an old successful receipt is not proof
                // of current absence after a concurrent explicit backfill.
                match port.lookup(context, vec![job.image_id]).await {
                    Ok(holds) if holds.is_empty() => Ok(()),
                    _ => release.and_then(|_| {
                        Err(PortError::unavailable(
                            "product.media_release_unconfirmed",
                            "Media release is not confirmed",
                        ))
                    }),
                }
            }
            _ => Err(PortError::invariant_violation(
                "product.media_release_invalid",
                "Product removal could not be verified",
            )),
        };
        match result {
            Ok(()) => {
                product_media_release::Entity::delete_by_id(job.image_id)
                    .exec(db)
                    .await?;
                report.released += 1;
            }
            Err(error) => {
                tracing::warn!(code = %error.code, image_id = %job.image_id, "Product Media release retained for retry");
                let attempts = job.attempts.saturating_add(1);
                // Bounded backoff prevents an unavailable first page starving the queue.
                let delay = (1_i64 << attempts.clamp(1, 10)).min(900);
                // Another replica may already have acknowledged/deleted this row,
                // or scheduled a newer retry while this attempt was in flight.
                // A stale failure is not an error and must not resurrect/overwrite it.
                product_media_release::Entity::update_many()
                    .col_expr(
                        product_media_release::Column::Attempts,
                        sea_orm::sea_query::Expr::value(attempts),
                    )
                    .col_expr(
                        product_media_release::Column::NextAttemptAt,
                        sea_orm::sea_query::Expr::value(
                            Utc::now() + ChronoDuration::seconds(delay),
                        ),
                    )
                    .col_expr(
                        product_media_release::Column::LastErrorCode,
                        sea_orm::sea_query::Expr::value("media_release_failed"),
                    )
                    .filter(product_media_release::Column::ImageId.eq(job.image_id))
                    .filter(product_media_release::Column::Attempts.eq(job.attempts))
                    .exec(db)
                    .await?;
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> PortContext {
        PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::new_v4().to_string()),
            "en",
            "test",
        )
        .with_idempotency_key("test")
        .with_deadline(Duration::from_millis(1))
    }
    #[test]
    fn maintenance_uses_effective_permissions_and_rejects_anonymous_or_nil_scope() {
        let base = context();
        assert!(maintenance(&base, true).is_err());
        let managed = base
            .clone()
            .with_claim(rustok_api::Permission::PRODUCTS_MANAGE.to_string());
        assert!(maintenance(&managed, true).is_ok());
        assert!(maintenance(&managed, false).is_ok());
        let mut service = managed.clone();
        service.actor = PortActor::service("product-media-operator");
        assert!(maintenance(&service, true).is_ok());
        let read = base.with_claim(rustok_api::Permission::PRODUCTS_READ.to_string());
        assert!(maintenance(&read, false).is_ok());
        assert!(maintenance(&read, true).is_err());
        let mut invalid = managed;
        invalid.actor.id = Uuid::nil().to_string();
        assert!(maintenance(&invalid, true).is_err());
    }
    #[test]
    fn only_authoritative_not_found_is_terminal_media_validation() {
        assert!(matches!(
            media_error(PortError::not_found("media.not_found", "not found")),
            CommerceError::Validation(_)
        ));
        assert!(matches!(
            media_error(PortError::forbidden("media.denied", "denied")),
            CommerceError::Database(_)
        ));
        assert!(matches!(
            media_error(PortError::unavailable("media.unknown_commit", "unknown")),
            CommerceError::Database(_)
        ));
    }
    #[tokio::test]
    async fn inherited_context_must_match_the_owner_operation_tenant_and_actor() {
        let original = context();
        let tenant = Uuid::parse_str(&original.tenant_id).unwrap();
        let actor = Uuid::parse_str(&original.actor.id).unwrap();
        let image = Uuid::new_v4();
        with_media_context(original.clone(), async {
            assert_eq!(operation_context(tenant, actor, image).unwrap(), original);
            assert!(operation_context(Uuid::new_v4(), actor, image).is_err());
            assert!(operation_context(tenant, Uuid::new_v4(), image).is_err());
        })
        .await;
    }
    #[tokio::test]
    async fn timeout_is_real_and_does_not_invoke_a_compensating_action() {
        let context = context();
        let result: Result<(), PortError> = bounded(
            reference_deadline(&context, true).unwrap(),
            std::future::pending(),
        )
        .await;
        assert_eq!(result.unwrap_err().kind, PortErrorKind::Timeout);
        let mut generous = context;
        generous.deadline_ms = Some(600_000);
        assert_eq!(reference_deadline(&generous, true).unwrap(), DEADLINE);
    }
}
