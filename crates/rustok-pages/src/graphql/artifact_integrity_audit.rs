use async_graphql::{
    Context, ErrorExtensions, FieldError, InputObject, Object, Result, SimpleObject,
};
use rustok_api::{
    Action, AuthContext, Permission, Resource, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    AuditPageArtifactsInput, PageArtifactIntegrityAuditResult, PageArtifactIntegrityFinding,
    PageService, PagesError,
};

const MODULE_SLUG: &str = "pages";
const PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT: &str =
    "PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT";
const PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED: &str = "PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED";

#[derive(Default)]
pub(crate) struct PageArtifactIntegrityAuditMutation;

#[Object]
impl PageArtifactIntegrityAuditMutation {
    async fn audit_page_artifacts(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: AuditGqlPageArtifactsInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPageArtifactIntegrityAuditResult> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_manage_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_current_tenant(tenant, &auth, tenant_id)?;
        let input = audit_input(input)?;

        PageService::new(db.clone(), event_bus.clone())
            .audit_immutable_artifact_integrity(tenant_id, page_security(&auth), id, input)
            .await
            .map(Into::into)
            .map_err(map_artifact_audit_error)
    }
}

#[derive(InputObject)]
pub struct AuditGqlPageArtifactsInput {
    pub max_records: Option<i32>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageArtifactIntegrityFinding {
    pub artifact_id: Uuid,
    pub locale_hash: String,
    pub record_identity_hash: String,
    pub code: String,
    pub diagnostic_hash: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageArtifactIntegrityAuditResult {
    pub format: String,
    pub page_id: Uuid,
    pub max_records: i32,
    pub scanned_artifact_count: i32,
    pub valid_artifact_count: i32,
    pub invalid_artifact_count: i32,
    pub truncated: bool,
    pub findings_truncated: bool,
    pub findings: Vec<GqlPageArtifactIntegrityFinding>,
    pub audit_hash: String,
}

impl From<PageArtifactIntegrityFinding> for GqlPageArtifactIntegrityFinding {
    fn from(finding: PageArtifactIntegrityFinding) -> Self {
        Self {
            artifact_id: finding.artifact_id,
            locale_hash: finding.locale_hash,
            record_identity_hash: finding.record_identity_hash,
            code: finding.code,
            diagnostic_hash: finding.diagnostic_hash,
        }
    }
}

impl From<PageArtifactIntegrityAuditResult> for GqlPageArtifactIntegrityAuditResult {
    fn from(result: PageArtifactIntegrityAuditResult) -> Self {
        Self {
            format: result.format,
            page_id: result.page_id,
            max_records: result.max_records as i32,
            scanned_artifact_count: result.scanned_artifact_count as i32,
            valid_artifact_count: result.valid_artifact_count as i32,
            invalid_artifact_count: result.invalid_artifact_count as i32,
            truncated: result.truncated,
            findings_truncated: result.findings_truncated,
            findings: result.findings.into_iter().map(Into::into).collect(),
            audit_hash: result.audit_hash,
        }
    }
}

fn audit_input(input: AuditGqlPageArtifactsInput) -> Result<AuditPageArtifactsInput> {
    let max_records = match input.max_records {
        Some(value) => Some(u32::try_from(value).map_err(|_| {
            public_graphql_error(
                PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT,
                "Invalid immutable artifact audit input",
            )
        })?),
        None => None,
    };
    Ok(AuditPageArtifactsInput { max_records })
}

fn require_pages_manage_permission(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    let permission = Permission::new(Resource::Pages, Action::Manage);
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: pages:manage required",
        ));
    }
    Ok(auth)
}

fn resolve_current_tenant(
    tenant: &TenantContext,
    auth: &AuthContext,
    requested: Option<Uuid>,
) -> Result<Uuid> {
    if auth.tenant_id != tenant.id || requested.is_some_and(|id| id != tenant.id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Pages artifact audits must use the current tenant",
        ));
    }
    Ok(tenant.id)
}

fn page_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn map_artifact_audit_error(error: PagesError) -> async_graphql::Error {
    match error {
        PagesError::PageNotFound(_) => public_graphql_error("PAGE_NOT_FOUND", "Page not found"),
        PagesError::Forbidden(_) => {
            public_graphql_error("PAGES_PERMISSION_DENIED", "Permission denied")
        }
        PagesError::Validation(_) => public_graphql_error(
            PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT,
            "Invalid immutable artifact audit input",
        ),
        _ => public_graphql_error(
            PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED,
            "Immutable artifact audit failed",
        ),
    }
}

fn public_graphql_error(code: &'static str, message: &'static str) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
    })
}
