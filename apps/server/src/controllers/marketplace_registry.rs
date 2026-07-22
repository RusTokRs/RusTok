use axum::{
    body::Body,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
        HeaderMap, HeaderName, HeaderValue, Response, StatusCode,
    },
    response::IntoResponse,
    routing::{get, post, put},
    Json,
};
use object_store::ObjectStoreExt;

/// Maximum allowed size for a registry publish-bundle upload (2 MiB).
///
/// Publish bundles contain only contract metadata and bounded manifest text;
/// executable payloads belong to the immutable artifact/CAS pipeline. The
/// Axum limit rejects oversized requests before they are read into memory.
const REGISTRY_ARTIFACT_MAX_BYTES: usize =
    crate::services::registry_governance::MODULE_PUBLISH_ARTIFACT_MAX_BYTES;
const LEGACY_REGISTRY_ACTOR_HEADER: &str = concat!("x-rustok-", "actor");
const LEGACY_REGISTRY_PUBLISHER_HEADER: &str = concat!("x-rustok-", "publisher");
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{http_error, Error};
use crate::modules::{CatalogManifestModule, ManifestManager};
use crate::services::marketplace_catalog::{
    registry_catalog_from_modules, registry_catalog_module_path, registry_catalog_path,
    registry_owner_transfer_path, registry_publish_approve_path, registry_publish_artifact_path,
    registry_publish_external_stage_path, registry_publish_hold_path, registry_publish_path,
    registry_publish_platform_build_stage_path, registry_publish_reject_path,
    registry_publish_request_changes_path, registry_publish_resume_path,
    registry_publish_stage_report_path, registry_publish_status_path,
    registry_publish_validate_path, registry_runner_claim_path, registry_runner_complete_path,
    registry_runner_fail_path, registry_runner_heartbeat_path, registry_yank_path,
    validate_registry_mutation_schema_version, RegistryCatalogModule, RegistryCatalogResponse,
    RegistryExternalPrebuiltStageRequest, RegistryExternalPrebuiltStageResponse,
    RegistryGovernanceAction, RegistryMutationResponse, RegistryOwnerTransferRequest,
    RegistryPlatformBuildStageRequest, RegistryPlatformBuildStageResponse,
    RegistryPublishDecisionRequest, RegistryPublishRequest, RegistryPublishStatusFollowUpGate,
    RegistryPublishStatusResponse, RegistryPublishStatusValidationStage,
    RegistryPublishValidationRequest, RegistryRunnerClaimPayload, RegistryRunnerClaimRequest,
    RegistryRunnerClaimResponse, RegistryRunnerCompletionRequest, RegistryRunnerHeartbeatRequest,
    RegistryRunnerMutationResponse, RegistryValidationStageReportRequest, RegistryYankRequest,
};
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::registry_governance::{
    release_status_label, request_status_label, validation_stage_status_label,
    RegistryArtifactUpload, RegistryExternalPrebuiltStageInput, RegistryFollowUpGateSnapshot,
    RegistryGovernanceActionSnapshot, RegistryGovernanceError, RegistryGovernanceService,
    RegistryPlatformBuildStageInput, RegistryValidationStageSnapshot,
    REGISTRY_APPROVE_OVERRIDE_REASON_CODES, REGISTRY_HOLD_REASON_CODES,
    REGISTRY_OWNER_TRANSFER_REASON_CODES, REGISTRY_REJECT_REASON_CODES,
    REGISTRY_REQUEST_CHANGES_REASON_CODES, REGISTRY_RESUME_REASON_CODES,
    REGISTRY_VALIDATION_STAGE_REASON_CODES, REGISTRY_YANK_REASON_CODES,
};
use crate::services::registry_principal::RegistryAuthority;
use crate::services::registry_remote_runner::claim_remote_validation_stage_atomic;
use crate::services::registry_remote_transitions::{
    finish_remote_validation_stage_atomic, heartbeat_remote_validation_stage_atomic,
    RegistryRemoteTransitionError, RemoteTerminalOutcome,
};
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_api::context::AuthContextExtension;
use rustok_api::request::RequestContext;
use rustok_modules::{ModuleExternalSourceEvidence, ModuleGovernanceError};
use rustok_web::HttpError;

#[derive(Debug, Default, Deserialize, ToSchema, utoipa::IntoParams)]
struct RegistryCatalogListParams {
    search: Option<String>,
    category: Option<String>,
    tag: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

/// GET /catalog - Reference read-only marketplace registry catalog
#[utoipa::path(
    get,
    path = "/catalog",
    tag = "marketplace",
    params(
        RegistryCatalogListParams,
        ("If-None-Match" = Option<String>, Header, description = "Conditional request ETag")
    ),
    responses(
        (
            status = 200,
            description = "Schema-versioned reference catalog of first-party modules with optional filtering and paging",
            body = RegistryCatalogResponse,
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry"),
                ("x-total-count" = i64, description = "Total number of modules in the filtered collection before limit/offset")
            )
        ),
        (
            status = 304,
            description = "Catalog has not changed since the provided ETag",
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry"),
                ("x-total-count" = i64, description = "Total number of modules in the filtered collection before limit/offset")
            )
        )
    )
)]
async fn catalog(
    State(ctx): State<ServerRuntimeContext>,
    request_context: RequestContext,
    headers: HeaderMap,
    Query(params): Query<RegistryCatalogListParams>,
) -> Result<Response<Body>, Error> {
    let first_party_modules = sort_catalog_modules(filter_catalog_modules(
        first_party_catalog_modules(&ctx, &request_context).await?,
        &params,
    ));
    let (first_party_modules, total_count) = paginate_catalog_modules(first_party_modules, &params);
    let payload = registry_catalog_from_modules(first_party_modules);

    build_registry_response(&headers, &payload, Some(total_count))
}

/// GET /catalog/{slug} - Reference read-only marketplace registry module detail
#[utoipa::path(
    get,
    path = "/catalog/{slug}",
    tag = "marketplace",
    params(
        ("slug" = String, Path, description = "Module slug"),
        ("If-None-Match" = Option<String>, Header, description = "Conditional request ETag")
    ),
    responses(
        (
            status = 200,
            description = "Normalized first-party module detail from the reference registry catalog",
            body = RegistryCatalogModule,
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry")
            )
        ),
        (
            status = 304,
            description = "Module detail has not changed since the provided ETag",
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry")
            )
        ),
        (
            status = 404,
            description = "Module is not present in the reference registry catalog"
        )
    )
)]
async fn catalog_module(
    State(ctx): State<ServerRuntimeContext>,
    request_context: RequestContext,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response<Body>, Error> {
    let module = first_party_catalog_modules(&ctx, &request_context)
        .await?
        .into_iter()
        .find(|module| module.slug == slug)
        .map(RegistryCatalogModule::from_catalog_module)
        .ok_or(Error::NotFound)?;

    build_registry_response(&headers, &module, None)
}

/// POST /v2/catalog/publish - Registry publish request entrypoint
#[utoipa::path(
    post,
    path = "/v2/catalog/publish",
    tag = "marketplace",
    request_body = RegistryPublishRequest,
    responses(
        (
            status = 200,
            description = "Dry-run registry publish request accepted and normalized",
            body = RegistryMutationResponse
        ),
        (
            status = 202,
            description = "Live registry publish request created and awaiting artifact upload",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Publish request failed local contract validation"
        )
    )
)]
async fn publish(
    State(ctx): State<ServerRuntimeContext>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPublishRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;

    let warnings = RegistryGovernanceService::publish_request_warnings(&request)
        .map_err(map_registry_governance_error)?;

    if !request.dry_run {
        let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
        let authority = authority_from_auth(&headers, auth, "Registry publish operations")?;
        if !request.module.ownership.eq_ignore_ascii_case("first_party")
            && !authority.can_manage_modules
        {
            return Err(Error::BadRequest(
                "Live third-party registry publish requires modules.manage authority".to_string(),
            ));
        }
        if matches!(
            request.module.artifact_origin,
            crate::services::marketplace_catalog::RegistryPublishArtifactOrigin::ExternalPrebuilt
                | crate::services::marketplace_catalog::RegistryPublishArtifactOrigin::AlloyAuthored
        ) && !authority.can_manage_modules
        {
            return Err(http_error(HttpError::forbidden(
                "forbidden",
                "External prebuilt and Alloy-authored publish requests require modules.manage authority",
            )));
        }
        let created = RegistryGovernanceService::new(ctx.db_clone())
            .create_publish_request(&request, &authority)
            .await
            .map_err(map_registry_governance_error)?;

        return Ok((
            StatusCode::ACCEPTED,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "publish".to_string(),
                dry_run: false,
                accepted: true,
                request_id: Some(created.id.clone()),
                status: Some(request_status_label(created.status).to_string()),
                slug: created.slug,
                version: created.version,
                warnings,
                errors: Vec::new(),
                next_step: Some(format!(
                    "Upload the module artifact via PUT {}",
                    registry_publish_artifact_path().replace("{request_id}", &created.id)
                )),
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "publish".to_string(),
            dry_run: true,
            accepted: true,
            request_id: None,
            status: Some("dry_run".to_string()),
            slug: request.module.slug.clone(),
            version: request.module.version.clone(),
            warnings,
            errors: Vec::new(),
            next_step: Some(
                "Dry-run preview only. Re-run with dry_run=false to create a publish request."
                    .to_string(),
            ),
        }),
    ))
}

/// GET /v2/catalog/publish/{request_id} - Registry publish request lifecycle status
#[utoipa::path(
    get,
    path = "/v2/catalog/publish/{request_id}",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    responses(
        (
            status = 200,
            description = "Current lifecycle state of a registry publish request",
            body = RegistryPublishStatusResponse
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn publish_status(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
) -> Result<Json<RegistryPublishStatusResponse>, Error> {
    let governance = RegistryGovernanceService::new(ctx.db_clone());
    let request = governance
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = optional_authority_from_auth(&headers, auth)?;
    let follow_up = governance
        .publish_request_follow_up_snapshot_for_authority(&request, authority.as_ref())
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to load registry publish request follow-up stages: {error}"
            ))
        })?;
    let mut warnings = deserialize_message_list(&request.validation_warnings);
    let next_step =
        publish_request_status_next_step(&request, &request_id, &follow_up.validation_stages);
    if follow_up.approval_override_required {
        warnings.push(approval_override_warning_message(
            &follow_up.validation_stages,
        ));
    }

    Ok(Json(RegistryPublishStatusResponse {
        schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
        request_id: request.id,
        slug: request.slug,
        version: request.version,
        status: request_status_label(request.status.clone()).to_string(),
        artifact_origin: request.artifact_origin.clone(),
        accepted: publish_request_accepted(&request.status),
        warnings,
        errors: deserialize_message_list(&request.validation_errors),
        follow_up_gates: follow_up
            .follow_up_gates
            .into_iter()
            .map(publish_status_follow_up_gate)
            .collect(),
        validation_stages: follow_up
            .validation_stages
            .iter()
            .map(publish_status_validation_stage)
            .collect(),
        approval_override_required: follow_up.approval_override_required,
        approval_override_reason_codes: REGISTRY_APPROVE_OVERRIDE_REASON_CODES
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        governance_actions: follow_up
            .governance_actions
            .into_iter()
            .map(publish_status_governance_action)
            .collect(),
        next_step,
    }))
}

/// PUT /v2/catalog/publish/{request_id}/artifact - Upload a registry publish artifact
#[utoipa::path(
    put,
    path = "/v2/catalog/publish/{request_id}/artifact",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body(
        content = String,
        content_type = "application/octet-stream",
        description = "Opaque module publish artifact bytes"
    ),
    responses(
        (
            status = 202,
            description = "Artifact uploaded and queued for validation",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Artifact upload failed local validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn upload_publish_artifact(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    body: Bytes,
) -> Result<impl IntoResponse, Error> {
    if body.is_empty() {
        return Err(Error::BadRequest(
            "Registry publish artifact upload requires a non-empty request body".to_string(),
        ));
    }

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream")
        .to_string();
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry artifact upload")?;
    let storage = ctx
        .shared_get::<rustok_storage::StorageRuntime>()
        .ok_or_else(|| Error::Message("StorageRuntime not initialized".to_string()))?;

    let request = RegistryGovernanceService::new(ctx.db_clone())
        .with_storage(storage)
        .upload_publish_artifact(
            &request_id,
            &authority,
            RegistryArtifactUpload {
                content_type,
                bytes: body,
            },
        )
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "publish".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&request.status),
            request_id: Some(request.id.clone()),
            status: Some(request_status_label(request.status.clone()).to_string()),
            slug: request.slug,
            version: request.version,
            warnings: deserialize_message_list(&request.validation_warnings),
            errors: deserialize_message_list(&request.validation_errors),
            next_step: publish_request_next_step(&request.status, &request_id),
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/external-prebuilt-stage - Record an operator-approved external prebuilt stage
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/external-prebuilt-stage",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryExternalPrebuiltStageRequest,
    responses(
        (
            status = 200,
            description = "External prebuilt stage accepted or idempotently replayed",
            body = RegistryExternalPrebuiltStageResponse
        ),
        (
            status = 400,
            description = "External source, provenance, quarantine, or artifact evidence is invalid"
        ),
        (
            status = 403,
            description = "External prebuilt staging requires modules.manage authority"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn stage_external_prebuilt(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryExternalPrebuiltStageRequest>,
) -> Result<Json<RegistryExternalPrebuiltStageResponse>, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let source_evidence = external_prebuilt_source_evidence(&request)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "External prebuilt staging")?;
    if !authority.can_manage_modules {
        return Err(http_error(HttpError::forbidden(
            "forbidden",
            "External prebuilt staging requires modules.manage authority",
        )));
    }
    let governance = RegistryGovernanceService::new(ctx.db_clone());
    let existing = governance
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    if request.dry_run {
        return Ok(Json(RegistryExternalPrebuiltStageResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            request_id,
            slug: existing.slug,
            version: existing.version,
            status: "dry_run".to_string(),
            staging_id: None,
            created: false,
            dry_run: true,
            next_step: Some(
                "Re-run with dry_run=false to persist the external provenance and quarantine stage."
                    .to_string(),
            ),
        }));
    }

    let staged = governance
        .stage_external_prebuilt(
            &request_id,
            &authority,
            RegistryExternalPrebuiltStageInput {
                artifact_digest: request.artifact_digest,
                source_evidence,
                provenance_reference: request.provenance_reference,
                provenance_digest: request.provenance_digest,
                provenance_policy_revision: request.provenance_policy_revision,
                quarantine_review_reference: request.quarantine_review_reference,
                quarantine_policy_revision: request.quarantine_policy_revision,
                idempotency_key: request.idempotency_key,
            },
        )
        .await
        .map_err(map_registry_governance_error)?;
    Ok(Json(RegistryExternalPrebuiltStageResponse {
        schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
        request_id,
        slug: existing.slug,
        version: existing.version,
        status: request_status_label(existing.status).to_string(),
        staging_id: Some(staged.staging_id),
        created: staged.created,
        dry_run: false,
        next_step: Some(
            "Continue registry validation and final approval through the owner workflow."
                .to_string(),
        ),
    }))
}

/// POST /v2/catalog/publish/{request_id}/platform-build-stage - Bind one completed tenant-scoped platform build to the uploaded artifact
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/platform-build-stage",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPlatformBuildStageRequest,
    responses(
        (
            status = 200,
            description = "Platform build stage accepted or idempotently replayed",
            body = RegistryPlatformBuildStageResponse
        ),
        (
            status = 400,
            description = "The completed tenant-scoped build does not match the uploaded artifact"
        ),
        (
            status = 403,
            description = "Platform build staging requires modules.manage authority"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        ),
        (
            status = 409,
            description = "Idempotency key was reused with different immutable build input"
        )
    )
)]
async fn stage_platform_build(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPlatformBuildStageRequest>,
) -> Result<Json<RegistryPlatformBuildStageResponse>, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Platform build staging")?;
    if !authority.can_manage_modules {
        return Err(http_error(HttpError::forbidden(
            "forbidden",
            "Platform build staging requires modules.manage authority",
        )));
    }
    let tenant_id = auth
        .map(|AuthContextExtension(context)| context.tenant_id)
        .ok_or_else(|| {
            Error::Unauthorized("Platform build staging requires authentication".into())
        })?;
    let governance = RegistryGovernanceService::new(ctx.db_clone());
    let existing = governance
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    if request.dry_run {
        return Ok(Json(RegistryPlatformBuildStageResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            request_id,
            slug: existing.slug,
            version: existing.version,
            status: "dry_run".to_string(),
            staging_id: None,
            created: false,
            dry_run: true,
            next_step: Some(
                "Re-run with dry_run=false to bind the completed tenant-scoped platform build."
                    .to_string(),
            ),
        }));
    }

    let staged = governance
        .stage_platform_build(
            &request_id,
            &authority,
            RegistryPlatformBuildStageInput {
                tenant_id,
                build_request_id: request.build_request_id,
                idempotency_key: request.idempotency_key,
            },
        )
        .await
        .map_err(map_registry_governance_error)?;
    Ok(Json(RegistryPlatformBuildStageResponse {
        schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
        request_id,
        slug: existing.slug,
        version: existing.version,
        status: request_status_label(existing.status).to_string(),
        staging_id: Some(staged.staging_id),
        created: staged.created,
        dry_run: false,
        next_step: Some(
            "Continue registry validation and final approval through the owner workflow."
                .to_string(),
        ),
    }))
}

fn external_prebuilt_source_evidence(
    request: &RegistryExternalPrebuiltStageRequest,
) -> Result<ModuleExternalSourceEvidence, Error> {
    match (
        request.source_reference.as_ref(),
        request.source_digest.as_ref(),
        request.source_absence_reason.as_ref(),
    ) {
        (Some(reference), Some(digest), None) => Ok(ModuleExternalSourceEvidence::Reproducible {
            reference: reference.clone(),
            digest: digest.clone(),
        }),
        (None, None, Some(reason_code)) => Ok(ModuleExternalSourceEvidence::Unavailable {
            reason_code: reason_code.clone(),
        }),
        _ => Err(Error::BadRequest(
            "External prebuilt staging requires either sourceReference plus sourceDigest or sourceAbsenceReason, but not both."
                .to_string(),
        )),
    }
}

async fn download_publish_artifact(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
) -> Result<Response<Body>, Error> {
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let has_user_session = optional_authority_from_auth(&headers, auth)?.is_some();
    if !has_user_session && require_remote_executor_access(&ctx, &headers).is_err() {
        return Err(Error::Unauthorized(
            "Registry artifact download requires a user session or x-rustok-runner-token"
                .to_string(),
        ));
    }

    let request = RegistryGovernanceService::new(ctx.db_clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let storage_key = request
        .artifact_storage_key
        .clone()
        .ok_or_else(|| Error::NotFound)?;
    let storage = ctx
        .shared_get::<rustok_storage::StorageRuntime>()
        .ok_or_else(|| Error::Message("StorageRuntime not initialized".to_string()))?;

    if let Some(download_url) = storage
        .signed_download_url(
            &object_store::path::Path::from(storage_key.as_str()),
            std::time::Duration::from_secs(300),
        )
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to create private download URL: {error}"))
        })?
    {
        return Ok(axum::response::Redirect::temporary(&download_url).into_response());
    }

    let bytes = storage
        .objects
        .get(&object_store::path::Path::from(storage_key.as_str()))
        .await
        .map_err(|error| Error::Message(format!("Failed to read registry artifact: {error}")))?
        .bytes()
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to read registry artifact body: {error}"))
        })?;
    let content_type = request
        .artifact_content_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(CACHE_CONTROL, "private, no-store")
        .body(Body::from(bytes))
        .map_err(|error| {
            Error::Message(format!(
                "Failed to build artifact download response: {error}"
            ))
        })
}

/// POST /v2/catalog/publish/{request_id}/validate - Run publish artifact validation outside the upload path
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/validate",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishValidationRequest,
    responses(
        (
            status = 200,
            description = "Publish request validation already completed or returned the current terminal lifecycle state",
            body = RegistryMutationResponse
        ),
        (
            status = 202,
            description = "Publish request validation was accepted and queued as a background lifecycle step",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Validation request failed lifecycle or governance checks"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn validate_publish_request_step(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPublishValidationRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let existing = RegistryGovernanceService::new(ctx.db_clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry validation operations")?;

    if request.dry_run {
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "validate".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings: vec!["Dry-run preview only. Re-run with dry_run=false to execute publish validation outside the upload path.".to_string()],
                errors: Vec::new(),
                next_step: Some("Use the same endpoint with dry_run=false after artifact upload completes.".to_string()),
            }),
        ));
    }

    let governance = RegistryGovernanceService::new(ctx.db_clone());
    let validation = governance
        .validate_publish_request(&request_id, &authority)
        .await
        .map_err(map_registry_governance_error)?;
    let validated = validation.request;
    let status_code = if validated.status
        == crate::models::registry_publish_request::RegistryPublishRequestStatus::Validating
    {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };

    Ok((
        status_code,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "validate".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&validated.status),
            request_id: Some(validated.id.clone()),
            status: Some(request_status_label(validated.status.clone()).to_string()),
            slug: validated.slug,
            version: validated.version,
            warnings: deserialize_message_list(&validated.validation_warnings),
            errors: deserialize_message_list(&validated.validation_errors),
            next_step: publish_request_next_step(&validated.status, &validated.id),
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/stages - Record or requeue an external validation stage
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/stages",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryValidationStageReportRequest,
    responses(
        (
            status = 200,
            description = "Dry-run preview or live validation stage update accepted",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Validation stage update failed lifecycle or governance checks"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn report_validation_stage(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryValidationStageReportRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    validate_validation_stage_report_request(&request)?;
    let existing = RegistryGovernanceService::new(ctx.db_clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry validation stage reporting")?;

    if request.dry_run {
        let mut warnings = Vec::new();
        let normalized_status = request.status.trim().to_ascii_lowercase();
        if matches!(normalized_status.as_str(), "passed" | "failed" | "blocked")
            && request
                .reason_code
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty())
        {
            warnings.push(format!(
                "Live validation stage status '{}' should include reason_code ({}).",
                normalized_status,
                REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
            ));
        }
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "validation_stage".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some(normalized_status),
                slug: existing.slug,
                version: existing.version,
                warnings,
                errors: Vec::new(),
                next_step: Some(
                    "Dry-run preview only. Re-run with dry_run=false to persist the validation stage update."
                        .to_string(),
                ),
            }),
        ));
    }

    let result = RegistryGovernanceService::new(ctx.db_clone())
        .report_validation_stage(
            &request_id,
            &authority,
            &request.stage,
            &request.status,
            request.detail.as_deref(),
            request.reason_code.as_deref(),
            request.requeue,
        )
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "validation_stage".to_string(),
            dry_run: false,
            accepted: true,
            request_id: Some(result.request.id.clone()),
            status: Some(validation_stage_status_label(result.stage.status).to_string()),
            slug: result.request.slug,
            version: result.request.version,
            warnings: Vec::new(),
            errors: Vec::new(),
            next_step: Some(format!(
                "Inspect {} for the updated publish lifecycle and follow-up validation stages.",
                registry_publish_status_path().replace("{request_id}", &request_id)
            )),
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/approve - Finalize a validated publish request
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/approve",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier"),
        ("Idempotency-Key" = String, Header, description = "Required non-nil UUID for final approval; not required for dry-run previews")
    ),
    request_body = RegistryPublishDecisionRequest,
    responses(
        (
            status = 200,
            description = "Validated publish request approved and projected into the published registry release trail",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Approve request failed governance validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn approve_publish_request(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPublishDecisionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    validate_publish_approve_request(&request)?;
    let governance = RegistryGovernanceService::new(ctx.db_clone());
    let existing = governance
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let follow_up = governance
        .publish_request_follow_up_snapshot(&existing)
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to load registry publish request follow-up stages: {error}"
            ))
        })?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry publish approval")?;

    if request.dry_run {
        let mut warnings = vec![String::from(
            "Dry-run preview only. Re-run with dry_run=false to finalize the publish request.",
        )];
        let next_step = if follow_up.approval_override_required {
            warnings.push(approval_override_warning_message(
                &follow_up.validation_stages,
            ));
            Some(approval_override_next_step(
                &existing.id,
                &follow_up.validation_stages,
            ))
        } else {
            Some(
                "Use the same endpoint with dry_run=false after artifact validation succeeds."
                    .to_string(),
            )
        };
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "approve".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings,
                errors: Vec::new(),
                next_step,
            }),
        ));
    }

    let idempotency_key = required_non_nil_idempotency_key(&headers)?;
    let approved = RegistryGovernanceService::new(ctx.db_clone())
        .approve_publish_request(
            &request_id,
            &authority,
            idempotency_key,
            request.reason.as_deref(),
            request.reason_code.as_deref(),
        )
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "approve".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&approved.status),
            request_id: Some(approved.id.clone()),
            status: Some(request_status_label(approved.status.clone()).to_string()),
            slug: approved.slug,
            version: approved.version,
            warnings: deserialize_message_list(&approved.validation_warnings),
            errors: deserialize_message_list(&approved.validation_errors),
            next_step: publish_request_next_step(&approved.status, &approved.id),
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/reject - Reject a publish request with a governance reason
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/reject",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishDecisionRequest,
    responses(
        (
            status = 200,
            description = "Publish request rejected with surfaced governance reason",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Reject request failed governance validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn reject_publish_request(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPublishDecisionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_publish_reject_request(&request)?;
    let existing = RegistryGovernanceService::new(ctx.db_clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry publish rejection")?;

    if request.dry_run {
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "reject".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings: warnings
                    .into_iter()
                    .chain(std::iter::once(
                        "Dry-run preview only. Re-run with dry_run=false to persist the governance rejection."
                            .to_string(),
                    ))
                    .collect(),
                errors: Vec::new(),
                next_step: Some(format!(
                    "Use the same endpoint with dry_run=false, a non-empty reason, and a supported reason_code ({}) to reject the publish request.",
                    REGISTRY_REJECT_REASON_CODES.join(", ")
                )),
            }),
        ));
    }
    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live registry publish reject requires a non-empty reason for the governance audit trail"
                    .to_string(),
            )
        })?;
    let reason_code = request
        .reason_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live registry publish reject requires a non-empty reason_code for the policy audit trail"
                    .to_string(),
            )
        })?;

    let rejected = RegistryGovernanceService::new(ctx.db_clone())
        .reject_publish_request(&request_id, &authority, reason, reason_code)
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "reject".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&rejected.status),
            request_id: Some(rejected.id.clone()),
            status: Some(request_status_label(rejected.status.clone()).to_string()),
            slug: rejected.slug,
            version: rejected.version,
            warnings,
            errors: deserialize_message_list(&rejected.validation_errors),
            next_step: publish_request_next_step(&rejected.status, &rejected.id),
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/request-changes - Request a fresh artifact revision
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/request-changes",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishDecisionRequest,
    responses(
        (
            status = 200,
            description = "Publish request moved into changes_requested",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Request-changes failed governance validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn request_changes_publish_request(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPublishDecisionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_publish_request_changes_request(&request)?;
    let existing = RegistryGovernanceService::new(ctx.db_clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry request-changes operations")?;

    if request.dry_run {
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "request_changes".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings: warnings
                    .into_iter()
                    .chain(std::iter::once(
                        "Dry-run preview only. Re-run with dry_run=false to request a fresh artifact revision."
                            .to_string(),
                    ))
                    .collect(),
                errors: Vec::new(),
                next_step: Some(format!(
                    "Use the same endpoint with dry_run=false, a non-empty reason, and a supported reason_code ({}) to move the request into changes_requested.",
                    REGISTRY_REQUEST_CHANGES_REASON_CODES.join(", ")
                )),
            }),
        ));
    }

    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live request-changes requires a non-empty reason for the governance audit trail"
                    .to_string(),
            )
        })?;
    let reason_code = request
        .reason_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live request-changes requires a non-empty reason_code for the policy audit trail"
                    .to_string(),
            )
        })?;

    let updated = RegistryGovernanceService::new(ctx.db_clone())
        .request_changes_publish_request(&request_id, &authority, reason, reason_code)
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "request_changes".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&updated.status),
            request_id: Some(updated.id.clone()),
            status: Some(request_status_label(updated.status.clone()).to_string()),
            slug: updated.slug,
            version: updated.version,
            warnings,
            errors: deserialize_message_list(&updated.validation_errors),
            next_step: publish_request_next_step(&updated.status, &updated.id),
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/hold - Temporarily pause a publish request
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/hold",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishDecisionRequest,
    responses(
        (
            status = 200,
            description = "Publish request moved into on_hold",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Hold request failed governance validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn hold_publish_request(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPublishDecisionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_publish_hold_request(&request)?;
    let existing = RegistryGovernanceService::new(ctx.db_clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry hold operations")?;

    if request.dry_run {
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "hold".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings: warnings
                    .into_iter()
                    .chain(std::iter::once(
                        "Dry-run preview only. Re-run with dry_run=false to place the request on hold."
                            .to_string(),
                    ))
                    .collect(),
                errors: Vec::new(),
                next_step: Some(format!(
                    "Use the same endpoint with dry_run=false, a non-empty reason, and a supported reason_code ({}) to move the request into on_hold.",
                    REGISTRY_HOLD_REASON_CODES.join(", ")
                )),
            }),
        ));
    }

    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live hold requires a non-empty reason for the governance audit trail".to_string(),
            )
        })?;
    let reason_code = request
        .reason_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live hold requires a non-empty reason_code for the policy audit trail".to_string(),
            )
        })?;

    let updated = RegistryGovernanceService::new(ctx.db_clone())
        .hold_publish_request(&request_id, &authority, reason, reason_code)
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "hold".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&updated.status),
            request_id: Some(updated.id.clone()),
            status: Some(request_status_label(updated.status.clone()).to_string()),
            slug: updated.slug,
            version: updated.version,
            warnings,
            errors: deserialize_message_list(&updated.validation_errors),
            next_step: publish_request_next_step(&updated.status, &updated.id),
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/resume - Resume a held publish request
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/resume",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishDecisionRequest,
    responses(
        (
            status = 200,
            description = "Held publish request resumed back into its previous status",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Resume request failed governance validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn resume_publish_request(
    State(ctx): State<ServerRuntimeContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryPublishDecisionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_publish_resume_request(&request)?;
    let existing = RegistryGovernanceService::new(ctx.db_clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
    let authority = authority_from_auth(&headers, auth, "Registry resume operations")?;

    if request.dry_run {
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "resume".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings: warnings
                    .into_iter()
                    .chain(std::iter::once(
                        "Dry-run preview only. Re-run with dry_run=false to resume the held request."
                            .to_string(),
                    ))
                    .collect(),
                errors: Vec::new(),
                next_step: Some(format!(
                    "Use the same endpoint with dry_run=false, a non-empty reason, and a supported reason_code ({}) to resume the held request.",
                    REGISTRY_RESUME_REASON_CODES.join(", ")
                )),
            }),
        ));
    }

    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live resume requires a non-empty reason for the governance audit trail"
                    .to_string(),
            )
        })?;
    let reason_code = request
        .reason_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live resume requires a non-empty reason_code for the policy audit trail"
                    .to_string(),
            )
        })?;

    let updated = RegistryGovernanceService::new(ctx.db_clone())
        .resume_publish_request(&request_id, &authority, reason, reason_code)
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "resume".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&updated.status),
            request_id: Some(updated.id.clone()),
            status: Some(request_status_label(updated.status.clone()).to_string()),
            slug: updated.slug,
            version: updated.version,
            warnings,
            errors: deserialize_message_list(&updated.validation_errors),
            next_step: publish_request_next_step(&updated.status, &updated.id),
        }),
    ))
}

/// POST /v2/catalog/runner/claim - Claim the next runnable remote validation stage
#[utoipa::path(
    post,
    path = "/v2/catalog/runner/claim",
    tag = "marketplace",
    request_body = RegistryRunnerClaimRequest,
    responses(
        (
            status = 200,
            description = "Remote validation claim response",
            body = RegistryRunnerClaimResponse
        ),
        (
            status = 401,
            description = "Missing or invalid runner token"
        ),
        (
            status = 400,
            description = "Runner claim failed validation"
        )
    )
)]
async fn claim_remote_validation_stage(
    State(ctx): State<ServerRuntimeContext>,
    headers: HeaderMap,
    Json(request): Json<RegistryRunnerClaimRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let runner_id = validate_runner_id(&request.runner_id)?;
    validate_supported_runner_stages(&request.supported_stages)?;
    let remote_executor = require_remote_executor_access(&ctx, &headers)?;
    let claim = claim_remote_validation_stage_atomic(
        ctx.db(),
        &runner_id,
        &request.supported_stages,
        remote_executor.lease_ttl_ms,
    )
    .await
    .map_err(map_remote_validation_claim_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryRunnerClaimResponse {
            accepted: true,
            claim: claim.map(runner_claim_payload),
        }),
    ))
}

/// POST /v2/catalog/runner/{claim_id}/heartbeat - Refresh a remote validation lease
#[utoipa::path(
    post,
    path = "/v2/catalog/runner/{claim_id}/heartbeat",
    tag = "marketplace",
    params(
        ("claim_id" = String, Path, description = "Remote validation claim identifier")
    ),
    request_body = RegistryRunnerHeartbeatRequest,
    responses(
        (
            status = 200,
            description = "Remote validation heartbeat accepted",
            body = RegistryRunnerMutationResponse
        ),
        (
            status = 401,
            description = "Missing or invalid runner token"
        ),
        (
            status = 400,
            description = "Heartbeat failed validation"
        )
    )
)]
async fn heartbeat_remote_validation_stage(
    State(ctx): State<ServerRuntimeContext>,
    Path(claim_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RegistryRunnerHeartbeatRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let runner_id = validate_runner_id(&request.runner_id)?;
    let remote_executor = require_remote_executor_access(&ctx, &headers)?;
    let stage = heartbeat_remote_validation_stage_atomic(
        ctx.db(),
        &claim_id,
        &runner_id,
        remote_executor.lease_ttl_ms,
    )
    .await
    .map_err(map_remote_validation_transition_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryRunnerMutationResponse {
            accepted: true,
            claim_id,
            status: validation_stage_status_label(stage.status).to_string(),
            warnings: Vec::new(),
        }),
    ))
}

/// POST /v2/catalog/runner/{claim_id}/complete - Mark a remote validation stage as passed
#[utoipa::path(
    post,
    path = "/v2/catalog/runner/{claim_id}/complete",
    tag = "marketplace",
    params(
        ("claim_id" = String, Path, description = "Remote validation claim identifier")
    ),
    request_body = RegistryRunnerCompletionRequest,
    responses(
        (
            status = 200,
            description = "Remote validation completion accepted",
            body = RegistryRunnerMutationResponse
        ),
        (
            status = 401,
            description = "Missing or invalid runner token"
        ),
        (
            status = 400,
            description = "Completion failed validation"
        )
    )
)]
async fn complete_remote_validation_stage(
    State(ctx): State<ServerRuntimeContext>,
    Path(claim_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RegistryRunnerCompletionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let runner_id = validate_runner_id(&request.runner_id)?;
    require_remote_executor_access(&ctx, &headers)?;
    let stage = finish_remote_validation_stage_atomic(
        ctx.db(),
        &claim_id,
        &runner_id,
        RemoteTerminalOutcome::Passed,
        request.detail.as_deref(),
        request.reason_code.as_deref(),
    )
    .await
    .map_err(map_remote_validation_transition_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryRunnerMutationResponse {
            accepted: true,
            claim_id,
            status: validation_stage_status_label(stage.status).to_string(),
            warnings: Vec::new(),
        }),
    ))
}

/// POST /v2/catalog/runner/{claim_id}/fail - Mark a remote validation stage as failed
#[utoipa::path(
    post,
    path = "/v2/catalog/runner/{claim_id}/fail",
    tag = "marketplace",
    params(
        ("claim_id" = String, Path, description = "Remote validation claim identifier")
    ),
    request_body = RegistryRunnerCompletionRequest,
    responses(
        (
            status = 200,
            description = "Remote validation failure accepted",
            body = RegistryRunnerMutationResponse
        ),
        (
            status = 401,
            description = "Missing or invalid runner token"
        ),
        (
            status = 400,
            description = "Failure report failed validation"
        )
    )
)]
async fn fail_remote_validation_stage(
    State(ctx): State<ServerRuntimeContext>,
    Path(claim_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RegistryRunnerCompletionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let runner_id = validate_runner_id(&request.runner_id)?;
    require_remote_executor_access(&ctx, &headers)?;
    let stage = finish_remote_validation_stage_atomic(
        ctx.db(),
        &claim_id,
        &runner_id,
        RemoteTerminalOutcome::Failed,
        request.detail.as_deref(),
        request.reason_code.as_deref(),
    )
    .await
    .map_err(map_remote_validation_transition_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryRunnerMutationResponse {
            accepted: true,
            claim_id,
            status: validation_stage_status_label(stage.status).to_string(),
            warnings: Vec::new(),
        }),
    ))
}

/// POST /v2/catalog/yank - Registry release lifecycle yank contract
#[utoipa::path(
    post,
    path = "/v2/catalog/yank",
    tag = "marketplace",
    request_body = RegistryYankRequest,
    responses(
        (
            status = 200,
            description = "Registry yank request accepted and normalized",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Yank request failed local contract validation"
        ),
        (
            status = 404,
            description = "Published release was not found"
        )
    )
)]
async fn yank(
    State(ctx): State<ServerRuntimeContext>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryYankRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_yank_request(&request)?;

    if !request.dry_run {
        let reason = request
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry yank requires a non-empty reason for the audit trail"
                        .to_string(),
                )
            })?;
        let reason_code = request
            .reason_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry yank requires a non-empty reason_code for the policy audit trail"
                        .to_string(),
                )
            })?;
        let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
        let authority = authority_from_auth(&headers, auth, "Registry yank operations")?;
        let release = RegistryGovernanceService::new(ctx.db_clone())
            .yank_release(
                &request.slug,
                &request.version,
                reason,
                reason_code,
                &authority,
            )
            .await
            .map_err(map_registry_governance_error)?;

        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "yank".to_string(),
                dry_run: false,
                accepted: true,
                request_id: release.request_id,
                status: Some(release_status_label(release.status).to_string()),
                slug: request.slug,
                version: request.version,
                warnings,
                errors: Vec::new(),
                next_step: None,
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "yank".to_string(),
            dry_run: true,
            accepted: true,
            request_id: None,
            status: Some("dry_run".to_string()),
            slug: request.slug.clone(),
            version: request.version.clone(),
                warnings,
                errors: Vec::new(),
                next_step: Some(
                "Dry-run preview only. Re-run with dry_run=false, a non-empty reason, and a supported reason_code to yank the published release."
                    .to_string(),
            ),
        }),
    ))
}

/// POST /v2/catalog/owner-transfer - Registry owner transfer governance contract
#[utoipa::path(
    post,
    path = "/v2/catalog/owner-transfer",
    tag = "marketplace",
    request_body = RegistryOwnerTransferRequest,
    responses(
        (
            status = 200,
            description = "Registry owner transfer request accepted and normalized",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Owner transfer request failed local contract or governance validation"
        ),
        (
            status = 404,
            description = "Registry owner binding was not found"
        )
    )
)]
async fn transfer_owner(
    State(ctx): State<ServerRuntimeContext>,
    headers: HeaderMap,
    auth_ext: Option<axum::Extension<AuthContextExtension>>,
    Json(request): Json<RegistryOwnerTransferRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_owner_transfer_request(&request)?;

    if !request.dry_run {
        let reason = request
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry owner transfer requires a non-empty reason for the governance audit trail"
                        .to_string(),
                )
            })?;
        let reason_code = request
            .reason_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry owner transfer requires a non-empty reason_code for the policy audit trail"
                        .to_string(),
                )
            })?;
        let auth = auth_ext.as_ref().map(|axum::Extension(a)| a);
        let authority = authority_from_auth(&headers, auth, "Registry owner transfer operations")?;
        let binding = RegistryGovernanceService::new(ctx.db_clone())
            .transfer_registry_slug_owner(
                &request.slug,
                &crate::services::registry_principal::RegistryPrincipalRef::user(
                    request.new_owner_user_id,
                ),
                reason,
                reason_code,
                &authority,
            )
            .await
            .map_err(map_registry_governance_error)?;

        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "owner_transfer".to_string(),
                dry_run: false,
                accepted: true,
                request_id: None,
                status: Some("owner_transferred".to_string()),
                slug: binding.slug,
                version: String::new(),
                warnings,
                errors: Vec::new(),
                next_step: None,
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "owner_transfer".to_string(),
            dry_run: true,
            accepted: true,
            request_id: None,
            status: Some("dry_run".to_string()),
            slug: request.slug.clone(),
            version: String::new(),
            warnings,
            errors: Vec::new(),
            next_step: Some(format!(
                "Dry-run preview only. Re-run with dry_run=false, a non-empty reason, and a supported reason_code ({}) to transfer the persisted owner binding.",
                REGISTRY_OWNER_TRANSFER_REASON_CODES.join(", ")
            )),
        }),
    ))
}

pub fn router() -> crate::routes::ServerRouter {
    read_only_router()
        .route(registry_publish_path(), post(publish))
        .route(registry_publish_status_path(), get(publish_status))
        .route(
            registry_publish_artifact_path(),
            put(upload_publish_artifact).layer(DefaultBodyLimit::max(REGISTRY_ARTIFACT_MAX_BYTES)),
        )
        .route(
            registry_publish_external_stage_path(),
            post(stage_external_prebuilt),
        )
        .route(
            registry_publish_platform_build_stage_path(),
            post(stage_platform_build),
        )
        .route(
            registry_publish_artifact_download_path(),
            get(download_publish_artifact),
        )
        .route(
            registry_publish_validate_path(),
            post(validate_publish_request_step),
        )
        .route(
            registry_publish_stage_report_path(),
            post(report_validation_stage),
        )
        .route(
            registry_publish_approve_path(),
            post(approve_publish_request),
        )
        .route(registry_publish_reject_path(), post(reject_publish_request))
        .route(
            registry_publish_request_changes_path(),
            post(request_changes_publish_request),
        )
        .route(registry_publish_hold_path(), post(hold_publish_request))
        .route(registry_publish_resume_path(), post(resume_publish_request))
        .route(
            registry_runner_claim_path(),
            post(claim_remote_validation_stage),
        )
        .route(
            registry_runner_heartbeat_path(),
            post(heartbeat_remote_validation_stage),
        )
        .route(
            registry_runner_complete_path(),
            post(complete_remote_validation_stage),
        )
        .route(
            registry_runner_fail_path(),
            post(fail_remote_validation_stage),
        )
        .route(registry_owner_transfer_path(), post(transfer_owner))
        .route(registry_yank_path(), post(yank))
}

fn registry_publish_artifact_download_path() -> &'static str {
    "/v2/catalog/publish/{request_id}/artifact/download"
}

pub fn read_only_router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(registry_catalog_path(), get(catalog))
        .route(registry_catalog_module_path(), get(catalog_module))
}

async fn first_party_catalog_modules(
    ctx: &ServerRuntimeContext,
    request_context: &RequestContext,
) -> Result<Vec<CatalogManifestModule>, Error> {
    let manifest = PlatformCompositionService::active_manifest(ctx.db())
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to load platform composition for catalog: {error}"
            ))
        })?;
    let modules = ManifestManager::catalog_modules(&manifest)
        .map_err(|error| Error::Message(format!("Failed to build marketplace catalog: {error}")))?;

    let first_party_modules = modules
        .into_iter()
        .filter(|module| module.ownership == "first_party")
        .collect::<Vec<_>>();

    RegistryGovernanceService::new(ctx.db_clone())
        .apply_catalog_projection(
            first_party_modules,
            Some(request_context.locale.as_str()),
            Some(request_context.locale.as_str()),
        )
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to project registry releases into catalog: {error}"
            ))
        })
}

fn filter_catalog_modules(
    modules: Vec<CatalogManifestModule>,
    params: &RegistryCatalogListParams,
) -> Vec<CatalogManifestModule> {
    let search = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let category = params
        .category
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let tag = params
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    modules
        .into_iter()
        .filter(|module| {
            search.is_none_or(|search| {
                let search = search.to_ascii_lowercase();
                module.slug.to_ascii_lowercase().contains(&search)
                    || module
                        .name
                        .as_deref()
                        .is_some_and(|name| name.to_ascii_lowercase().contains(&search))
                    || module.description.as_deref().is_some_and(|description| {
                        description.to_ascii_lowercase().contains(&search)
                    })
            })
        })
        .filter(|module| {
            category.is_none_or(|category| {
                module
                    .category
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(category))
            })
        })
        .filter(|module| {
            tag.is_none_or(|tag| {
                module
                    .tags
                    .iter()
                    .any(|value| value.eq_ignore_ascii_case(tag))
            })
        })
        .collect()
}

fn sort_catalog_modules(mut modules: Vec<CatalogManifestModule>) -> Vec<CatalogManifestModule> {
    modules.sort_by(|left, right| {
        left.slug
            .cmp(&right.slug)
            .then_with(|| left.crate_name.cmp(&right.crate_name))
    });
    modules
}

fn paginate_catalog_modules(
    modules: Vec<CatalogManifestModule>,
    params: &RegistryCatalogListParams,
) -> (Vec<CatalogManifestModule>, usize) {
    let total_count = modules.len();
    let offset = params.offset.unwrap_or(0).min(total_count);
    let limit = params.limit.map(|value| value.min(100));

    let modules = modules
        .into_iter()
        .skip(offset)
        .take(limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();

    (modules, total_count)
}

fn build_registry_response<T>(
    headers: &HeaderMap,
    payload: &T,
    total_count: Option<usize>,
) -> Result<Response<Body>, Error>
where
    T: serde::Serialize,
{
    let etag = registry_etag(payload)?;
    let etag_header = HeaderValue::from_str(&etag)
        .map_err(|err| Error::Message(format!("Failed to build registry ETag header: {err}")))?;
    let total_count_header = total_count.map(registry_total_count_header).transpose()?;
    if request_matches_etag(headers, &etag) {
        let mut builder = Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(CACHE_CONTROL, registry_cache_control())
            .header(ETAG, etag_header.clone());
        if let Some(total_count_header) = total_count_header.as_ref() {
            builder = builder.header(registry_total_count_header_name(), total_count_header);
        }
        return builder.body(Body::empty()).map_err(|err| {
            Error::Message(format!("Failed to build registry 304 response: {err}"))
        });
    }

    let mut response = Json(payload).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, registry_cache_control());
    response.headers_mut().insert(ETAG, etag_header);
    if let Some(total_count_header) = total_count_header {
        response
            .headers_mut()
            .insert(registry_total_count_header_name(), total_count_header);
    }

    Ok(response)
}

fn registry_cache_control() -> HeaderValue {
    HeaderValue::from_static("public, max-age=60")
}

fn registry_total_count_header_name() -> HeaderName {
    HeaderName::from_static("x-total-count")
}

fn registry_total_count_header(total_count: usize) -> Result<HeaderValue, Error> {
    HeaderValue::from_str(&total_count.to_string()).map_err(|err| {
        Error::Message(format!(
            "Failed to build registry total-count header: {err}"
        ))
    })
}

fn registry_etag<T>(payload: &T) -> Result<String, Error>
where
    T: serde::Serialize,
{
    let body = serde_json::to_vec(payload)
        .map_err(|err| Error::Message(format!("Failed to serialize registry payload: {err}")))?;
    let hash = Sha256::digest(body);
    Ok(format!("\"{}\"", hex::encode(hash)))
}

fn request_matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == "*" || candidate == etag)
        })
        .unwrap_or(false)
}

fn validate_yank_request(request: &RegistryYankRequest) -> Result<Vec<String>, Error> {
    validate_registry_slug(&request.slug)?;
    validate_registry_version(&request.version)?;

    let mut warnings = Vec::new();
    if request
        .reason
        .as_deref()
        .map(str::trim)
        .is_none_or(|reason| reason.is_empty())
    {
        warnings.push(
            "No yank reason supplied; live yank requires a non-empty reason for the governance audit trail."
                .to_string(),
        );
    }
    if request
        .reason_code
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        warnings.push(
            format!(
                "No yank reason_code supplied; live yank requires one of {} for the policy audit trail.",
                REGISTRY_YANK_REASON_CODES.join(", ")
            ),
        );
    } else if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !REGISTRY_YANK_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry yank reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_YANK_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn validate_publish_reject_request(
    request: &RegistryPublishDecisionRequest,
) -> Result<Vec<String>, Error> {
    let mut warnings = Vec::new();
    if request
        .reason
        .as_deref()
        .map(str::trim)
        .is_none_or(|reason| reason.is_empty())
    {
        warnings.push(
            "No reject reason supplied; live reject requires a non-empty reason for the governance audit trail."
                .to_string(),
        );
    }
    if request
        .reason_code
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        warnings.push(format!(
            "No reject reason_code supplied; live reject requires one of {} for the policy audit trail.",
            REGISTRY_REJECT_REASON_CODES.join(", ")
        ));
    } else if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !REGISTRY_REJECT_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry publish reject reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_REJECT_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn validate_publish_request_changes_request(
    request: &RegistryPublishDecisionRequest,
) -> Result<Vec<String>, Error> {
    validate_registry_publish_reason_code_request(
        request,
        "request-changes",
        REGISTRY_REQUEST_CHANGES_REASON_CODES,
    )
}

fn validate_publish_hold_request(
    request: &RegistryPublishDecisionRequest,
) -> Result<Vec<String>, Error> {
    validate_registry_publish_reason_code_request(request, "hold", REGISTRY_HOLD_REASON_CODES)
}

fn validate_publish_resume_request(
    request: &RegistryPublishDecisionRequest,
) -> Result<Vec<String>, Error> {
    validate_registry_publish_reason_code_request(request, "resume", REGISTRY_RESUME_REASON_CODES)
}

fn validate_publish_approve_request(request: &RegistryPublishDecisionRequest) -> Result<(), Error> {
    if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !reason_code.is_empty()
            && !REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry publish approval override reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(())
}

fn validate_registry_publish_reason_code_request(
    request: &RegistryPublishDecisionRequest,
    action: &str,
    allowed_reason_codes: &[&str],
) -> Result<Vec<String>, Error> {
    let mut warnings = Vec::new();
    if request
        .reason
        .as_deref()
        .map(str::trim)
        .is_none_or(|reason| reason.is_empty())
    {
        warnings.push(format!(
            "No {action} reason supplied; live {action} requires a non-empty reason for the governance audit trail."
        ));
    }
    if request
        .reason_code
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        warnings.push(format!(
            "No {action} reason_code supplied; live {action} requires one of {} for the policy audit trail.",
            allowed_reason_codes.join(", ")
        ));
    } else if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !allowed_reason_codes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry publish {action} reason_code '{}' is not supported; expected one of {}",
                reason_code,
                allowed_reason_codes.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn validate_validation_stage_report_request(
    request: &RegistryValidationStageReportRequest,
) -> Result<(), Error> {
    let stage = request.stage.trim();
    if stage.is_empty() {
        return Err(Error::BadRequest(
            "Registry validation stage report must include a non-empty stage".to_string(),
        ));
    }

    let status = request.status.trim().to_ascii_lowercase();
    let allowed = ["queued", "running", "passed", "failed", "blocked"];
    if !allowed.iter().any(|candidate| *candidate == status) {
        return Err(Error::BadRequest(format!(
            "Registry validation stage report status '{}' is not supported; expected one of {}",
            request.status.trim(),
            allowed.join(", ")
        )));
    }

    if request.requeue && status != "queued" {
        return Err(Error::BadRequest(
            "Registry validation stage requeue requires status='queued'".to_string(),
        ));
    }
    if !request.dry_run
        && matches!(status.as_str(), "passed" | "failed" | "blocked")
        && request
            .reason_code
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty())
    {
        return Err(Error::BadRequest(format!(
            "Live registry validation stage status '{}' requires reason_code; expected one of {}",
            status,
            REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
        )));
    }
    if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !reason_code.is_empty()
            && !REGISTRY_VALIDATION_STAGE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry validation stage reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(())
}

fn validate_owner_transfer_request(
    request: &RegistryOwnerTransferRequest,
) -> Result<Vec<String>, Error> {
    validate_registry_slug(&request.slug)?;

    let mut warnings = Vec::new();
    if request
        .reason
        .as_deref()
        .map(str::trim)
        .is_none_or(|reason| reason.is_empty())
    {
        warnings.push(
            "No transfer reason supplied; live owner transfer requires a non-empty reason for the governance audit trail."
                .to_string(),
        );
    }
    if request
        .reason_code
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        warnings.push(format!(
            "No transfer reason_code supplied; live owner transfer requires one of {} for the policy audit trail.",
            REGISTRY_OWNER_TRANSFER_REASON_CODES.join(", ")
        ));
    } else if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !REGISTRY_OWNER_TRANSFER_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry owner transfer reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_OWNER_TRANSFER_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn deserialize_message_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

fn reject_legacy_registry_headers(headers: &HeaderMap) -> Result<(), Error> {
    if headers.contains_key(LEGACY_REGISTRY_ACTOR_HEADER)
        || headers.contains_key(LEGACY_REGISTRY_PUBLISHER_HEADER)
    {
        return Err(Error::BadRequest(
            "Registry endpoints no longer accept legacy actor/publisher headers; use Authorization: Bearer with a real user session.".to_string(),
        ));
    }
    Ok(())
}

/// Derive registry authority for a mutating operation from verified
/// authentication context.
///
/// Missing auth maps to HTTP 401. OAuth service tokens are rejected with HTTP
/// 403 because registry write-paths require a session-backed user principal.
///
/// Legacy actor/publisher headers are intentionally ignored: they are untrusted
/// client input and cannot be used as authorization signals.
fn authority_from_auth(
    headers: &HeaderMap,
    auth: Option<&AuthContextExtension>,
    action_label: &str,
) -> Result<RegistryAuthority, Error> {
    // Canonical contract: missing auth -> 401, OAuth service token -> 403,
    // session-backed user bearer -> typed RegistryAuthority.
    reject_legacy_registry_headers(headers)?;
    match auth {
        None => Err(Error::Unauthorized(format!(
            "{action_label} requires authentication"
        ))),
        Some(AuthContextExtension(ctx)) if ctx.client_id.is_some() && ctx.session_id.is_nil() => {
            Err(http_error(HttpError::forbidden(
                "forbidden",
                format!(
                    "{action_label} requires a user session; OAuth service tokens are not supported"
                ),
            )))
        }
        Some(auth) => Ok(RegistryAuthority::from_auth(auth)),
    }
}

fn optional_authority_from_auth(
    headers: &HeaderMap,
    auth: Option<&AuthContextExtension>,
) -> Result<Option<RegistryAuthority>, Error> {
    reject_legacy_registry_headers(headers)?;
    Ok(match auth {
        Some(AuthContextExtension(ctx))
            if !(ctx.client_id.is_some() && ctx.session_id.is_nil()) =>
        {
            Some(RegistryAuthority::from_auth_context(ctx))
        }
        _ => None,
    })
}

fn require_remote_executor_access(
    ctx: &ServerRuntimeContext,
    headers: &HeaderMap,
) -> Result<crate::common::settings::RegistryRemoteExecutorSettings, Error> {
    let executor = ctx.settings().registry.remote_executor.clone();
    if !executor.enabled {
        return Err(Error::NotFound);
    }
    let expected_token = executor.shared_token.clone().ok_or_else(|| {
        Error::Unauthorized("Remote executor is enabled but shared_token is missing".to_string())
    })?;
    let provided_token = headers
        .get("x-rustok-runner-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Unauthorized("Missing x-rustok-runner-token header".to_string()))?;
    if provided_token != expected_token {
        return Err(Error::Unauthorized(
            "Invalid x-rustok-runner-token header".to_string(),
        ));
    }
    Ok(executor)
}

fn validate_runner_id(runner_id: &str) -> Result<String, Error> {
    let runner_id = runner_id.trim();
    if runner_id.is_empty() {
        return Err(Error::BadRequest(
            "Registry remote runner request must include a non-empty runner_id".to_string(),
        ));
    }
    Ok(runner_id.to_string())
}

fn validate_supported_runner_stages(stages: &[String]) -> Result<(), Error> {
    if stages.is_empty() {
        return Err(Error::BadRequest(
            "Registry remote runner claim must include at least one supported stage".to_string(),
        ));
    }
    for stage in stages {
        let normalized = stage.trim();
        if !matches!(
            normalized,
            "compile_smoke" | "targeted_tests" | "security_policy_review"
        ) {
            return Err(Error::BadRequest(format!(
                "Unsupported remote runner stage '{}'; expected compile_smoke, targeted_tests, or security_policy_review",
                stage
            )));
        }
    }
    Ok(())
}

fn runner_claim_payload(
    claim: crate::services::registry_governance::RegistryRemoteValidationClaim,
) -> RegistryRunnerClaimPayload {
    RegistryRunnerClaimPayload {
        claim_id: claim.claim_id,
        request_id: claim.request_id,
        slug: claim.slug,
        version: claim.version,
        stage_key: claim.stage_key,
        execution_mode: claim.execution_mode,
        runnable: claim.runnable,
        requires_manual_confirmation: claim.requires_manual_confirmation,
        allowed_terminal_reason_codes: claim.allowed_terminal_reason_codes,
        suggested_pass_reason_code: claim.suggested_pass_reason_code,
        suggested_failure_reason_code: claim.suggested_failure_reason_code,
        suggested_blocked_reason_code: claim.suggested_blocked_reason_code,
        artifact_download_url: claim.artifact_download_url,
        artifact_checksum_sha256: claim.artifact_checksum_sha256,
        crate_name: claim.crate_name,
    }
}

fn publish_request_accepted(
    status: &crate::models::registry_publish_request::RegistryPublishRequestStatus,
) -> bool {
    !matches!(
        status,
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Rejected
    )
}

fn publish_request_next_step(
    status: &crate::models::registry_publish_request::RegistryPublishRequestStatus,
    request_id: &str,
) -> Option<String> {
    match status {
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Draft => {
            Some(registry_publish_artifact_path().replace("{request_id}", request_id))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::ArtifactUploaded
        | crate::models::registry_publish_request::RegistryPublishRequestStatus::Submitted => {
            Some(format!(
                "Trigger artifact validation via POST {}",
                registry_publish_validate_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Validating => {
            Some(format!(
                "Poll {} for the latest publish lifecycle status.",
                registry_publish_status_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved => {
            Some(format!(
                "Finalize the validated publish request via POST {}",
                registry_publish_approve_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::ChangesRequested => {
            Some(format!(
                "Upload a fresh artifact revision via PUT {} before re-running validation.",
                registry_publish_artifact_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::OnHold => {
            Some(format!(
                "Resume the held publish request via POST {} when the blocking condition is cleared.",
                registry_publish_resume_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Rejected => {
            Some(format!(
                "If the request was rejected by automated validation, fix the artifact and retry via POST {}; otherwise create a new publish request after governance review resolves the rejection.",
                registry_publish_validate_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Published => None,
    }
}

fn publish_request_status_next_step(
    request: &crate::models::registry_publish_request::Model,
    request_id: &str,
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Option<String> {
    if request.status
        == crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved
        && request.artifact_origin == "external_prebuilt"
    {
        return Some(format!(
            "Stage approved external provenance and quarantine evidence via POST {} before final publication.",
            registry_publish_external_stage_path().replace("{request_id}", request_id)
        ));
    }

    if request.status
        == crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved
        && request.artifact_origin == "platform_built"
    {
        return Some(format!(
            "Bind the completed tenant-scoped platform build via POST {} before final publication.",
            registry_publish_platform_build_stage_path().replace("{request_id}", request_id)
        ));
    }

    if request.status
        == crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved
        && request.artifact_origin == "alloy_authored"
    {
        return Some(
            "Stage the reviewed Alloy source through POST /api/alloy/scripts/{id}/releases/stage before final publication."
                .to_string(),
        );
    }

    if request.status
        == crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved
        && validation_stages
            .iter()
            .any(|stage| !stage.status.eq_ignore_ascii_case("passed"))
    {
        return Some(approval_override_next_step(request_id, validation_stages));
    }

    publish_request_next_step(&request.status, request_id)
}

fn approval_override_next_step(
    request_id: &str,
    validation_stages: &[RegistryValidationStageSnapshot],
) -> String {
    format!(
        "Mark the remaining follow-up stages as passed via POST {} or approve with an explicit override reason plus reason_code ({}). Pending stages: {}.",
        registry_publish_stage_report_path().replace("{request_id}", request_id),
        REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", "),
        approval_override_stage_labels(validation_stages).join(", ")
    )
}

fn approval_override_warning_message(
    validation_stages: &[RegistryValidationStageSnapshot],
) -> String {
    format!(
        "Approval override is required because these follow-up validation stages are not passed yet: {}. Live approve must include both reason and reason_code ({}).",
        approval_override_stage_labels(validation_stages).join(", "),
        REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
    )
}

fn approval_override_stage_labels(
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<String> {
    validation_stages
        .iter()
        .filter(|stage| !stage.status.eq_ignore_ascii_case("passed"))
        .map(|stage| format!("{} ({})", stage.key, stage.status.to_ascii_lowercase()))
        .collect()
}

fn publish_status_follow_up_gate(
    gate: RegistryFollowUpGateSnapshot,
) -> RegistryPublishStatusFollowUpGate {
    RegistryPublishStatusFollowUpGate {
        key: gate.key,
        status: gate.status,
        detail: gate.detail,
        updated_at: gate.updated_at,
    }
}

fn publish_status_validation_stage(
    stage: &RegistryValidationStageSnapshot,
) -> RegistryPublishStatusValidationStage {
    RegistryPublishStatusValidationStage {
        key: stage.key.clone(),
        status: stage.status.clone(),
        detail: stage.detail.clone(),
        attempt_number: stage.attempt_number,
        updated_at: stage.updated_at.clone(),
        started_at: stage.started_at.clone(),
        finished_at: stage.finished_at.clone(),
    }
}

fn publish_status_governance_action(
    action: RegistryGovernanceActionSnapshot,
) -> RegistryGovernanceAction {
    RegistryGovernanceAction {
        key: action.key,
        reason_required: action.reason_required,
        reason_code_required: action.reason_code_required,
        reason_codes: action.reason_codes,
        destructive: action.destructive,
    }
}

fn map_registry_governance_error(error: anyhow::Error) -> Error {
    if let Some(owner_error) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ModuleGovernanceError>())
    {
        return map_module_governance_error(owner_error, &error);
    }

    let typed = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<RegistryGovernanceError>());

    match typed {
        Some(RegistryGovernanceError::Malformed(message)) => Error::BadRequest(message.clone()),
        Some(RegistryGovernanceError::Unauthorized(message)) => {
            tracing::warn!(error = %error, "Registry governance unauthorized");
            Error::Unauthorized(message.clone())
        }
        Some(RegistryGovernanceError::Forbidden(message)) => {
            tracing::warn!(error = %error, "Registry governance forbidden");
            http_error(HttpError::forbidden("forbidden", message.as_str()))
        }
        Some(RegistryGovernanceError::NotFound(_)) => Error::NotFound,
        Some(RegistryGovernanceError::Conflict(message)) => http_error(HttpError::new(
            StatusCode::CONFLICT,
            "conflict",
            message.as_str(),
        )),
        Some(RegistryGovernanceError::Internal(_)) | None => {
            tracing::error!(error = %error, "Registry governance error");
            Error::InternalServerError
        }
    }
}

/// Maps the stable owner error contract at the HTTP edge. The registry adapter
/// must not reclassify owner failures into server-local error types.
fn map_module_governance_error(error: &ModuleGovernanceError, source: &anyhow::Error) -> Error {
    match error {
        ModuleGovernanceError::InvalidLifecycleQuery
        | ModuleGovernanceError::InvalidYankCommand
        | ModuleGovernanceError::InvalidYankReasonCode(_)
        | ModuleGovernanceError::InvalidOwnerTransferCommand
        | ModuleGovernanceError::InvalidOwnerBindCommand
        | ModuleGovernanceError::InvalidPublishRequestRejectCommand
        | ModuleGovernanceError::InvalidPublishRequestChangesCommand
        | ModuleGovernanceError::InvalidPublishRequestHoldCommand
        | ModuleGovernanceError::InvalidPublishRequestResumeCommand
        | ModuleGovernanceError::InvalidPublishRequestPublicationCommand
        | ModuleGovernanceError::InvalidPublishApprovalOverride
        | ModuleGovernanceError::InvalidValidationStageReportCommand
        | ModuleGovernanceError::ValidationStageNotRequiredForArtifactOrigin { .. }
        | ModuleGovernanceError::OwnerEvidenceValidationStageCannotBeReported(_)
        | ModuleGovernanceError::InvalidValidationStageRequeue
        | ModuleGovernanceError::InvalidRemoteValidationLeaseCommand
        | ModuleGovernanceError::InvalidValidationJobEnqueueCommand
        | ModuleGovernanceError::InvalidValidationJobClaimCommand
        | ModuleGovernanceError::InvalidValidationJobResultCommand
        | ModuleGovernanceError::InvalidValidationJobRetryCommand
        | ModuleGovernanceError::InvalidPublishRequestCreateCommand
        | ModuleGovernanceError::InvalidPublishArtifactAttachCommand
        | ModuleGovernanceError::InvalidPublicationEvidenceCommand
        | ModuleGovernanceError::InvalidBuildServiceAttestationCommand
        | ModuleGovernanceError::InvalidPlatformAdmissionCommand
        | ModuleGovernanceError::InvalidPlatformBuildStageCommand
        | ModuleGovernanceError::InvalidExternalPrebuiltStageCommand
        | ModuleGovernanceError::InvalidAlloyAuthoredStageCommand
        | ModuleGovernanceError::InvalidRemoteValidationClaimStage(_)
        | ModuleGovernanceError::InvalidOwnerTransferReasonCode(_)
        | ModuleGovernanceError::InvalidPublishRequestRejectReasonCode(_)
        | ModuleGovernanceError::InvalidPublishRequestChangesReasonCode(_)
        | ModuleGovernanceError::InvalidPublishRequestHoldReasonCode(_)
        | ModuleGovernanceError::InvalidPublishRequestResumeReasonCode(_)
        | ModuleGovernanceError::InvalidPublishApprovalOverrideReasonCode(_)
        | ModuleGovernanceError::InvalidValidationStageReasonCode(_) => {
            Error::BadRequest(error.to_string())
        }
        ModuleGovernanceError::PublicationEvidenceAuthorityReserved => {
            http_error(HttpError::forbidden("forbidden", error.to_string()))
        }
        ModuleGovernanceError::ReleaseNotFound
        | ModuleGovernanceError::OwnerBindingNotFound
        | ModuleGovernanceError::PublishRequestNotFound
        | ModuleGovernanceError::ValidationJobNotFound
        | ModuleGovernanceError::ValidationStageNotFound
        | ModuleGovernanceError::RemoteValidationLeaseNotFound => Error::NotFound,
        ModuleGovernanceError::Store(_)
        | ModuleGovernanceError::InvalidLifecycleArtifactOrigin(_) => {
            tracing::error!(error = %source, "Registry governance owner store error");
            Error::InternalServerError
        }
        ModuleGovernanceError::OwnerUnchanged
        | ModuleGovernanceError::OwnerAlreadyBound
        | ModuleGovernanceError::PublishRequestReleaseAlreadyActive { .. }
        | ModuleGovernanceError::PublishRequestCannotBeRejected(_)
        | ModuleGovernanceError::PublishRequestCannotRequestChanges(_)
        | ModuleGovernanceError::PublishRequestCannotBeHeld(_)
        | ModuleGovernanceError::PublishRequestCannotBeResumed(_)
        | ModuleGovernanceError::PublishRequestCannotBePublished(_)
        | ModuleGovernanceError::PublishedRequestMissingRelease
        | ModuleGovernanceError::PublishRequestCannotAttachArtifact(_)
        | ModuleGovernanceError::PublishRequestCannotRecordPublicationEvidence(_)
        | ModuleGovernanceError::PublishRequestCannotReportValidationStage(_)
        | ModuleGovernanceError::PublishRequestCannotQueueValidation(_)
        | ModuleGovernanceError::ValidationJobNotRunning(_)
        | ModuleGovernanceError::ValidationJobRequestStateMismatch(_)
        | ModuleGovernanceError::PublishRequestMissingArtifactStorageKey
        | ModuleGovernanceError::PublishRequestMissingArtifactChecksum
        | ModuleGovernanceError::PublishRequestInvalidArtifactChecksum
        | ModuleGovernanceError::PublishRequestMissingArtifactSize
        | ModuleGovernanceError::PublishRequestMissingPlatformBuildStage
        | ModuleGovernanceError::PublishRequestMissingExternalPrebuiltStage
        | ModuleGovernanceError::PublishRequestMissingAlloyAuthoredStage
        | ModuleGovernanceError::PublishRequestArtifactOriginUnclassified
        | ModuleGovernanceError::PublishRequestMissingAuthorSignature
        | ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission
        | ModuleGovernanceError::PublishRequestMissingExternalPlatformAdmission
        | ModuleGovernanceError::PublishRequestMissingAlloyPlatformAdmission
        | ModuleGovernanceError::PublishRequestMissingTranslations
        | ModuleGovernanceError::PublishRequestInvalidTranslations
        | ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch
        | ModuleGovernanceError::RemoteValidationLeaseNotRunning(_)
        | ModuleGovernanceError::RemoteValidationLeaseExpired
        | ModuleGovernanceError::InvalidValidationStageTransition { .. }
        | ModuleGovernanceError::PublishRequestInvalidHeldFromStatus
        | ModuleGovernanceError::PlatformBuildStageIdempotencyConflict
        | ModuleGovernanceError::ExternalPrebuiltStageIdempotencyConflict
        | ModuleGovernanceError::AlloyAuthoredStageIdempotencyConflict
        | ModuleGovernanceError::PublicationIdempotencyConflict
        | ModuleGovernanceError::PublishedRequestMissingIdempotencyRecord => http_error(
            HttpError::new(StatusCode::CONFLICT, "conflict", error.to_string()),
        ),
    }
}

fn map_remote_validation_claim_error(error: anyhow::Error) -> Error {
    map_registry_governance_error(error)
}

fn map_remote_validation_transition_error(error: RegistryRemoteTransitionError) -> Error {
    match error {
        RegistryRemoteTransitionError::Invalid(message) => Error::BadRequest(message),
        RegistryRemoteTransitionError::Forbidden(message) => {
            http_error(HttpError::forbidden("forbidden", message.as_str()))
        }
        RegistryRemoteTransitionError::NotFound(_) => Error::NotFound,
        RegistryRemoteTransitionError::Conflict(message) => http_error(HttpError::new(
            StatusCode::CONFLICT,
            "conflict",
            message.as_str(),
        )),
        RegistryRemoteTransitionError::Internal(message) => {
            tracing::error!(%message, "Remote validation transition failed");
            Error::InternalServerError
        }
    }
}

fn required_non_nil_idempotency_key(headers: &HeaderMap) -> Result<Uuid, Error> {
    let value = headers
        .get("idempotency-key")
        .ok_or_else(|| Error::BadRequest("Idempotency-Key header is required".to_string()))?
        .to_str()
        .map_err(|_| Error::BadRequest("Idempotency-Key header is invalid".to_string()))?;
    let key = value
        .trim()
        .parse::<Uuid>()
        .map_err(|_| Error::BadRequest("Idempotency-Key header must be a UUID".to_string()))?;
    if key.is_nil() {
        return Err(Error::BadRequest(
            "Idempotency-Key header must not be the nil UUID".to_string(),
        ));
    }
    Ok(key)
}

fn validate_registry_slug(slug: &str) -> Result<(), Error> {
    let slug = slug.trim();
    if slug.is_empty() {
        return Err(Error::BadRequest(
            "Registry mutation request must include a non-empty slug".to_string(),
        ));
    }
    if !slug
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(Error::BadRequest(format!(
            "Registry mutation slug '{slug}' may contain only lowercase ASCII letters, digits, and hyphen"
        )));
    }
    Ok(())
}

fn validate_registry_version(version: &str) -> Result<(), Error> {
    Version::parse(version.trim()).map_err(|error| {
        Error::BadRequest(format!(
            "Registry mutation version '{version}' is not valid semver: {error}"
        ))
    })?;
    Ok(())
}
