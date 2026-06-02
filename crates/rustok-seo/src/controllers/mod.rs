use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
        Response as HttpResponse, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use loco_rs::{app::AppContext, controller::Routes};
use rustok_api::{
    graphql::ErrorCode, has_any_effective_permission,
    loco::transactional_event_bus_from_context, AuthContext, RequestContext, TenantContext,
};
use rustok_core::{ModuleRuntimeExtensions, Permission};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    SeoBulkJobRecord, SeoBulkJobStatus, SeoCrossLinkSuggestionRecord, SeoDiagnosticCountRecord,
    SeoDiagnosticSeverity, SeoDiagnosticsSummaryRecord, SeoError, SeoPageContext, SeoService,
    SeoSitemapJobRecord, SeoSitemapStatusRecord, SeoTargetCapabilityKind, SeoTargetRegistryEntry,
    SeoTargetSlug,
};

#[derive(Debug, Deserialize)]
pub struct SeoPageContextQuery {
    pub route: String,
}

#[derive(Debug, Deserialize)]
pub struct SeoTargetsQuery {
    pub capability: Option<SeoTargetCapabilityKind>,
}

#[derive(Debug, Deserialize)]
pub struct SeoCrossLinkSuggestionsQuery {
    pub locale: Option<String>,
    pub per_target_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SeoDiagnosticsQuery {
    pub locale: Option<String>,
    pub severity: Option<SeoDiagnosticSeverity>,
    pub code: Option<String>,
    pub target_kind: Option<SeoTargetSlug>,
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SeoSitemapJobsQuery {
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SeoBulkJobsQuery {
    pub limit: Option<i32>,
    pub status: Option<SeoBulkJobStatus>,
}

type SeoHttpResult<T> = std::result::Result<T, SeoHttpError>;

#[derive(Debug)]
struct SeoHttpError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
}

#[derive(Debug, Serialize)]
struct SeoRestErrorExtensions<'a> {
    code: &'a str,
}

#[derive(Debug, Serialize)]
struct SeoRestErrorItem<'a> {
    message: &'a str,
    extensions: SeoRestErrorExtensions<'a>,
}

#[derive(Debug, Serialize)]
struct SeoRestErrorEnvelope<'a> {
    errors: Vec<SeoRestErrorItem<'a>>,
}

impl SeoHttpError {
    fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn bad_user_input(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::BadUserInput, message)
    }

    fn internal_error(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::InternalError,
            message,
        )
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, message)
    }

    fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, ErrorCode::PermissionDenied, message)
    }

    fn envelope(&self) -> SeoRestErrorEnvelope<'_> {
        SeoRestErrorEnvelope {
            errors: vec![SeoRestErrorItem {
                message: self.message.as_str(),
                extensions: SeoRestErrorExtensions {
                    code: self.code.as_str(),
                },
            }],
        }
    }
}

impl IntoResponse for SeoHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.envelope())).into_response()
    }
}

pub async fn page_context_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request: RequestContext,
    Query(query): Query<SeoPageContextQuery>,
) -> SeoHttpResult<Json<SeoPageContext>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    let context = service
        .resolve_page_context_for_channel(
            &tenant,
            request.locale.as_str(),
            query.route.as_str(),
            request.channel_slug.as_deref(),
        )
        .await
        .map_err(map_seo_http_error)?
        .ok_or_else(|| SeoHttpError::not_found("SEO page context not found"))?;
    Ok(Json(context))
}

pub async fn robots_txt(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
) -> SeoHttpResult<Response> {
    let service = seo_service_from_app_ctx(&ctx)?;
    let body = service
        .render_robots(&tenant)
        .await
        .map_err(map_seo_http_error)?;
    Ok(([(CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response())
}

pub async fn sitemap_index(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
) -> SeoHttpResult<Response> {
    let service = seo_service_from_app_ctx(&ctx)?;
    if !service
        .load_settings(tenant.id)
        .await
        .map_err(map_seo_http_error)?
        .sitemap_enabled
    {
        return Err(SeoHttpError::not_found("SEO sitemap index is disabled"));
    }

    let file = match service
        .latest_sitemap_index(tenant.id)
        .await
        .map_err(map_seo_http_error)?
    {
        Some(file) => file,
        None => {
            service
                .generate_sitemaps(&tenant)
                .await
                .map_err(map_seo_http_error)?;
            service
                .latest_sitemap_index(tenant.id)
                .await
                .map_err(map_seo_http_error)?
                .ok_or_else(|| SeoHttpError::not_found("SEO sitemap index not found"))?
        }
    };

    Ok((
        [(CONTENT_TYPE, "application/xml; charset=utf-8")],
        file.content,
    )
        .into_response())
}

pub async fn sitemap_file(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(name): Path<String>,
) -> SeoHttpResult<Response> {
    let service = seo_service_from_app_ctx(&ctx)?;
    let file = service
        .sitemap_file(tenant.id, name.as_str())
        .await
        .map_err(map_seo_http_error)?
        .ok_or_else(|| SeoHttpError::not_found("SEO sitemap file not found"))?;

    Ok((
        [(CONTENT_TYPE, "application/xml; charset=utf-8")],
        file.content,
    )
        .into_response())
}

pub async fn diagnostics_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<SeoDiagnosticsQuery>,
) -> SeoHttpResult<Json<SeoDiagnosticsSummaryRecord>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(
        &auth,
        &[Permission::SEO_READ, Permission::SEO_MANAGE],
        "seo:read or seo:manage required",
    )?;

    let summary = service
        .diagnostics_summary(&tenant, query.locale.as_deref())
        .await
        .map_err(map_seo_http_error)?;

    Ok(Json(apply_diagnostics_filters(summary, &query)))
}

pub async fn sitemap_status_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
) -> SeoHttpResult<Json<SeoSitemapStatusRecord>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(
        &auth,
        &[Permission::SEO_READ, Permission::SEO_GENERATE],
        "seo:read or seo:generate required",
    )?;

    let status = service
        .sitemap_status(&tenant)
        .await
        .map_err(map_seo_http_error)?;
    Ok(Json(status))
}

pub async fn sitemap_jobs_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<SeoSitemapJobsQuery>,
) -> SeoHttpResult<Json<Vec<SeoSitemapJobRecord>>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(
        &auth,
        &[Permission::SEO_READ, Permission::SEO_GENERATE],
        "seo:read or seo:generate required",
    )?;

    let jobs = service
        .list_sitemap_jobs(tenant.id, query.limit.unwrap_or(20).clamp(1, 100) as usize)
        .await
        .map_err(map_seo_http_error)?;
    Ok(Json(jobs))
}

pub async fn sitemap_job_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(job_id): Path<Uuid>,
) -> SeoHttpResult<Json<SeoSitemapJobRecord>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(
        &auth,
        &[Permission::SEO_READ, Permission::SEO_GENERATE],
        "seo:read or seo:generate required",
    )?;

    let job = service
        .sitemap_job(tenant.id, job_id)
        .await
        .map_err(map_seo_http_error)?
        .ok_or_else(|| SeoHttpError::not_found("SEO sitemap job not found"))?;
    Ok(Json(job))
}

pub async fn bulk_jobs_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<SeoBulkJobsQuery>,
) -> SeoHttpResult<Json<Vec<SeoBulkJobRecord>>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(&auth, &[Permission::SEO_MANAGE], "seo:manage required")?;

    let jobs = service
        .list_bulk_jobs(
            tenant.id,
            query.limit.unwrap_or(20).clamp(1, 100) as usize,
            query.status,
        )
        .await
        .map_err(map_seo_http_error)?;

    Ok(Json(jobs))
}

pub async fn bulk_job_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(job_id): Path<Uuid>,
) -> SeoHttpResult<Json<SeoBulkJobRecord>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(&auth, &[Permission::SEO_MANAGE], "seo:manage required")?;

    let job = service
        .bulk_job(tenant.id, job_id)
        .await
        .map_err(map_seo_http_error)?
        .ok_or_else(|| SeoHttpError::not_found("SEO bulk job not found"))?;

    Ok(Json(job))
}

pub async fn bulk_artifact_download(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path((job_id, artifact_id)): Path<(Uuid, Uuid)>,
) -> SeoHttpResult<HttpResponse<axum::body::Body>> {
    ensure_seo_permission(&auth, &[Permission::SEO_MANAGE], "seo:manage required")?;

    let service = seo_service_from_app_ctx(&ctx)?;
    let artifact = service
        .bulk_artifact(tenant.id, job_id, artifact_id)
        .await
        .map_err(map_seo_http_error)?
        .ok_or_else(|| SeoHttpError::not_found("SEO bulk artifact not found"))?;

    HttpResponse::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, artifact.mime_type)
        .header(
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", artifact.file_name),
        )
        .body(axum::body::Body::from(artifact.content))
        .map_err(|err| {
            SeoHttpError::internal_error(format!("failed to build SEO bulk artifact response: {err}"))
        })
}

pub async fn targets_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<SeoTargetsQuery>,
) -> SeoHttpResult<Json<Vec<SeoTargetRegistryEntry>>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(&auth, &[Permission::SEO_MANAGE], "seo:manage required")?;
    Ok(Json(service.target_registry_entries(query.capability)))
}

pub async fn cross_link_suggestions_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<SeoCrossLinkSuggestionsQuery>,
) -> SeoHttpResult<Json<Vec<SeoCrossLinkSuggestionRecord>>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(
        &auth,
        &[Permission::SEO_READ, Permission::SEO_MANAGE],
        "seo:read or seo:manage required",
    )?;

    let suggestions = service
        .cross_link_suggestions(
            &tenant,
            query.locale.as_deref(),
            query.per_target_limit.map(|value| value.max(1) as usize),
        )
        .await
        .map_err(map_seo_http_error)?;

    Ok(Json(suggestions))
}

pub fn routes() -> Routes {
    use axum::routing::get;

    Routes::new()
        .add("/robots.txt", get(robots_txt))
        .add("/sitemap.xml", get(sitemap_index))
        .add("/sitemaps/{name}", get(sitemap_file))
        .nest("/api/seo", api_routes())
}

fn api_routes() -> Routes {
    use axum::routing::get;

    Routes::new()
        .add("/page-context", get(page_context_json))
        .add("/diagnostics", get(diagnostics_json))
        .add("/targets", get(targets_json))
        .add("/cross-link-suggestions", get(cross_link_suggestions_json))
        .add("/sitemaps/status", get(sitemap_status_json))
        .add("/sitemaps/jobs", get(sitemap_jobs_json))
        .add("/sitemaps/jobs/{job_id}", get(sitemap_job_json))
        .add("/bulk/jobs", get(bulk_jobs_json))
        .add("/bulk/jobs/{job_id}", get(bulk_job_json))
        .add(
            "/bulk/jobs/{job_id}/artifacts/{artifact_id}",
            get(bulk_artifact_download),
        )
}

fn ensure_seo_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> SeoHttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(SeoHttpError::permission_denied(message));
    }
    Ok(())
}

async fn ensure_seo_module_enabled(service: &SeoService, tenant_id: Uuid) -> SeoHttpResult<()> {
    if service
        .is_enabled(tenant_id)
        .await
        .map_err(map_seo_http_error)?
    {
        Ok(())
    } else {
        Err(SeoHttpError::not_found(
            "SEO module is not enabled for this tenant",
        ))
    }
}

fn map_seo_http_error(error: SeoError) -> SeoHttpError {
    match error {
        SeoError::Validation(message) => SeoHttpError::bad_user_input(message),
        SeoError::Configuration(message) => {
            tracing::warn!(message = %message, "SEO runtime wiring is incomplete");
            SeoHttpError::internal_error(message)
        }
        SeoError::NotFound => SeoHttpError::not_found("SEO record not found"),
        SeoError::PermissionDenied => SeoHttpError::permission_denied("Permission denied"),
        SeoError::Database(error) => {
            tracing::warn!(error = %error, "SEO HTTP handler failed");
            SeoHttpError::internal_error(error.to_string())
        }
    }
}

fn seo_service_from_app_ctx(ctx: &AppContext) -> SeoHttpResult<SeoService> {
    let extensions = ctx
        .shared_store
        .get::<std::sync::Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| {
            map_seo_http_error(SeoError::configuration(
                "SEO runtime extensions are not initialized; host bootstrap must insert ModuleRuntimeExtensions",
            ))
        })?;

    SeoService::from_runtime_extensions(
        ctx.db.clone(),
        transactional_event_bus_from_context(ctx),
        extensions.as_ref(),
    )
    .map_err(map_seo_http_error)
}

fn apply_diagnostics_filters(
    summary: SeoDiagnosticsSummaryRecord,
    query: &SeoDiagnosticsQuery,
) -> SeoDiagnosticsSummaryRecord {
    let code_filter = query
        .code
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    let mut issues = summary
        .issues
        .into_iter()
        .filter(|issue| {
            if let Some(severity) = query.severity {
                if issue.severity != severity {
                    return false;
                }
            }
            if let Some(code) = code_filter.as_deref() {
                if issue.code.to_ascii_lowercase() != code {
                    return false;
                }
            }
            if let Some(target_kind) = query.target_kind.as_ref() {
                if issue.target_kind != *target_kind {
                    return false;
                }
            }
            true
        })
        .collect::<Vec<_>>();

    if let Some(limit) = query.limit {
        issues.truncate(limit.clamp(1, 500) as usize);
    }

    let issue_count = issues.len() as i32;
    let error_count = issues
        .iter()
        .filter(|issue| issue.severity == SeoDiagnosticSeverity::Error)
        .count() as i32;
    let warning_count = issues
        .iter()
        .filter(|issue| issue.severity == SeoDiagnosticSeverity::Warning)
        .count() as i32;

    SeoDiagnosticsSummaryRecord {
        locale: summary.locale,
        total_targets: summary.total_targets,
        readiness_score: summary.readiness_score,
        issue_count,
        error_count,
        warning_count,
        generated_count: summary.generated_count,
        explicit_count: summary.explicit_count,
        fallback_count: summary.fallback_count,
        issue_counts_by_code: count_issue_keys(issues.iter().map(|issue| issue.code.as_str())),
        issue_counts_by_target_kind: count_issue_keys(
            issues.iter().map(|issue| issue.target_kind.as_str()),
        ),
        issues,
    }
}

fn count_issue_keys<'a>(keys: impl Iterator<Item = &'a str>) -> Vec<SeoDiagnosticCountRecord> {
    let mut counts = BTreeMap::<String, i32>::new();
    for key in keys {
        *counts.entry(key.to_string()).or_insert(0) += 1;
    }

    let mut result = counts
        .into_iter()
        .map(|(key, count)| SeoDiagnosticCountRecord { key, count })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use crate::{SeoDiagnosticIssueRecord, SeoTargetSlug};
    use serde_json::json;

    #[test]
    fn apply_diagnostics_filters_recomputes_issue_aggregates() {
        let summary = SeoDiagnosticsSummaryRecord {
            locale: "en-US".to_string(),
            total_targets: 10,
            readiness_score: 85,
            issue_count: 2,
            error_count: 1,
            warning_count: 1,
            generated_count: 4,
            explicit_count: 3,
            fallback_count: 3,
            issue_counts_by_code: vec![],
            issue_counts_by_target_kind: vec![],
            issues: vec![
                SeoDiagnosticIssueRecord {
                    code: "missing_title".to_string(),
                    severity: SeoDiagnosticSeverity::Error,
                    target_kind: SeoTargetSlug::new("page").expect("slug"),
                    target_id: Uuid::from_u128(1),
                    target_label: "Page A".to_string(),
                    route: "/a".to_string(),
                    locale: "en-US".to_string(),
                    message: "title missing".to_string(),
                    canonical_url: Some("/a".to_string()),
                    source: "fallback".to_string(),
                },
                SeoDiagnosticIssueRecord {
                    code: "missing_description".to_string(),
                    severity: SeoDiagnosticSeverity::Warning,
                    target_kind: SeoTargetSlug::new("product").expect("slug"),
                    target_id: Uuid::from_u128(2),
                    target_label: "Product B".to_string(),
                    route: "/b".to_string(),
                    locale: "en-US".to_string(),
                    message: "description missing".to_string(),
                    canonical_url: Some("/b".to_string()),
                    source: "generated".to_string(),
                },
            ],
        };

        let filtered = apply_diagnostics_filters(
            summary,
            &SeoDiagnosticsQuery {
                locale: None,
                severity: Some(SeoDiagnosticSeverity::Error),
                code: None,
                target_kind: None,
                limit: None,
            },
        );

        assert_eq!(filtered.issue_count, 1);
        assert_eq!(filtered.error_count, 1);
        assert_eq!(filtered.warning_count, 0);
        assert_eq!(filtered.issue_counts_by_code.len(), 1);
        assert_eq!(filtered.issue_counts_by_code[0].key, "missing_title");
        assert_eq!(filtered.issue_counts_by_target_kind.len(), 1);
        assert_eq!(filtered.issue_counts_by_target_kind[0].key, "page");
    }

    #[tokio::test]
    async fn seo_http_error_response_uses_graphql_compatible_envelope() {
        let response = SeoHttpError::permission_denied("seo:manage required").into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("error response body should serialize");
        let payload: serde_json::Value =
            serde_json::from_slice(body.as_ref()).expect("error response should be valid json");

        assert_eq!(
            payload,
            json!({
                "errors": [{
                    "message": "seo:manage required",
                    "extensions": { "code": "PERMISSION_DENIED" }
                }]
            })
        );
    }

    #[test]
    fn map_seo_http_error_maps_validation_to_bad_user_input_code() {
        let error = map_seo_http_error(SeoError::validation("invalid target locale"));

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code.as_str(), ErrorCode::BadUserInput.as_str());
        assert_eq!(error.message, "invalid target locale");
    }
}
